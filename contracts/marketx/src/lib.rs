#![no_std]
#![allow(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_cast)]
#![allow(dead_code)]

//! # MarketX Smart Contract
//!
//! A decentralized escrow smart contract built on the Stellar network using Soroban.
//! This contract provides secure, trustless escrow services for peer-to-peer transactions
//! with support for multi-item releases, dispute resolution, and flexible fee structures.
//!
//! ## Features
//!
//! - **Multi-token Support**: Works with native XLM and any SEP-41 compatible token
//! - **Multi-item Escrows**: Support for milestone-based releases
//! - **Dispute Resolution**: Optional arbiter for dispute handling
//! - **Fee Management**: Configurable fee percentage with collector
//! - **Circuit Breaker**: Admin pause/unpause functionality
//! - **Comprehensive Events**: Full audit trail of all operations
//!
//! ## Core Concepts
//!
//! ### Escrow Lifecycle
//! 1. **Created** → **Pending** (after creation)
//! 2. **Pending** → **Released** (buyer releases funds)
//! 3. **Pending** → **Disputed** (buyer requests refund)
//! 4. **Disputed** → **Released** (arbiter/admin resolves for seller)
//! 5. **Disputed** → **Refunded** (arbiter/admin resolves for buyer)
//!
//! ### Key Components
//!
//! - **Buyer**: Initiates escrow and can release funds to seller
//! - **Seller**: Receives funds upon successful completion
//! - **Arbiter**: Optional third party for dispute resolution
//! - **Admin**: Contract administrator with pause/unpause and fee management
//!
//! ## Usage Examples
//!
//! ### Basic Escrow
//! ```ignore
//! // Create escrow
//! let escrow_id = contract.create_escrow(
//!     &buyer, &seller, &token_address, &amount, &None, &None, &None
//! );
//!
//! // Fund escrow (buyer transfers tokens)
//! contract.fund_escrow(&escrow_id);
//!
//! // Release funds to seller
//! contract.release_escrow(&escrow_id);
//! ```
//!
//! ### Multi-item Escrow
//! ```ignore
//! let items = vec![
//!     EscrowItem { amount: 500, released: false, description: None },
//!     EscrowItem { amount: 500, released: false, description: None },
//! ];
//!
//! let escrow_id = contract.create_escrow(
//!     &buyer, &seller, &token_address, &1000, &None, &None, &Some(items)
//! );
//!
//! // Release individual items
//! contract.release_item(&escrow_id, 0); // First item
//! contract.release_item(&escrow_id, 1); // Second item
//! ```
//!
//! ## Error Handling
//!
//! All public functions return `Result<T, ContractError>`. See the [`ContractError`] enum
//! for detailed error information and usage patterns.
//!
//! ## Events
//!
//! The contract emits comprehensive events for all state changes:
//! - `EscrowCreatedEvent`: New escrow creation
//! - `FundsReleasedEvent`: Fund releases (full or partial)
//! - `FeeCollectedEvent`: Fee collection
//! - `StatusChangeEvent`: Escrow status changes
//! - `RefundRequestedEvent`: Refund requests
//!
//! ## Security Considerations
//!
//! - All sensitive operations require proper authentication
//! - Contract can be paused by admin in emergencies
//! - Duplicate escrow prevention via content hashing
//! - Reentrancy protection on critical paths
//! - Comprehensive input validation

use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, Vec};

mod errors;
mod types;

use soroban_sdk::xdr::ToXdr;

pub use errors::ContractError;
pub use types::{
    AdminTransferredEvent, CounterEvidenceSubmittedEvent, DataKey, Escrow, EscrowCreatedEvent,
    EscrowItem, EscrowStatus, FeeChangedEvent, FeeCollectedEvent, FundsReleasedEvent,
    RefundHistoryEntry, RefundReason, RefundRequest, RefundRequestedEvent, RefundStatus,
    StatusChangeEvent, MAX_ITEMS_PER_ESCROW, MAX_METADATA_SIZE,
};

#[cfg(test)]
mod test;

/// The MarketX escrow contract.
///
/// This contract provides secure escrow services on the Stellar network.
/// All public methods are available through the contract's public interface.
#[contract]
pub struct Contract;

impl Contract {
    fn assert_admin(env: &Env) -> Result<Address, ContractError> {
        let admin = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Admin)
            .ok_or(ContractError::NotAdmin)?;

        admin.require_auth();
        Ok(admin)
    }

    fn assert_not_paused(env: &Env) -> Result<(), ContractError> {
        let paused: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Paused)
            .unwrap_or(false);

        if paused {
            Err(ContractError::ContractPaused)
        } else {
            Ok(())
        }
    }

    fn next_escrow_id(env: &Env) -> Result<u64, ContractError> {
        let current: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0);

        let next = current
            .checked_add(1)
            .ok_or(ContractError::EscrowIdOverflow)?;

        env.storage()
            .persistent()
            .set(&DataKey::EscrowCounter, &next);

        Ok(next)
    }

    fn next_refund_id(env: &Env) -> Result<u64, ContractError> {
        let current: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::RefundCount)
            .unwrap_or(0);

        let next = current
            .checked_add(1)
            .ok_or(ContractError::EscrowIdOverflow)?;

        env.storage().persistent().set(&DataKey::RefundCount, &next);

        Ok(next)
    }

    fn validate_metadata(metadata: &Option<Bytes>) -> Result<(), ContractError> {
        if let Some(ref data) = metadata {
            if data.len() > MAX_METADATA_SIZE {
                return Err(ContractError::MetadataTooLarge);
            }
        }
        Ok(())
    }

    fn generate_escrow_hash(
        env: &Env,
        buyer: &Address,
        seller: &Address,
        metadata: &Option<Bytes>,
    ) -> BytesN<32> {
        let mut bytes = Bytes::new(env);

        bytes.append(&buyer.to_xdr(env));
        bytes.append(&seller.to_xdr(env));

        if let Some(ref data) = metadata {
            bytes.append(data);
        }

        env.crypto().sha256(&bytes).into()
    }

    fn check_duplicate_escrow(
        env: &Env,
        buyer: &Address,
        seller: &Address,
        metadata: &Option<Bytes>,
    ) -> Result<(), ContractError> {
        let hash = Self::generate_escrow_hash(env, buyer, seller, metadata);

        let existing: Option<u64> = env.storage().persistent().get(&DataKey::EscrowHash(hash));

        if existing.is_some() {
            return Err(ContractError::DuplicateEscrow);
        }

        Ok(())
    }

    fn emit_status_change(
        env: &Env,
        escrow_id: u64,
        from_status: EscrowStatus,
        to_status: EscrowStatus,
        actor: Address,
    ) {
        StatusChangeEvent {
            escrow_id,
            from_status,
            to_status,
            actor,
        }
        .publish(env);
    }
}

#[contractimpl]
impl Contract {
    /// Initialize the contract with admin, fee collector, and fee settings.
    ///
    /// # Arguments
    /// * `admin` - The contract administrator address
    /// * `fee_collector` - Address that receives transaction fees
    /// * `fee_bps` - Fee percentage in basis points (100 bps = 1%)
    ///
    /// # Requirements
    /// - Must be called exactly once during contract deployment
    /// - `fee_bps` should be reasonable (typically < 1000 bps = 10%)
    ///
    /// # Events
    /// Emits no events during initialization
    ///
    /// # Errors
    /// This function cannot fail as it's the initialization function
    pub fn initialize(env: Env, admin: Address, fee_collector: Address, fee_bps: u32) {
        admin.require_auth();

        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::FeeCollector, &fee_collector);
        env.storage().persistent().set(&DataKey::FeeBps, &fee_bps);

        env.storage().persistent().set(&DataKey::Paused, &false);
        env.storage()
            .persistent()
            .set(&DataKey::EscrowCounter, &0u64);
        env.storage().persistent().set(&DataKey::RefundCount, &0u64);
        env.storage()
            .persistent()
            .set(&DataKey::TotalFundedAmount, &0i128);
    }

    /// Pause the contract, disabling all critical operations.
    ///
    /// This is a safety mechanism that can be used in emergencies.
    /// When paused, operations like creating, funding, and releasing escrows
    /// will fail with `ContractError::ContractPaused`.
    ///
    /// # Requirements
    /// - Caller must be the contract admin
    ///
    /// # Events
    /// Emits no events
    ///
    /// # Errors
    /// * `NotAdmin` - If caller is not the contract admin
    pub fn pause(env: Env) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage().persistent().set(&DataKey::Paused, &true);
        Ok(())
    }

    /// Unpause the contract, re-enabling all operations.
    ///
    /// This reverses the effects of `pause()` and allows normal operation
    /// to resume.
    ///
    /// # Requirements
    /// - Caller must be the contract admin
    ///
    /// # Events
    /// Emits no events
    ///
    /// # Errors
    /// * `NotAdmin` - If caller is not the contract admin
    pub fn unpause(env: Env) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage().persistent().set(&DataKey::Paused, &false);
        Ok(())
    }

    /// Check if the contract is currently paused.
    ///
    /// # Returns
    /// `true` if the contract is paused, `false` otherwise
    ///
    /// # Events
    /// Emits no events
    ///
    /// # Errors
    /// This function cannot fail
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    // =========================
    // 💰 ESCROW ACTIONS
    // =========================

    /// Create a new escrow with optional metadata and multiple items.
    ///
    /// # Arguments
    /// * `buyer` - The buyer's address
    /// * `seller` - The seller's address
    /// * `token` - The token contract address (can be native XLM or any SEP-41 compatible token)
    /// * `amount` - The total escrow amount (in the token's base unit, e.g., stroops for XLM)
    /// * `metadata` - Optional metadata (max 1KB)
    /// * `arbiter` - Optional arbiter mutually agreed upon by buyer and seller.
    ///               If provided, only this address may call `resolve_dispute` for this escrow.
    /// * `items` - Optional array of items/milestones. If provided, each item can be released
    ///             independently using `release_item`. The sum of item amounts must equal
    ///             the total escrow amount.
    ///
    /// # Native XLM Support
    /// To create an escrow with native XLM, pass the Stellar Asset Contract address for XLM
    /// as the `token` parameter. The native XLM SAC implements the SEP-41 Token Interface,
    /// making it fully compatible with all escrow operations.
    ///
    /// # Example - Native XLM Escrow with Items
    /// ```ignore
    /// // Amount is in stroops: 1 XLM = 10,000,000 stroops
    /// let amount: i128 = 100_000_000; // 10 XLM
    /// let xlm_address = /* native XLM SAC address */;
    ///
    /// // Create items for a multi-product purchase
    /// let items = vec![
    ///     EscrowItem { amount: 30_000_000, released: false, description: None }, // Product 1: 3 XLM
    ///     EscrowItem { amount: 40_000_000, released: false, description: None }, // Product 2: 4 XLM
    ///     EscrowItem { amount: 30_000_000, released: false, description: None }, // Product 3: 3 XLM
    /// ];
    ///
    /// let escrow_id = client.create_escrow(
    ///     &buyer, &seller, &xlm_address, &amount, &None, &None, &Some(items)
    /// );
    ///
    /// // Later, release individual items as they're delivered
    /// client.release_item(&escrow_id, &0); // Release product 1
    /// client.release_item(&escrow_id, &1); // Release product 2
    /// ```
    ///
    /// # Errors
    /// * `MetadataTooLarge` - If metadata exceeds 1KB
    /// * `DuplicateEscrow` - If an escrow with same buyer, seller, and metadata exists
    /// * `TooManyItems` - If more than MAX_ITEMS_PER_ESCROW items are provided
    /// * `ItemAmountInvalid` - If item amounts don't sum to the total escrow amount
    pub fn create_escrow(
        env: Env,
        buyer: Address,
        seller: Address,
        token: Address,
        amount: i128,
        metadata: Option<Bytes>,
        arbiter: Option<Address>,
        items: Option<Vec<EscrowItem>>,
    ) -> Result<u64, ContractError> {
        Self::assert_not_paused(&env)?;
        buyer.require_auth();

        Self::validate_metadata(&metadata)?;

        if amount <= 0 {
            return Err(ContractError::InvalidEscrowAmount);
        }

        Self::check_duplicate_escrow(&env, &buyer, &seller, &metadata)?;

        // Process items
        let escrow_items = match items {
            Some(items_vec) => {
                // Check max items limit
                if items_vec.len() > MAX_ITEMS_PER_ESCROW {
                    return Err(ContractError::TooManyItems);
                }

                // Validate item amounts sum to total
                let items_sum: i128 = items_vec.iter().map(|item| item.amount).sum();
                if items_sum != amount {
                    return Err(ContractError::ItemAmountInvalid);
                }

                items_vec
            }
            None => Vec::new(&env),
        };

        let escrow_id = Self::next_escrow_id(&env)?;

        let escrow = Escrow {
            buyer: buyer.clone(),
            seller: seller.clone(),
            token: token.clone(),
            amount,
            status: EscrowStatus::Pending,
            metadata: metadata.clone(),
            arbiter: arbiter.clone(),
            items: escrow_items,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        let hash = Self::generate_escrow_hash(&env, &buyer, &seller, &metadata);
        env.storage()
            .persistent()
            .set(&DataKey::EscrowHash(hash), &escrow_id);

        // Emit event
        let event = EscrowCreatedEvent {
            escrow_id,
            buyer,
            seller,
            token,
            amount,
            status: EscrowStatus::Pending,
            arbiter,
        };
        event.publish(&env);

        Ok(escrow_id)
    }

    pub fn get_escrow(env: Env, escrow_id: u64) -> Option<Escrow> {
        env.storage().persistent().get(&DataKey::Escrow(escrow_id))
    }

    pub fn get_escrow_metadata(env: Env, escrow_id: u64) -> Option<Bytes> {
        let escrow: Option<Escrow> = env.storage().persistent().get(&DataKey::Escrow(escrow_id));
        escrow.and_then(|e| e.metadata)
    }

    /// Get the items for an escrow.
    pub fn get_escrow_items(env: Env, escrow_id: u64) -> Option<Vec<EscrowItem>> {
        let escrow: Option<Escrow> = env.storage().persistent().get(&DataKey::Escrow(escrow_id));

        escrow.map(|e| e.items)
    }

    /// Get a paginated list of escrows.
    ///
    /// # Arguments
    /// * `start` - The starting escrow ID (1-based)
    /// * `limit` - Maximum number of escrows to return
    ///
    /// # Returns
    /// A vector of optional escrows. Missing escrows (if any) are returned as None.
    pub fn get_escrows(env: Env, start: u64, limit: u32) -> Vec<Option<Escrow>> {
        let counter: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0);

        let mut result = Vec::new(&env);

        // Handle empty case or invalid start
        if counter == 0 || start == 0 || start > counter {
            return result;
        }

        // Calculate end bound (inclusive)
        let end = (start + limit as u64 - 1).min(counter);

        // Iterate through IDs and fetch escrows
        for id in start..=end {
            let escrow: Option<Escrow> = env.storage().persistent().get(&DataKey::Escrow(id));
            result.push_back(escrow);
        }

        result
    }

    // =========================
    // 📊 ANALYTIC VIEWS
    // =========================

    /// Get the total number of escrows created.
    pub fn get_total_escrows(env: Env) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0)
    }

    pub fn get_total_funded_amount(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalFundedAmount)
            .unwrap_or(0)
    }

    pub fn get_total_released_amount(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalReleasedAmount)
            .unwrap_or(0)
    }

    pub fn fund_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        // 1. Load and validate the escrow exists
        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // 2. Validate escrow is in Pending state
        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        // 3. Enforce buyer authorization (covers the token transfer below)
        escrow.buyer.require_auth();

        // 4. Transfer funds from buyer into the contract
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        #[allow(clippy::needless_borrows_for_generic_args)]
        token_client.transfer(
            &escrow.buyer,
            &env.current_contract_address(),
            &escrow.amount,
        );

        let current_total: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalFundedAmount)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalFundedAmount, &(current_total + escrow.amount));

        Ok(())
    }

    pub fn release_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        // 1. Load and validate the escrow exists
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // 2. Validate escrow is in Pending state
        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        // 3. Enforce buyer authorization
        escrow.buyer.require_auth();
        let actor = escrow.buyer.clone();
        let from_status = escrow.status.clone();

        // 4. Calculate fee: amount * fee_bps / 10_000 (integer floor division)
        let fee_bps: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::FeeBps)
            .unwrap_or(0);
        let fee: i128 = escrow.amount * (fee_bps as i128) / 10_000;
        let seller_amount = escrow.amount - fee;

        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);

        // 5. Transfer seller_amount to seller
        #[allow(clippy::needless_borrows_for_generic_args)]
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.seller,
            &seller_amount,
        );

        // 6. Route fee to fee collector (only if fee > 0)
        if fee > 0 {
            let fee_collector: Address = env
                .storage()
                .persistent()
                .get(&DataKey::FeeCollector)
                .ok_or(ContractError::InvalidFeeConfig)?;

            #[allow(clippy::needless_borrows_for_generic_args)]
            token_client.transfer(&env.current_contract_address(), &fee_collector, &fee);

            FeeCollectedEvent {
                escrow_id,
                fee_collector,
                fee,
            }
            .publish(&env);
        }

        // 7. Update escrow status to Released
        // 5. Update escrow status to Released
        escrow.status = EscrowStatus::Released;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        // 8. Emit FundsReleasedEvent (amount = full escrow amount, fee = calculated fee)
        FundsReleasedEvent {
            escrow_id,
            amount: escrow.amount,
            fee,
        }
        .publish(&env);
        Self::emit_status_change(&env, escrow_id, from_status, escrow.status.clone(), actor);

        let current_released_total: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalReleasedAmount)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalReleasedAmount, &(current_released_total + escrow.amount));

        Ok(())
    }
    pub fn release_partial(env: Env, _escrow_id: u64, _amount: i128) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Ok(())
    }

    /// Release a specific item from an escrow.
    ///
    /// This allows partial release of escrow funds as individual items are delivered.
    /// Only the buyer can release items. Once all items are released, the escrow
    /// status changes to Released.
    ///
    /// # Arguments
    /// * `escrow_id` - The ID of the escrow
    /// * `item_index` - The index of the item to release (0-based)
    ///
    /// # Errors
    /// * `EscrowNotFound` - If the escrow doesn't exist
    /// * `InvalidEscrowState` - If the escrow is not in Pending state
    /// * `ItemNotFound` - If the item index is out of bounds
    /// * `ItemAlreadyReleased` - If the item has already been released
    pub fn release_item(env: Env, escrow_id: u64, item_index: u32) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        // 1. Load and validate the escrow exists
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // 2. Validate escrow is in Pending state
        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        // 3. Enforce buyer authorization
        escrow.buyer.require_auth();

        // 4. Validate item exists
        if item_index as u32 >= escrow.items.len() {
            return Err(ContractError::ItemNotFound);
        }

        // 5. Get the item and check if already released
        let mut item = escrow.items.get(item_index as u32).unwrap();
        if item.released {
            return Err(ContractError::ItemAlreadyReleased);
        }

        // 6. Mark item as released
        item.released = true;
        escrow.items.set(item_index as u32, item.clone());

        // 7. Transfer the item's amount to the seller
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.seller,
            &item.amount,
        );

        // 8. Check if all items are released
        let all_released = escrow.items.iter().all(|i| i.released);

        // 9. Emit FundsReleasedEvent for this item
        let event = FundsReleasedEvent {
            escrow_id,
            amount: item.amount,
            fee: 0,
        };
        event.publish(&env);

        // 10. If all items released, update escrow status
        if all_released {
            let from_status = escrow.status.clone();
            escrow.status = EscrowStatus::Released;
            Self::emit_status_change(
                &env,
                escrow_id,
                from_status,
                escrow.status.clone(),
                escrow.buyer.clone(),
            );
        }

        let current_released_total: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalReleasedAmount)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalReleasedAmount, &(current_released_total + item.amount));

        // 11. Save updated escrow
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Ok(())
    }

    pub fn refund_escrow(
        env: Env,
        escrow_id: u64,
        initiator: Address,
        amount: i128,
        reason: RefundReason,
        evidence_hash: Bytes,
    ) -> Result<u64, ContractError> {
        Self::assert_not_paused(&env)?;
        initiator.require_auth();

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if initiator != escrow.buyer {
            return Err(ContractError::Unauthorized);
        }

        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        if amount <= 0 || amount > escrow.amount {
            return Err(ContractError::InvalidEscrowAmount);
        }

        let request_id = Self::next_refund_id(&env)?;

        let refund_request = RefundRequest {
            request_id,
            escrow_id,
            requester: initiator.clone(),
            amount,
            reason,
            status: RefundStatus::Pending,
            created_at: env.ledger().timestamp(),
            evidence_hash: Some(evidence_hash.clone()),
            counter_evidence_hash: None,
        };

        env.storage()
            .persistent()
            .set(&DataKey::RefundRequest(request_id), &refund_request);

        let mut escrow_refunds: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowRefunds(escrow_id))
            .unwrap_or(Vec::new(&env));
        escrow_refunds.push_back(request_id);
        env.storage()
            .persistent()
            .set(&DataKey::EscrowRefunds(escrow_id), &escrow_refunds);

        let from_status = escrow.status.clone();
        escrow.status = EscrowStatus::Disputed;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        let event = RefundRequestedEvent {
            request_id,
            escrow_id,
            requester: initiator.clone(),
            evidence_hash: Some(evidence_hash),
        };
        event.publish(&env);

        Self::emit_status_change(
            &env,
            escrow_id,
            from_status,
            escrow.status.clone(),
            initiator,
        );

        Ok(request_id)
    }

    pub fn bump_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        let max_ttl = env.storage().max_ttl();
        let escrow_key = DataKey::Escrow(escrow_id);
        env.storage()
            .persistent()
            .extend_ttl(&escrow_key, max_ttl, max_ttl);

        let hash_key = DataKey::EscrowHash(Self::generate_escrow_hash(
            &env,
            &escrow.buyer,
            &escrow.seller,
            &escrow.metadata,
        ));
        if env.storage().persistent().has(&hash_key) {
            env.storage()
                .persistent()
                .extend_ttl(&hash_key, max_ttl, max_ttl);
        }

        Ok(())
    }

    /// Resolve a disputed escrow.
    ///
    /// If the escrow has an assigned arbiter, only that arbiter may call this.
    /// Otherwise, the contract admin may resolve it.
    ///
    /// `resolution`: 0 = release to seller, 1 = refund to buyer
    pub fn resolve_dispute(env: Env, escrow_id: u64, resolution: u32) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
        }

        // Enforce arbiter or admin authorization
        let actor = match &escrow.arbiter {
            Some(arbiter) => {
                arbiter.require_auth();
                arbiter.clone()
            }
            None => Self::assert_admin(&env)?,
        };
        let from_status = escrow.status.clone();

        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);

        if resolution == 0 {
            // Release to seller
            token_client.transfer(
                &env.current_contract_address(),
                &escrow.seller,
                &escrow.amount,
            );
            escrow.status = EscrowStatus::Released;
        } else if resolution == 1 {
            // Refund to buyer
            token_client.transfer(
                &env.current_contract_address(),
                &escrow.buyer,
                &escrow.amount,
            );
            escrow.status = EscrowStatus::Refunded;
        } else {
            return Err(ContractError::InvalidEscrowState);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Self::emit_status_change(&env, escrow_id, from_status, escrow.status.clone(), actor);

        let current_released_total: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalReleasedAmount)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalReleasedAmount, &(current_released_total + escrow.amount));

        Ok(())
    }

    // =========================
    // 🔧 ADMIN FUNCTIONS
    // =========================

    /// Upgrade the contract WASM.
    pub fn upgrade(env: Env, new_wasm_hash: soroban_sdk::BytesN<32>) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }

    /// Propose a new admin. The transfer is not complete until the new admin accepts.
    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage()
            .persistent()
            .set(&DataKey::ProposedAdmin, &new_admin);
        Ok(())
    }

    /// Accept the administrative role. Must be called by the proposed admin.
    pub fn accept_admin(env: Env) -> Result<(), ContractError> {
        let proposed_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::ProposedAdmin)
            .ok_or(ContractError::NotProposedAdmin)?;

        // The proposed admin must authenticate this transaction
        proposed_admin.require_auth();

        let old_admin: Address = env.storage().persistent().get(&DataKey::Admin).unwrap();

        // Transfer the admin role
        env.storage()
            .persistent()
            .set(&DataKey::Admin, &proposed_admin);

        // Clean up the proposal
        env.storage().persistent().remove(&DataKey::ProposedAdmin);

        // Emit the event
        AdminTransferredEvent {
            old_admin,
            new_admin: proposed_admin,
        }
        .publish(&env);

        Ok(())
    }

    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Admin)
    }

    pub fn set_fee_percentage(env: Env, fee_bps: u32) -> Result<(), ContractError> {
        let admin = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Admin)
            .ok_or(ContractError::NotAdmin)?;
        admin.require_auth();
        let old_fee_bps = env
            .storage()
            .persistent()
            .get(&DataKey::FeeBps)
            .unwrap_or(0);

        if fee_bps > 1000 {
            return Err(ContractError::InvalidFeeConfig);
        }

        env.storage().persistent().set(&DataKey::FeeBps, &fee_bps);

        FeeChangedEvent {
            old_fee_bps,
            new_fee_bps: fee_bps,
            actor: admin,
        }
        .publish(&env);

        Ok(())
    }

    pub fn get_fee_bps(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::FeeBps)
            .unwrap_or(0)
    }

    /// Get a refund request by ID.
    pub fn get_refund_request(env: Env, request_id: u64) -> Option<RefundRequest> {
        env.storage()
            .persistent()
            .get(&DataKey::RefundRequest(request_id))
    }

    /// Get the total number of refund requests.
    pub fn get_refund_count(env: Env) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::RefundCount)
            .unwrap_or(0)
    }
}
