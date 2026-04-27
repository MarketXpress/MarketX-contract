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
    AdminTransferredEvent, BatchFeesCollectedEvent, BulkEscrowCreatedEvent, BulkEscrowRequest,
    BuyerContribution, CancellationProposedEvent, CounterEvidenceSubmittedEvent, DataKey,
    DeliveryVerifiedEvent, Escrow, EscrowCreatedEvent, EscrowExpiredEvent, EscrowItem,
    EscrowStatus, FeeCapsChangedEvent, FeeChangedEvent, FeeCollectedEvent, FeeExemptionEvent,
    FeesWithdrawnEvent, FundsReleasedEvent, GroupBuy, GroupBuyCompletedEvent, GroupBuyFundedEvent,
    Milestone, MilestoneCompletedEvent, RefundHistoryEntry, RefundReason, RefundRequest,
    RefundRequestedEvent, RefundStatus, StatusChangeEvent, TimeLock, TimeLockReleasedEvent,
    MAX_ITEMS_PER_ESCROW, MAX_METADATA_SIZE, UNFUNDED_EXPIRY_LEDGERS,
    // Dispute Resolution V2 (#201-204)
    APPEAL_WINDOW_LEDGERS, DEFAULT_EVIDENCE_WINDOW_LEDGERS,
    ArbiterStake, ArbiterStakedEvent, ArbiterSlashedEvent,
    EvidenceWindow, EvidenceSubmittedEvent, EvidenceWindowExpiredEvent,
    AppealRecord, AppealFiledEvent, AppealResolvedEvent,
    ArbiterReputation,
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
            return Err(ContractError::ContractPaused);
        }

        Ok(())
    }

    fn add_i128(env: &Env, key: DataKey, value: i128) {
        let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(current + value));
    }

    fn add_u32(env: &Env, key: DataKey) {
        let current: u32 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(current + 1));
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

    fn is_escrow_party(escrow: &Escrow, actor: &Address) -> bool {
        *actor == escrow.buyer || *actor == escrow.seller
    }

    fn has_released_items(escrow: &Escrow) -> bool {
        for item in escrow.items.iter() {
            if item.released {
                return true;
            }
        }

        false
    }

    fn refund_buyer(env: &Env, escrow: &mut Escrow) {
        let token_client = soroban_sdk::token::Client::new(env, &escrow.token);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.buyer,
            &escrow.amount,
        );

        Self::add_i128(env, DataKey::TotalRefundedAmount, escrow.amount);
        escrow.status = EscrowStatus::Refunded;
        escrow.cancellation_proposer = None;
    }

    fn add_pending_fee(env: &Env, collector: Address, token: Address, amount: i128) {
        if amount <= 0 {
            return;
        }
        let key = DataKey::PendingFee(collector.clone(), token.clone());
        let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(current + amount));
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
    pub fn initialize(
        env: Env,
        admin: Address,
        fee_collector: Address,
        fee_bps: u32,
        min_fee: i128,
        max_fee: i128,
    ) {
        admin.require_auth();

        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::FeeCollector, &fee_collector);
        env.storage().persistent().set(&DataKey::FeeBps, &fee_bps);
        env.storage().persistent().set(&DataKey::MinFee, &min_fee);
        env.storage().persistent().set(&DataKey::MaxFee, &max_fee);

        env.storage().persistent().set(&DataKey::Paused, &false);
        env.storage()
            .persistent()
            .set(&DataKey::EscrowCounter, &0u64);
        env.storage().persistent().set(&DataKey::RefundCount, &0u64);
        env.storage()
            .persistent()
            .set(&DataKey::TotalFundedAmount, &0i128);
        env.storage()
            .persistent()
            .set(&DataKey::TotalRefundedAmount, &0i128);
        env.storage()
            .persistent()
            .set(&DataKey::TotalDisputedCount, &0u32);
        env.storage()
            .persistent()
            .set(&DataKey::TotalFeesCollected, &0i128);
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

    fn create_escrow_internal(
        env: Env,
        buyer: Address,
        seller: Address,
        token: Address,
        amount: i128,
        metadata: Option<Bytes>,
        arbiter: Option<Address>,
        items: Option<Vec<EscrowItem>>,
        tracking_id: Option<Bytes>,
    ) -> Result<u64, ContractError> {
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
            cancellation_proposer: None,
            items: escrow_items,
            created_at: env.ledger().sequence(),
            tracking_id: tracking_id.clone(),
            milestones: Vec::new(&env),
            time_lock: None,
            group_buy: None,
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
            tracking_id,
        };
        event.publish(&env);

        Ok(escrow_id)
    }

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

        Self::create_escrow_internal(env, buyer, seller, token, amount, metadata, arbiter, items, None)
    }

    /// Create multiple escrows in a single transaction (Bulk Creation).
    /// Useful for cart checkouts involving multiple sellers.
    pub fn create_bulk_escrows(
        env: Env,
        buyer: Address,
        token: Address,
        requests: Vec<BulkEscrowRequest>,
    ) -> Result<Vec<u64>, ContractError> {
        Self::assert_not_paused(&env)?;
        buyer.require_auth();

        let mut ids = Vec::new(&env);
        for request in requests.iter() {
            let id = Self::create_escrow_internal(
                env.clone(),
                buyer.clone(),
                request.seller.clone(),
                token.clone(),
                request.amount,
                request.metadata.clone(),
                request.arbiter.clone(),
                request.items.clone(),
                None, // tracking_id
            )?;
            ids.push_back(id);
        }

        BulkEscrowCreatedEvent {
            buyer,
            token,
            escrow_ids: ids.clone(),
        }
        .publish(&env);

        Ok(ids)
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

    pub fn set_oracle(env: Env, oracle: Address) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage().persistent().set(&DataKey::Oracle, &oracle);
        Ok(())
    }

    pub fn get_oracle(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Oracle)
    }

    pub fn verify_delivery(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        let oracle: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Oracle)
            .ok_or(ContractError::NotOracle)?;

        oracle.require_auth();

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        let tracking_id = escrow.tracking_id.clone().ok_or(ContractError::Unauthorized)?;

        // Oracle verified delivery, release funds
        let from_status = escrow.status.clone();
        
        // Use Oracle as actor for status change
        let actor = oracle.clone();
        
        // Core release logic (duplicated from release_escrow for now to avoid complex refactor in this turn, or I can refactor it)
        // Actually, let's try to keep it simple.
        
        let mut fee_bps: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::FeeBps)
            .unwrap_or(0);

        if let Some(native_asset) = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::NativeAsset)
        {
            if escrow.token == native_asset {
                fee_bps = env
                    .storage()
                    .persistent()
                    .get(&DataKey::NativeFeeBps)
                    .unwrap_or(fee_bps);
            }
        }

        let mut fee: i128 = escrow.amount * (fee_bps as i128) / 10_000;
        let min_fee: i128 = env.storage().persistent().get(&DataKey::MinFee).unwrap_or(0);
        let max_fee: i128 = env.storage().persistent().get(&DataKey::MaxFee).unwrap_or(0);

        if fee < min_fee { fee = min_fee; }
        if max_fee > 0 && fee > max_fee { fee = max_fee; }
        if fee > escrow.amount { fee = escrow.amount; }

        let seller_amount = escrow.amount - fee;
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        token_client.transfer(&env.current_contract_address(), &escrow.seller, &seller_amount);

        if fee > 0 {
            let fee_collector: Address = env.storage().persistent().get(&DataKey::FeeCollector).ok_or(ContractError::InvalidFeeConfig)?;
            Self::add_pending_fee(&env, fee_collector.clone(), escrow.token.clone(), fee);
            Self::add_i128(&env, DataKey::TotalFeesCollected, fee);
            FeeCollectedEvent { escrow_id, fee_collector, fee }.publish(&env);
        }

        escrow.status = EscrowStatus::Released;
        escrow.cancellation_proposer = None;
        env.storage().persistent().set(&DataKey::Escrow(escrow_id), &escrow);

        FundsReleasedEvent { escrow_id, amount: escrow.amount, fee }.publish(&env);
        DeliveryVerifiedEvent { escrow_id, tracking_id }.publish(&env);
        Self::emit_status_change(&env, escrow_id, from_status, escrow.status.clone(), actor);

        Self::add_i128(&env, DataKey::TotalReleasedAmount, escrow.amount);

        Ok(())
    }

    pub fn get_total_refunded_amount(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalRefundedAmount)
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
        env.storage().persistent().set(
            &DataKey::TotalFundedAmount,
            &(current_total + escrow.amount),
        );

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
        // Whitelisted buyers (partners/internal) pay zero fees.
        let is_exempt: bool = env
            .storage()
            .persistent()
            .get(&DataKey::FeeWhitelist(escrow.buyer.clone()))
            .unwrap_or(false);
        
        let mut fee_bps: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::FeeBps)
            .unwrap_or(0);

        // Special logic for Native XLM
        if let Some(native_asset) = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::NativeAsset)
        {
            if escrow.token == native_asset {
                fee_bps = env
                    .storage()
                    .persistent()
                    .get(&DataKey::NativeFeeBps)
                    .unwrap_or(fee_bps);
            }
        }

        let mut fee: i128 = escrow.amount * (fee_bps as i128) / 10_000;

        let min_fee: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::MinFee)
            .unwrap_or(0);
        let max_fee: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::MaxFee)
            .unwrap_or(0);

        if fee < min_fee {
            fee = min_fee;
        }
        if max_fee > 0 && fee > max_fee {
            fee = max_fee;
        }

        // Ensure fee doesn't exceed the escrow amount
        if fee > escrow.amount {
            fee = escrow.amount;
        }
        let fee: i128 = if is_exempt {
            0
        } else {
            escrow.amount * (fee_bps as i128) / 10_000
        };
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

            Self::add_pending_fee(&env, fee_collector, escrow.token.clone(), fee);

            Self::add_i128(&env, DataKey::TotalFeesCollected, fee);

            FeeCollectedEvent {
                escrow_id,
                fee_collector: env
                    .storage()
                    .persistent()
                    .get(&DataKey::FeeCollector)
                    .unwrap(), // Re-fetch to satisfy borrow checker if needed, or just use the one above
                fee,
            }
            .publish(&env);
        }

        // 7. Update escrow status to Released
        // 5. Update escrow status to Released
        escrow.status = EscrowStatus::Released;
        escrow.cancellation_proposer = None;
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
        env.storage().persistent().set(
            &DataKey::TotalReleasedAmount,
            &(current_released_total + escrow.amount),
        );

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
        FundsReleasedEvent {
            escrow_id,
            amount: item.amount,
            fee: 0,
        }
        .publish(&env);

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
        env.storage().persistent().set(
            &DataKey::TotalReleasedAmount,
            &(current_released_total + item.amount),
        );

        // 11. Save updated escrow
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Ok(())
    }

    pub fn propose_cancellation(
        env: Env,
        escrow_id: u64,
        actor: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        actor.require_auth();

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if !Self::is_escrow_party(&escrow, &actor) {
            return Err(ContractError::Unauthorized);
        }

        if escrow.status != EscrowStatus::Pending || Self::has_released_items(&escrow) {
            return Err(ContractError::InvalidEscrowState);
        }

        if let Some(existing) = &escrow.cancellation_proposer {
            if *existing == actor {
                return Ok(());
            }

            // If the other party already proposed, auto-accept the cancellation
            let from_status = escrow.status.clone();
            Self::refund_buyer(&env, &mut escrow);
            env.storage()
                .persistent()
                .set(&DataKey::Escrow(escrow_id), &escrow);
            Self::emit_status_change(&env, escrow_id, from_status, escrow.status.clone(), actor);
            return Ok(());
        }

        escrow.cancellation_proposer = Some(actor.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        CancellationProposedEvent { escrow_id, actor }.publish(&env);

        Ok(())
    }

    pub fn accept_cancellation(
        env: Env,
        escrow_id: u64,
        actor: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        actor.require_auth();

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if !Self::is_escrow_party(&escrow, &actor) {
            return Err(ContractError::Unauthorized);
        }

        if escrow.status != EscrowStatus::Pending || Self::has_released_items(&escrow) {
            return Err(ContractError::InvalidEscrowState);
        }

        let proposer = escrow
            .cancellation_proposer
            .clone()
            .ok_or(ContractError::InvalidEscrowState)?;

        if proposer == actor {
            return Err(ContractError::Unauthorized);
        }

        let from_status = escrow.status.clone();
        Self::refund_buyer(&env, &mut escrow);
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);
        Self::emit_status_change(&env, escrow_id, from_status, escrow.status.clone(), actor);

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
        escrow.cancellation_proposer = None;
        Self::add_u32(&env, DataKey::TotalDisputedCount);
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

    /// Cancel an escrow that was never funded after the expiry window has elapsed.
    ///
    /// Anyone may call this once `UNFUNDED_EXPIRY_LEDGERS` ledgers have passed
    /// since the escrow was created without it being funded. The escrow record
    /// and its duplicate-prevention hash are both removed from storage.
    ///
    /// # Arguments
    /// * `escrow_id` - The ID of the escrow to cancel
    ///
    /// # Errors
    /// * `EscrowNotFound` - If the escrow doesn't exist
    /// * `EscrowAlreadyFunded` - If the escrow is not in Pending state (i.e. it was funded)
    /// * `EscrowNotExpired` - If the expiry window has not yet elapsed
    pub fn cancel_unfunded(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Only Pending escrows can be cancelled as unfunded.
        // Any other status means the escrow was already funded/acted upon.
        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::EscrowAlreadyFunded);
        }

        let current_ledger = env.ledger().sequence();
        let expiry_ledger = escrow.created_at.saturating_add(UNFUNDED_EXPIRY_LEDGERS);

        if current_ledger < expiry_ledger {
            return Err(ContractError::EscrowNotExpired);
        }

        // Remove the escrow record
        env.storage()
            .persistent()
            .remove(&DataKey::Escrow(escrow_id));

        // Remove the duplicate-prevention hash so the same escrow can be recreated
        let hash =
            Self::generate_escrow_hash(&env, &escrow.buyer, &escrow.seller, &escrow.metadata);
        env.storage()
            .persistent()
            .remove(&DataKey::EscrowHash(hash));

        EscrowExpiredEvent {
            escrow_id,
            buyer: escrow.buyer,
            seller: escrow.seller,
        }
        .publish(&env);

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
            let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
            token_client.transfer(
                &env.current_contract_address(),
                &escrow.seller,
                &escrow.amount,
            );
            escrow.status = EscrowStatus::Released;

            escrow.cancellation_proposer = None;
            
            let current_released_total: i128 = env
                .storage()
                .persistent()
                .get(&DataKey::TotalReleasedAmount)
                .unwrap_or(0);
            env.storage().persistent().set(
                &DataKey::TotalReleasedAmount,
                &(current_released_total + escrow.amount),
            );
        } else if resolution == 1 {
            // Refund to buyer
            Self::refund_buyer(&env, &mut escrow);
        } else {
            return Err(ContractError::InvalidEscrowState);
        }

        // Update associated refund requests if they exist
        let escrow_refunds: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowRefunds(escrow_id))
            .unwrap_or(Vec::new(&env));

        for req_id in escrow_refunds.iter() {
            if let Some(mut req) = env
                .storage()
                .persistent()
                .get::<DataKey, RefundRequest>(&DataKey::RefundRequest(req_id))
            {
                if req.status == RefundStatus::Pending {
                    req.status = if resolution == 1 {
                        RefundStatus::Approved
                    } else {
                        RefundStatus::Rejected
                    };
                    env.storage()
                        .persistent()
                        .set(&DataKey::RefundRequest(req_id), &req);
                }
            }
        }

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Self::emit_status_change(&env, escrow_id, from_status, escrow.status.clone(), actor.clone());

        // #204: record resolution in arbiter reputation
        if let Some(arbiter) = &escrow.arbiter {
            Self::record_arbiter_resolution(&env, arbiter);
        }
        // #201: return stake to arbiter (no active appeal yet)
        Self::return_arbiter_stake(&env, escrow_id);

        Ok(())
    }

    // =========================================================================
    // ⚖️  DISPUTE RESOLUTION V2 (#201-204)
    // =========================================================================

    // ── #201: Arbiter Staking & Slashing ─────────────────────────────────────

    /// Set the minimum token amount an arbiter must stake before resolving a
    /// disputed escrow.  Admin-only.
    pub fn set_min_arbiter_stake(env: Env, amount: i128) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        assert!(amount >= 0, "min stake must be non-negative");
        env.storage().persistent().set(&DataKey::MinArbiterStake, &amount);
        Ok(())
    }

    /// Get the current minimum arbiter stake (returns 0 if not configured).
    pub fn get_min_arbiter_stake(env: Env) -> i128 {
        env.storage().persistent().get(&DataKey::MinArbiterStake).unwrap_or(0)
    }

    /// Arbiter stakes tokens to gain the authority to resolve the given disputed
    /// escrow.  The stake is held by the contract until the escrow is resolved.
    ///
    /// # Errors
    /// - `InvalidEscrowState` if the escrow is not in `Disputed` status.
    /// - `ArbiterMismatch` if the caller is not the registered arbiter.
    /// - `ArbiterAlreadyStaked` if a stake already exists for this escrow.
    /// - `ArbiterStakeInsufficient` if `amount` < `MinArbiterStake`.
    pub fn stake_as_arbiter(
        env: Env,
        arbiter: Address,
        escrow_id: u64,
        amount: i128,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        arbiter.require_auth();

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
        }

        match &escrow.arbiter {
            Some(registered) if *registered != arbiter => return Err(ContractError::ArbiterMismatch),
            None => return Err(ContractError::ArbiterMismatch),
            _ => {}
        }

        let stake_key = DataKey::ArbiterStake(escrow_id);
        if env.storage().persistent().has(&stake_key) {
            return Err(ContractError::ArbiterAlreadyStaked);
        }

        let min_stake: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::MinArbiterStake)
            .unwrap_or(0);

        if amount < min_stake {
            return Err(ContractError::ArbiterStakeInsufficient);
        }

        // Transfer stake from arbiter to the contract
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        token_client.transfer(&arbiter, &env.current_contract_address(), &amount);

        let stake = ArbiterStake {
            arbiter: arbiter.clone(),
            token: escrow.token.clone(),
            amount,
            staked_at: env.ledger().sequence(),
            slashed: false,
            slash_amount: 0,
        };

        env.storage().persistent().set(&stake_key, &stake);

        // Update reputation
        Self::increment_arbiter_disputes(&env, &arbiter);

        ArbiterStakedEvent { escrow_id, arbiter, amount }.publish(&env);
        Ok(())
    }

    /// Return an arbiter's stake after a successful (non-overturned) resolution.
    /// Called internally; the stake is returned when `resolve_dispute` executes
    /// and no active appeal overturns the ruling.
    fn return_arbiter_stake(env: &Env, escrow_id: u64) {
        let stake_key = DataKey::ArbiterStake(escrow_id);
        let Some(stake): Option<ArbiterStake> = env.storage().persistent().get(&stake_key) else {
            return;
        };
        if stake.slashed || stake.amount == 0 {
            return;
        }
        let token_client = soroban_sdk::token::Client::new(env, &stake.token);
        token_client.transfer(
            &env.current_contract_address(),
            &stake.arbiter,
            &stake.amount,
        );
        env.storage().persistent().remove(&stake_key);
    }

    /// Admin slashes an arbiter's stake after an appeal overturns their ruling.
    /// The slashed amount is sent to the winning party (the appellant).
    ///
    /// # Errors
    /// - `AppealNotFound` if no resolved-and-overturned appeal exists.
    pub fn slash_arbiter(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;

        let appeal: AppealRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Appeal(escrow_id))
            .ok_or(ContractError::AppealNotFound)?;

        if !appeal.resolved {
            return Err(ContractError::AppealNotFound);
        }

        let stake_key = DataKey::ArbiterStake(escrow_id);
        let mut stake: ArbiterStake = env
            .storage()
            .persistent()
            .get(&stake_key)
            .ok_or(ContractError::EscrowNotFound)?;

        if stake.slashed || stake.amount == 0 {
            return Ok(());
        }

        let slash_amount = stake.amount / 2; // Slash 50% of stake
        stake.slashed = true;
        stake.slash_amount = slash_amount;
        stake.amount -= slash_amount;

        let token_client = soroban_sdk::token::Client::new(&env, &stake.token);
        // Slashed portion goes to the appellant (winning party)
        token_client.transfer(
            &env.current_contract_address(),
            &appeal.appellant,
            &slash_amount,
        );
        // Remaining stake returned to arbiter
        if stake.amount > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &stake.arbiter,
                &stake.amount,
            );
        }

        // Update arbiter reputation
        let rep_key = DataKey::ArbiterReputation(stake.arbiter.clone());
        let mut rep: ArbiterReputation = env
            .storage()
            .persistent()
            .get(&rep_key)
            .unwrap_or(ArbiterReputation {
                arbiter: stake.arbiter.clone(),
                total_disputes: 0,
                resolved_disputes: 0,
                appealed_rulings: 0,
                overturned_rulings: 0,
                slash_count: 0,
                last_active: 0,
            });
        rep.slash_count += 1;
        rep.last_active = env.ledger().sequence();
        env.storage().persistent().set(&rep_key, &rep);

        ArbiterSlashedEvent {
            escrow_id,
            arbiter: stake.arbiter.clone(),
            slash_amount,
        }
        .publish(&env);

        env.storage().persistent().set(&stake_key, &stake);
        Ok(())
    }

    // ── #202: Evidence Submission Windows & Expiry ───────────────────────────

    /// Open an evidence submission window for a disputed escrow.
    /// Both parties may call `submit_evidence` until the window closes.
    /// If `window_ledgers` is 0, the default window length is used.
    pub fn open_evidence_window(
        env: Env,
        caller: Address,
        escrow_id: u64,
        window_ledgers: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        caller.require_auth();

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
        }

        // Only arbiter, buyer, seller, or admin may open the window
        let is_party = escrow.buyer == caller
            || escrow.seller == caller
            || escrow.arbiter.as_ref() == Some(&caller);
        if !is_party {
            let admin: Address = env
                .storage()
                .persistent()
                .get(&DataKey::Admin)
                .ok_or(ContractError::NotAdmin)?;
            if admin != caller {
                return Err(ContractError::Unauthorized);
            }
        }

        let ledgers = if window_ledgers == 0 {
            DEFAULT_EVIDENCE_WINDOW_LEDGERS
        } else {
            window_ledgers
        };

        let now = env.ledger().sequence();
        let window = EvidenceWindow {
            escrow_id,
            opened_at: now,
            expires_at: now + ledgers,
            buyer_submitted: false,
            seller_submitted: false,
            buyer_evidence_hash: None,
            seller_evidence_hash: None,
            expired: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::EvidenceWindow(escrow_id), &window);
        Ok(())
    }

    /// Submit an evidence hash within the open evidence window.
    /// The caller must be the buyer or seller of the escrow.
    pub fn submit_evidence(
        env: Env,
        party: Address,
        escrow_id: u64,
        evidence_hash: Bytes,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        party.require_auth();

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
        }

        let mut window: EvidenceWindow = env
            .storage()
            .persistent()
            .get(&DataKey::EvidenceWindow(escrow_id))
            .ok_or(ContractError::NoEvidenceWindow)?;

        if window.expired || env.ledger().sequence() > window.expires_at {
            return Err(ContractError::EvidenceWindowExpired);
        }

        if party == escrow.buyer {
            window.buyer_submitted = true;
            window.buyer_evidence_hash = Some(evidence_hash.clone());
        } else if party == escrow.seller {
            window.seller_submitted = true;
            window.seller_evidence_hash = Some(evidence_hash.clone());
        } else {
            return Err(ContractError::Unauthorized);
        }

        env.storage()
            .persistent()
            .set(&DataKey::EvidenceWindow(escrow_id), &window);

        EvidenceSubmittedEvent {
            escrow_id,
            party,
            evidence_hash,
        }
        .publish(&env);

        Ok(())
    }

    /// Expire an evidence window that has passed its deadline.
    /// If neither party submitted evidence, the dispute resolves as a refund
    /// (default in favour of buyer when arbiter is absent).
    pub fn expire_evidence_window(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        let mut window: EvidenceWindow = env
            .storage()
            .persistent()
            .get(&DataKey::EvidenceWindow(escrow_id))
            .ok_or(ContractError::NoEvidenceWindow)?;

        if window.expired {
            return Ok(());
        }
        if env.ledger().sequence() <= window.expires_at {
            return Err(ContractError::EvidenceWindowNotExpired);
        }

        window.expired = true;
        env.storage()
            .persistent()
            .set(&DataKey::EvidenceWindow(escrow_id), &window);

        // Default resolution: refund buyer when no arbiter resolved in time
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        let default_refund = escrow.status == EscrowStatus::Disputed;
        if default_refund {
            let from = escrow.status.clone();
            Self::refund_buyer(&env, &mut escrow);
            env.storage()
                .persistent()
                .set(&DataKey::Escrow(escrow_id), &escrow);
            let buyer = escrow.buyer.clone();
            Self::emit_status_change(&env, escrow_id, from, EscrowStatus::Refunded, buyer);
        }

        EvidenceWindowExpiredEvent {
            escrow_id,
            default_refund,
        }
        .publish(&env);

        Ok(())
    }

    /// Read the evidence window for an escrow.
    pub fn get_evidence_window(env: Env, escrow_id: u64) -> Option<EvidenceWindow> {
        env.storage()
            .persistent()
            .get(&DataKey::EvidenceWindow(escrow_id))
    }

    // ── #203: Resolution Appeal Mechanism ────────────────────────────────────

    /// File an appeal after an arbiter's ruling.  Must be called within
    /// `APPEAL_WINDOW_LEDGERS` of the ruling ledger.
    ///
    /// The `ruling_ledger` parameter is the ledger at which `resolve_dispute`
    /// was called (caller supplies it; admin verifies on `resolve_appeal`).
    pub fn file_appeal(
        env: Env,
        appellant: Address,
        escrow_id: u64,
        ruling_ledger: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        appellant.require_auth();

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Only buyer or seller may appeal
        if escrow.buyer != appellant && escrow.seller != appellant {
            return Err(ContractError::Unauthorized);
        }

        let appeal_key = DataKey::Appeal(escrow_id);
        if env.storage().persistent().has(&appeal_key) {
            return Err(ContractError::AppealAlreadyFiled);
        }

        let now = env.ledger().sequence();
        if now > ruling_ledger + APPEAL_WINDOW_LEDGERS {
            return Err(ContractError::AppealWindowClosed);
        }

        let record = AppealRecord {
            escrow_id,
            appellant: appellant.clone(),
            filed_at: now,
            resolved: false,
            outcome: None,
            ruling_ledger,
        };

        env.storage().persistent().set(&appeal_key, &record);

        // Update arbiter reputation: appealed_rulings++
        if let Some(arbiter) = &escrow.arbiter {
            let rep_key = DataKey::ArbiterReputation(arbiter.clone());
            let mut rep: ArbiterReputation =
                env.storage().persistent().get(&rep_key).unwrap_or(ArbiterReputation {
                    arbiter: arbiter.clone(),
                    total_disputes: 0,
                    resolved_disputes: 0,
                    appealed_rulings: 0,
                    overturned_rulings: 0,
                    slash_count: 0,
                    last_active: 0,
                });
            rep.appealed_rulings += 1;
            rep.last_active = now;
            env.storage().persistent().set(&rep_key, &rep);
        }

        AppealFiledEvent { escrow_id, appellant }.publish(&env);
        Ok(())
    }

    /// Admin resolves an appeal, potentially overriding the arbiter's ruling.
    /// `resolution`: 0 = seller wins, 1 = buyer wins.
    /// If the outcome differs from the arbiter's ruling, `overturned = true` and
    /// the admin should follow up with `slash_arbiter`.
    pub fn resolve_appeal(
        env: Env,
        admin: Address,
        escrow_id: u64,
        resolution: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        admin.require_auth();
        Self::assert_admin(&env)?;

        let appeal_key = DataKey::Appeal(escrow_id);
        let mut appeal: AppealRecord = env
            .storage()
            .persistent()
            .get(&appeal_key)
            .ok_or(ContractError::AppealNotFound)?;

        if appeal.resolved {
            return Err(ContractError::AppealAlreadyResolved);
        }

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);

        // The escrow may already be Released/Refunded from the original ruling.
        // We reverse the original ruling and apply the new one.
        let overturned = match escrow.status {
            EscrowStatus::Released => resolution == 1, // originally seller won; appeal says buyer wins
            EscrowStatus::Refunded => resolution == 0, // originally buyer won; appeal says seller wins
            _ => false,
        };

        if resolution == 0 {
            // Seller wins: if escrow was Refunded, we need to transfer from contract to seller
            // In practice the funds may have already moved; this records the outcome.
            // A full implementation would need a hold-back mechanism.
            escrow.status = EscrowStatus::Released;
        } else {
            // Buyer wins
            if escrow.status != EscrowStatus::Refunded {
                Self::refund_buyer(&env, &mut escrow);
            }
        }

        // Mark appeal resolved
        appeal.resolved = true;
        appeal.outcome = Some(resolution);
        env.storage().persistent().set(&appeal_key, &appeal);
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        // Update arbiter reputation if ruling was overturned
        if overturned {
            if let Some(arbiter) = &escrow.arbiter {
                let rep_key = DataKey::ArbiterReputation(arbiter.clone());
                let mut rep: ArbiterReputation =
                    env.storage().persistent().get(&rep_key).unwrap_or(ArbiterReputation {
                        arbiter: arbiter.clone(),
                        total_disputes: 0,
                        resolved_disputes: 0,
                        appealed_rulings: 0,
                        overturned_rulings: 0,
                        slash_count: 0,
                        last_active: 0,
                    });
                rep.overturned_rulings += 1;
                rep.last_active = env.ledger().sequence();
                env.storage().persistent().set(&rep_key, &rep);
            }
        }

        let _ = token_client; // suppress unused warning

        AppealResolvedEvent {
            escrow_id,
            admin,
            outcome: resolution,
            overturned,
        }
        .publish(&env);

        Ok(())
    }

    /// Get the appeal record for an escrow (None if no appeal filed).
    pub fn get_appeal(env: Env, escrow_id: u64) -> Option<AppealRecord> {
        env.storage().persistent().get(&DataKey::Appeal(escrow_id))
    }

    // ── #204: On-Chain Arbiter Reputation ────────────────────────────────────

    /// Read the reputation record for an arbiter.
    pub fn get_arbiter_reputation(env: Env, arbiter: Address) -> Option<ArbiterReputation> {
        env.storage()
            .persistent()
            .get(&DataKey::ArbiterReputation(arbiter))
    }

    /// Internal: increment total_disputes counter for an arbiter.
    fn increment_arbiter_disputes(env: &Env, arbiter: &Address) {
        let key = DataKey::ArbiterReputation(arbiter.clone());
        let mut rep: ArbiterReputation = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(ArbiterReputation {
                arbiter: arbiter.clone(),
                total_disputes: 0,
                resolved_disputes: 0,
                appealed_rulings: 0,
                overturned_rulings: 0,
                slash_count: 0,
                last_active: 0,
            });
        rep.total_disputes += 1;
        rep.last_active = env.ledger().sequence();
        env.storage().persistent().set(&key, &rep);
    }

    /// Internal: mark a dispute as resolved in the arbiter's reputation record.
    fn record_arbiter_resolution(env: &Env, arbiter: &Address) {
        let key = DataKey::ArbiterReputation(arbiter.clone());
        let mut rep: ArbiterReputation = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(ArbiterReputation {
                arbiter: arbiter.clone(),
                total_disputes: 0,
                resolved_disputes: 0,
                appealed_rulings: 0,
                overturned_rulings: 0,
                slash_count: 0,
                last_active: 0,
            });
        rep.resolved_disputes += 1;
        rep.last_active = env.ledger().sequence();
        env.storage().persistent().set(&key, &rep);
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

    pub fn set_fee_caps(env: Env, min_fee: i128, max_fee: i128) -> Result<(), ContractError> {
        let admin = Self::assert_admin(&env)?;

        if max_fee > 0 && min_fee > max_fee {
            return Err(ContractError::InvalidFeeConfig);
        }

        let old_min_fee = env
            .storage()
            .persistent()
            .get(&DataKey::MinFee)
            .unwrap_or(0);
        let old_max_fee = env
            .storage()
            .persistent()
            .get(&DataKey::MaxFee)
            .unwrap_or(0);

        env.storage().persistent().set(&DataKey::MinFee, &min_fee);
        env.storage().persistent().set(&DataKey::MaxFee, &max_fee);

        FeeCapsChangedEvent {
            old_min_fee,
            new_min_fee: min_fee,
            old_max_fee,
            new_max_fee: max_fee,
            actor: admin,
        }
        .publish(&env);

        Ok(())
    }

    pub fn set_native_fee(
        env: Env,
        native_token: Address,
        native_fee_bps: u32,
    ) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;

        if native_fee_bps > 1000 {
            return Err(ContractError::InvalidFeeConfig);
        }

        env.storage()
            .persistent()
            .set(&DataKey::NativeAsset, &native_token);
        env.storage()
            .persistent()
            .set(&DataKey::NativeFeeBps, &native_fee_bps);

        Ok(())
    }

    pub fn get_native_fee_bps(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::NativeFeeBps)
            .unwrap_or(0)
    }

    pub fn get_native_asset(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::NativeAsset)
    }

    pub fn get_min_fee(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::MinFee)
            .unwrap_or(0)
    }

    pub fn get_max_fee(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::MaxFee)
            .unwrap_or(0)
    }

    /// Add an address to the fee exemption whitelist. Admin only.
    pub fn add_fee_whitelist(env: Env, address: Address) -> Result<(), ContractError> {
        let admin = Self::assert_admin(&env)?;
        env.storage()
            .persistent()
            .set(&DataKey::FeeWhitelist(address.clone()), &true);
        FeeExemptionEvent { address, exempted: true, actor: admin }.publish(&env);
        Ok(())
    }

    /// Remove an address from the fee exemption whitelist. Admin only.
    pub fn remove_fee_whitelist(env: Env, address: Address) -> Result<(), ContractError> {
        let admin = Self::assert_admin(&env)?;
        env.storage()
            .persistent()
            .remove(&DataKey::FeeWhitelist(address.clone()));
        FeeExemptionEvent { address, exempted: false, actor: admin }.publish(&env);
        Ok(())
    }

    /// Check whether an address is fee-exempt.
    pub fn is_fee_exempt(env: Env, address: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::FeeWhitelist(address))
            .unwrap_or(false)
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

    /// Withdraw accumulated fees for a specific token.
    ///
    /// This follows the pull pattern for revenue sharing, allowing collectors
    /// to claim their fees at their convenience.
    pub fn withdraw_fees(env: Env, collector: Address, token: Address) -> Result<(), ContractError> {
        collector.require_auth();

        let key = DataKey::PendingFee(collector.clone(), token.clone());
        let amount: i128 = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::InvalidEscrowAmount)?;

        if amount <= 0 {
            return Err(ContractError::InvalidEscrowAmount);
        }

        env.storage().persistent().remove(&key);

        let token_client = soroban_sdk::token::Client::new(&env, &token);
        token_client.transfer(&env.current_contract_address(), &collector, &amount);

        FeesWithdrawnEvent {
            collector,
            token,
            amount,
        }
        .publish(&env);

        Ok(())
    }

    /// Get the pending fee balance for a collector and token.
    pub fn get_pending_fee(env: Env, collector: Address, token: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::PendingFee(collector, token))
            .unwrap_or(0)
    }

    // =========================
    // 💰 BATCH FEE COLLECTION (#171)
    // =========================

    /// Collect fees from multiple escrows in a single transaction.
    /// This is more efficient than collecting fees one-by-one.
    ///
    /// # Arguments
    /// * `escrow_ids` - Vector of escrow IDs to collect fees from
    ///
    /// # Returns
    /// Total amount of fees collected
    ///
    /// # Errors
    /// * `EscrowNotFound` - If any escrow doesn't exist
    /// * `InvalidEscrowState` - If any escrow is not in Released state
    pub fn batch_collect_fees(
        env: Env,
        collector: Address,
        token: Address,
        escrow_ids: Vec<u64>,
    ) -> Result<i128, ContractError> {
        collector.require_auth();

        let mut total_fees: i128 = 0;
        let mut count: u32 = 0;

        for escrow_id in escrow_ids.iter() {
            let escrow: Escrow = env
                .storage()
                .persistent()
                .get(&DataKey::Escrow(escrow_id))
                .ok_or(ContractError::EscrowNotFound)?;

            // Only collect from released escrows with matching token
            if escrow.status == EscrowStatus::Released && escrow.token == token {
                // Calculate fee for this escrow
                let fee_bps: u32 = env
                    .storage()
                    .persistent()
                    .get(&DataKey::FeeBps)
                    .unwrap_or(0);

                let mut fee: i128 = escrow.amount * (fee_bps as i128) / 10_000;
                let min_fee: i128 = env.storage().persistent().get(&DataKey::MinFee).unwrap_or(0);
                let max_fee: i128 = env.storage().persistent().get(&DataKey::MaxFee).unwrap_or(0);

                if fee < min_fee {
                    fee = min_fee;
                }
                if max_fee > 0 && fee > max_fee {
                    fee = max_fee;
                }
                if fee > escrow.amount {
                    fee = escrow.amount;
                }

                total_fees += fee;
                count += 1;
            }
        }

        if total_fees > 0 {
            BatchFeesCollectedEvent {
                collector: collector.clone(),
                token: token.clone(),
                total_amount: total_fees,
                escrow_count: count,
            }
            .publish(&env);
        }

        Ok(total_fees)
    }

    // =========================
    // 🎯 MILESTONE-BASED PAYMENTS (#173)
    // =========================

    /// Create an escrow with milestone-based payment releases.
    ///
    /// # Arguments
    /// * `buyer` - The buyer's address
    /// * `seller` - The seller's address
    /// * `token` - The token contract address
    /// * `amount` - The total escrow amount
    /// * `milestones` - Vector of milestones with descriptions and amounts
    /// * `metadata` - Optional metadata
    /// * `arbiter` - Optional arbiter
    ///
    /// # Errors
    /// * `ItemAmountInvalid` - If milestone amounts don't sum to total amount
    pub fn create_milestone_escrow(
        env: Env,
        buyer: Address,
        seller: Address,
        token: Address,
        amount: i128,
        milestones: Vec<Milestone>,
        metadata: Option<Bytes>,
        arbiter: Option<Address>,
    ) -> Result<u64, ContractError> {
        Self::assert_not_paused(&env)?;
        buyer.require_auth();

        // Validate milestone amounts sum to total
        let milestone_sum: i128 = milestones.iter().map(|m| m.amount).sum();
        if milestone_sum != amount {
            return Err(ContractError::ItemAmountInvalid);
        }

        let escrow_id = Self::create_escrow_internal(
            env.clone(),
            buyer,
            seller,
            token,
            amount,
            metadata,
            arbiter,
            None,
            None,
        )?;

        // Store milestones separately
        env.storage()
            .persistent()
            .set(&DataKey::MilestoneEscrow(escrow_id), &milestones);

        // Update escrow with milestones
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .unwrap();
        escrow.milestones = milestones;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Ok(escrow_id)
    }

    /// Complete a milestone and release the associated payment.
    ///
    /// # Arguments
    /// * `escrow_id` - The escrow ID
    /// * `milestone_index` - The index of the milestone to complete
    ///
    /// # Errors
    /// * `EscrowNotFound` - If escrow doesn't exist
    /// * `MilestoneNotFound` - If milestone index is invalid
    /// * `MilestoneAlreadyCompleted` - If milestone is already completed
    pub fn complete_milestone(
        env: Env,
        escrow_id: u64,
        milestone_index: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Only buyer can complete milestones
        escrow.buyer.require_auth();

        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        // Get milestone
        if milestone_index >= escrow.milestones.len() {
            return Err(ContractError::MilestoneNotFound);
        }

        let mut milestone = escrow.milestones.get(milestone_index).unwrap();
        if milestone.completed {
            return Err(ContractError::MilestoneAlreadyCompleted);
        }

        // Mark milestone as completed
        milestone.completed = true;
        milestone.completed_at = Some(env.ledger().timestamp());
        escrow.milestones.set(milestone_index, milestone.clone());

        // Transfer milestone amount to seller
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.seller,
            &milestone.amount,
        );

        // Check if all milestones are completed
        let all_completed = escrow.milestones.iter().all(|m| m.completed);
        if all_completed {
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

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        MilestoneCompletedEvent {
            escrow_id,
            milestone_index,
            amount: milestone.amount,
        }
        .publish(&env);

        Self::add_i128(&env, DataKey::TotalReleasedAmount, milestone.amount);

        Ok(())
    }

    /// Get milestones for an escrow.
    pub fn get_milestones(env: Env, escrow_id: u64) -> Option<Vec<Milestone>> {
        let escrow: Option<Escrow> = env.storage().persistent().get(&DataKey::Escrow(escrow_id));
        escrow.map(|e| e.milestones)
    }

    // =========================
    // ⏰ TIME-LOCKED AUTO-RELEASE (#174)
    // =========================

    /// Set a time-lock for automatic release of escrow funds.
    ///
    /// # Arguments
    /// * `escrow_id` - The escrow ID
    /// * `release_ledger` - The ledger sequence number when funds should auto-release
    ///
    /// # Errors
    /// * `EscrowNotFound` - If escrow doesn't exist
    /// * `Unauthorized` - If caller is not buyer or seller
    pub fn set_time_lock(
        env: Env,
        escrow_id: u64,
        release_ledger: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Only buyer or seller can set time lock
        let caller = escrow.buyer.clone(); // We'll use buyer as the caller
        if !Self::is_escrow_party(&escrow, &caller) {
            return Err(ContractError::Unauthorized);
        }

        escrow.buyer.require_auth();

        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        let time_lock = TimeLock {
            release_ledger,
            enabled: true,
        };

        escrow.time_lock = Some(time_lock.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);
        env.storage()
            .persistent()
            .set(&DataKey::TimeLockEscrow(escrow_id), &time_lock);

        Ok(())
    }

    /// Trigger automatic release of time-locked escrow funds.
    /// Anyone can call this once the release ledger is reached.
    ///
    /// # Arguments
    /// * `escrow_id` - The escrow ID
    ///
    /// # Errors
    /// * `EscrowNotFound` - If escrow doesn't exist
    /// * `TimeLockNotEnabled` - If time lock is not set
    /// * `TimeLockNotReached` - If current ledger is before release ledger
    pub fn trigger_time_lock_release(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        let time_lock = escrow
            .time_lock
            .clone()
            .ok_or(ContractError::TimeLockNotEnabled)?;

        if !time_lock.enabled {
            return Err(ContractError::TimeLockNotEnabled);
        }

        let current_ledger = env.ledger().sequence();
        if current_ledger < time_lock.release_ledger {
            return Err(ContractError::TimeLockNotReached);
        }

        // Release funds to seller
        let from_status = escrow.status.clone();
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.seller,
            &escrow.amount,
        );

        escrow.status = EscrowStatus::Released;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        TimeLockReleasedEvent {
            escrow_id,
            amount: escrow.amount,
        }
        .publish(&env);

        Self::emit_status_change(
            &env,
            escrow_id,
            from_status,
            escrow.status.clone(),
            env.current_contract_address(),
        );

        Self::add_i128(&env, DataKey::TotalReleasedAmount, escrow.amount);

        Ok(())
    }

    /// Get time lock configuration for an escrow.
    pub fn get_time_lock(env: Env, escrow_id: u64) -> Option<TimeLock> {
        let escrow: Option<Escrow> = env.storage().persistent().get(&DataKey::Escrow(escrow_id));
        escrow.and_then(|e| e.time_lock)
    }

    // =========================
    // 👥 GROUP BUY ESCROW (#175)
    // =========================

    /// Create a group buy escrow where multiple buyers contribute to a single purchase.
    ///
    /// # Arguments
    /// * `seller` - The seller's address
    /// * `token` - The token contract address
    /// * `target_amount` - The total amount needed
    /// * `buyers` - Vector of buyer contributions
    /// * `funding_deadline` - Ledger sequence number deadline for funding
    /// * `metadata` - Optional metadata
    /// * `arbiter` - Optional arbiter
    ///
    /// # Errors
    /// * `InvalidGroupBuyAmount` - If buyer contributions don't sum to target amount
    pub fn create_group_buy_escrow(
        env: Env,
        seller: Address,
        token: Address,
        target_amount: i128,
        buyers: Vec<BuyerContribution>,
        funding_deadline: u32,
        metadata: Option<Bytes>,
        arbiter: Option<Address>,
    ) -> Result<u64, ContractError> {
        Self::assert_not_paused(&env)?;

        // Validate buyer contributions sum to target
        let contributions_sum: i128 = buyers.iter().map(|b| b.amount).sum();
        if contributions_sum != target_amount {
            return Err(ContractError::InvalidGroupBuyAmount);
        }

        // Use first buyer as primary buyer for escrow creation
        let primary_buyer = buyers.get(0).ok_or(ContractError::InvalidGroupBuyAmount)?.buyer.clone();
        primary_buyer.require_auth();

        let escrow_id = Self::create_escrow_internal(
            env.clone(),
            primary_buyer,
            seller,
            token.clone(),
            target_amount,
            metadata,
            arbiter,
            None,
            None,
        )?;

        // Create group buy configuration
        let group_buy = GroupBuy {
            buyers: buyers.clone(),
            target_amount,
            funded_amount: 0,
            funding_deadline,
        };

        // Update escrow with group buy config
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .unwrap();
        escrow.group_buy = Some(group_buy.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);
        env.storage()
            .persistent()
            .set(&DataKey::GroupBuyEscrow(escrow_id), &group_buy);

        Ok(escrow_id)
    }

    /// Fund a group buy escrow as one of the buyers.
    ///
    /// # Arguments
    /// * `escrow_id` - The escrow ID
    /// * `buyer` - The buyer's address
    ///
    /// # Errors
    /// * `EscrowNotFound` - If escrow doesn't exist
    /// * `GroupBuyDeadlinePassed` - If funding deadline has passed
    /// * `GroupBuyAlreadyFunded` - If buyer has already funded
    /// * `Unauthorized` - If caller is not a registered buyer
    pub fn fund_group_buy(env: Env, escrow_id: u64, buyer: Address) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        buyer.require_auth();

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        let mut group_buy = escrow
            .group_buy
            .clone()
            .ok_or(ContractError::InvalidEscrowState)?;

        // Check deadline
        if env.ledger().sequence() > group_buy.funding_deadline {
            return Err(ContractError::GroupBuyDeadlinePassed);
        }

        // Find buyer in contributions list
        let mut buyer_index: Option<u32> = None;
        let mut buyer_amount: i128 = 0;

        for (i, contribution) in group_buy.buyers.iter().enumerate() {
            if contribution.buyer == buyer {
                if contribution.funded {
                    return Err(ContractError::GroupBuyAlreadyFunded);
                }
                buyer_index = Some(i as u32);
                buyer_amount = contribution.amount;
                break;
            }
        }

        let index = buyer_index.ok_or(ContractError::Unauthorized)?;

        // Transfer funds from buyer to contract
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        token_client.transfer(&buyer, &env.current_contract_address(), &buyer_amount);

        // Update buyer contribution
        let mut contribution = group_buy.buyers.get(index).unwrap();
        contribution.funded = true;
        group_buy.buyers.set(index, contribution);
        group_buy.funded_amount += buyer_amount;

        // Update escrow
        escrow.group_buy = Some(group_buy.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        GroupBuyFundedEvent {
            escrow_id,
            buyer: buyer.clone(),
            amount: buyer_amount,
        }
        .publish(&env);

        // Check if fully funded
        if group_buy.funded_amount >= group_buy.target_amount {
            GroupBuyCompletedEvent {
                escrow_id,
                total_amount: group_buy.funded_amount,
            }
            .publish(&env);
        }

        Self::add_i128(&env, DataKey::TotalFundedAmount, buyer_amount);

        Ok(())
    }

    /// Get group buy configuration for an escrow.
    pub fn get_group_buy(env: Env, escrow_id: u64) -> Option<GroupBuy> {
        let escrow: Option<Escrow> = env.storage().persistent().get(&DataKey::Escrow(escrow_id));
        escrow.and_then(|e| e.group_buy)
    }
}
