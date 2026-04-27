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
    AdminTransferredEvent, BulkEscrowCreatedEvent, BulkEscrowRequest, CancellationProposedEvent,
    CounterEvidenceSubmittedEvent, DataKey, DeliveryVerifiedEvent, Escrow, EscrowCreatedEvent,
    EscrowExpiredEvent, EscrowItem, EscrowStatus, FeeCapsChangedEvent, FeeChangedEvent,
    FeeCollectedEvent, FeeExemptionEvent, FeesWithdrawnEvent, FundsReleasedEvent,
    MetadataVisibility, RefundHistoryEntry, RefundReason, RefundRequest, RefundRequestedEvent,
    RefundStatus, StatusChangeEvent, MAX_ITEMS_PER_ESCROW, MAX_METADATA_SIZE,
    UNFUNDED_EXPIRY_LEDGERS,
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

    fn validate_address(env: &Env, address: &Address) -> Result<(), ContractError> {
        // Check if address is a zero address by serializing and checking if all bytes are zero
        let xdr_bytes = address.to_xdr(env);
        
        // Check if any byte is non-zero
        let is_zero = xdr_bytes.iter().all(|&byte| byte == 0);
        
        if is_zero {
            return Err(ContractError::ZeroAddress);
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

    fn get_volume_tiers_config(env: &Env) -> VolumeTierConfig {
        env.storage()
            .persistent()
            .get(&DataKey::VolumeTiers)
            .unwrap_or(VolumeTierConfig::default())
    }

    fn should_reset_volume(env: &Env, last_reset: u32) -> bool {
        let current_ledger = env.ledger().sequence();
        current_ledger.saturating_sub(last_reset) >= 1_576_800
    }

    fn calc_buyer_volume(env: &Env, buyer: &Address) -> i128 {
        let config = Self::get_volume_tiers_config(env);
        if Self::should_reset_volume(env, config.reset_ledger) {
            return 0;
        }
        env.storage()
            .persistent()
            .get(&DataKey::BuyerVolume(buyer.clone()))
            .unwrap_or(0)
    }

    fn calculate_buyer_tier(env: &Env, buyer: &Address) -> u8 {
        let volume = Self::calc_buyer_volume(env, buyer);
        let config = Self::get_volume_tiers_config(env);
        config.get_tier(volume)
    }

    fn update_buyer_volume(env: &Env, buyer: &Address, amount: i128) {
        let mut config = Self::get_volume_tiers_config(env);

        if Self::should_reset_volume(env, config.reset_ledger) {
            config.reset_ledger = env.ledger().sequence();
            env.storage().persistent().set(&DataKey::VolumeTiers, &config);
        }

        let current: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::BuyerVolume(buyer.clone()))
            .unwrap_or(0);

        let new_volume = current.saturating_add(amount);

        env.storage()
            .persistent()
            .set(&DataKey::BuyerVolume(buyer.clone()), &new_volume);

        let _tier = config.get_tier(new_volume);

        VolumeUpdatedEvent {
            buyer: buyer.clone(),
            added_amount: amount,
            new_volume,
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
    /// - `admin` and `fee_collector` must not be zero addresses
    ///
    /// # Events
    /// Emits no events during initialization
    ///
    /// # Errors
    /// * `ZeroAddress` - If admin or fee_collector is a zero address
    pub fn initialize(
        env: Env,
        admin: Address,
        fee_collector: Address,
        fee_bps: u32,
        min_fee: i128,
        max_fee: i128,
    ) -> Result<(), ContractError> {
        admin.require_auth();

        // Validate that admin and fee_collector are not zero addresses
        Self::validate_address(&env, &admin)?;
        Self::validate_address(&env, &fee_collector)?;

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

        let default_tiers = VolumeTierConfig {
            tier_1_threshold: 100_000,
            tier_2_threshold: 1_000_000,
            tier_3_threshold: 10_000_000,
            tier_1_discount_bps: 100,
            tier_2_discount_bps: 250,
            tier_3_discount_bps: 500,
            reset_ledger: env.ledger().sequence(),
        };
        env.storage().persistent().set(&DataKey::VolumeTiers, &default_tiers);
        Ok(())
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

        // Validate that addresses are not zero addresses
        Self::validate_address(&env, &buyer)?;
        Self::validate_address(&env, &seller)?;
        Self::validate_address(&env, &token)?;

        // Validate arbiter if provided
        if let Some(ref arb) = arbiter {
            Self::validate_address(&env, arb)?;
        }

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
        tracking_id: Option<Bytes>,
    ) -> Result<u64, ContractError> {
        Self::assert_not_paused(&env)?;
        buyer.require_auth();

        Self::create_escrow_internal(
            env,
            buyer,
            seller,
            token,
            amount,
            metadata,
            arbiter,
            items,
            tracking_id,
        )
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
                None,
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

    pub fn get_escrow_metadata(
        env: Env,
        escrow_id: u64,
        caller: Address,
    ) -> Result<Option<Bytes>, ContractError> {
        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        let visibility: MetadataVisibility = env
            .storage()
            .persistent()
            .get(&DataKey::MetadataVisibility(escrow_id))
            .unwrap_or(MetadataVisibility::Private);

        if visibility == MetadataVisibility::Private {
            let is_party = caller == escrow.buyer
                || caller == escrow.seller
                || escrow.arbiter.as_ref().is_some_and(|a| *a == caller);
            let is_admin = env
                .storage()
                .persistent()
                .get::<DataKey, Address>(&DataKey::Admin)
                .is_some_and(|a| a == caller);
            if !is_party && !is_admin {
                return Err(ContractError::MetadataAccessDenied);
            }
            caller.require_auth();
        }

        Ok(escrow.metadata)
    }

    /// Set metadata visibility. Only the buyer may call this.
    /// Defaults to `Private`; set to `Public` to allow anyone to read.
    pub fn set_metadata_visibility(
        env: Env,
        escrow_id: u64,
        visibility: MetadataVisibility,
    ) -> Result<(), ContractError> {
        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        escrow.buyer.require_auth();

        env.storage()
            .persistent()
            .set(&DataKey::MetadataVisibility(escrow_id), &visibility);

        Ok(())
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

        let tracking_id = escrow
            .tracking_id
            .clone()
            .ok_or(ContractError::Unauthorized)?;

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
        if fee > escrow.amount {
            fee = escrow.amount;
        }

        let seller_amount = escrow.amount - fee;
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.seller,
            &seller_amount,
        );

        if fee > 0 {
            let fee_collector: Address = env
                .storage()
                .persistent()
                .get(&DataKey::FeeCollector)
                .ok_or(ContractError::InvalidFeeConfig)?;
            Self::add_pending_fee(&env, fee_collector.clone(), escrow.token.clone(), fee);
            Self::add_i128(&env, DataKey::TotalFeesCollected, fee);
            FeeCollectedEvent {
                escrow_id,
                fee_collector,
                fee,
            }
            .publish(&env);
        }

        escrow.status = EscrowStatus::Released;
        escrow.cancellation_proposer = None;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        FundsReleasedEvent {
            escrow_id,
            amount: escrow.amount,
            fee,
        }
        .publish(&env);
        DeliveryVerifiedEvent {
            escrow_id,
            tracking_id,
        }
        .publish(&env);
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

    /// Allows a third party (like an Auction contract) to fund an escrow on behalf of the buyer.
    /// The funds are drawn from the `funder` address instead of the `buyer`.
    pub fn fund_escrow_by(env: Env, escrow_id: u64, funder: Address) -> Result<(), ContractError> {
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

        // 3. Enforce funder authorization
        funder.require_auth();

        // 4. Transfer funds from funder into the contract
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        #[allow(clippy::needless_borrows_for_generic_args)]
        token_client.transfer(
            &funder,
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

        // 4. Calculate fee with fixed single-path logic
        // Step 4a: Check whitelist first (100% exemption)
        let is_whitelisted: bool = env
        // 4. Calculate fee: amount * fee_bps / 10_000 (integer floor division)
        // Whitelisted buyers (partners/internal) pay zero fees.
        let is_exempt: bool = env
            .storage()
            .persistent()
            .get(&DataKey::FeeWhitelist(escrow.buyer.clone()))
            .unwrap_or(false);

        let fee = if is_whitelisted {
            0
        } else {
            // Step 4b: Get base fee bps
            let base_fee_bps: u32 = env.storage().persistent().get(&DataKey::FeeBps).unwrap_or(0);

            // Step 4c: Check for native XLM special rate
            let effective_fee_bps = if let Some(native_asset) = env
                .storage()
                .persistent()
                .get::<DataKey, Address>(&DataKey::NativeAsset)
            {
                if escrow.token == native_asset {
                    env.storage()
                        .persistent()
                        .get(&DataKey::NativeFeeBps)
                        .unwrap_or(base_fee_bps)
                } else {
                    base_fee_bps
                }
            } else {
                base_fee_bps
            };

            // Step 4d: Apply volume discount (capped at 50% = 500 bps)
            let tier = Self::calculate_buyer_tier(&env, &escrow.buyer);
            let tiers_config = Self::get_volume_tiers_config(&env);
            let volume_discount = tiers_config.get_discount_bps(tier);
            let actual_discount = volume_discount.min(500);
            let discounted_fee_bps = effective_fee_bps.saturating_sub(actual_discount);

            // Step 4e: Calculate fee (overflow-safe)
            let mut calculated_fee = (escrow.amount / 10_000).saturating_mul(discounted_fee_bps as i128);
            let remainder = escrow.amount % 10_000;
            calculated_fee = calculated_fee.saturating_add((remainder * discounted_fee_bps as i128) / 10_000);

            // Step 4f: Apply min/max caps
            let min_fee: i128 = env.storage().persistent().get(&DataKey::MinFee).unwrap_or(0);
            let max_fee: i128 = env.storage().persistent().get(&DataKey::MaxFee).unwrap_or(0);
            calculated_fee = calculated_fee.max(min_fee);
            if max_fee > 0 {
                calculated_fee = calculated_fee.min(max_fee);
            }

            // Step 4g: Cap at escrow amount
            calculated_fee.min(escrow.amount)
        };

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

        let mut fee: i128 = if is_exempt {
            0
        } else {
            escrow.amount * (fee_bps as i128) / 10_000
        };

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

        if !is_exempt {
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
        }
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
                    .unwrap(),
                fee,
            }
            .publish(&env);
        }

        // 7. Update escrow status to Released
        escrow.status = EscrowStatus::Released;
        escrow.cancellation_proposer = None;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        // 8. Emit FundsReleasedEvent
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

        // 9. Update buyer volume for tier-based discounts
        Self::update_buyer_volume(&env, &escrow.buyer, escrow.amount);

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

        Self::emit_status_change(&env, escrow_id, from_status, escrow.status.clone(), actor);

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
        FeeExemptionEvent {
            address,
            exempted: true,
            actor: admin,
        }
        .publish(&env);
        Ok(())
    }

    /// Remove an address from the fee exemption whitelist. Admin only.
    pub fn remove_fee_whitelist(env: Env, address: Address) -> Result<(), ContractError> {
        let admin = Self::assert_admin(&env)?;
        env.storage()
            .persistent()
            .remove(&DataKey::FeeWhitelist(address.clone()));
        FeeExemptionEvent {
            address,
            exempted: false,
            actor: admin,
        }
        .publish(&env);
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
    pub fn withdraw_fees(
        env: Env,
        collector: Address,
        token: Address,
    ) -> Result<(), ContractError> {
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

    /// Get buyer total volume (for display/debugging)
    pub fn buyer_volume(env: Env, buyer: Address) -> i128 {
        let config = Self::get_volume_tiers_config(&env);
        if Self::should_reset_volume(&env, config.reset_ledger) {
            return 0;
        }
        env.storage()
            .persistent()
            .get(&DataKey::BuyerVolume(buyer))
            .unwrap_or(0)
    }

    /// Get buyer current tier (0-3) based on volume
    pub fn buyer_tier(env: Env, buyer: Address) -> u32 {
        let volume = Self::buyer_volume(env.clone(), buyer);
        if volume == 0 {
            return 0;
        }
        let config = Self::get_volume_tiers_config(&env);
        config.get_tier(volume) as u32
    }

    /// Get volume tier configuration
    pub fn volume_tiers(env: Env) -> VolumeTierConfig {
        Self::get_volume_tiers_config(&env)
    }
}
const federationQuerySchema = z.object({
  q: z.string().min(1, "q is required"),
  type: z.enum(["name", "id", "txid", "forward"] as const, {
    error: () => ({ message: "type must be one of: name, id, txid, forward" }),
  }),
});if (!parsed.success) {
  return res.status(400).json({
    detail: parsed.error?.issues?.[0]?.message ?? "Invalid input",
  });
}pub use types::{
    AdminTransferredEvent,
    BulkEscrowCreatedEvent,
    BulkEscrowRequest,
    CancellationProposedEvent,
    CounterEvidenceSubmittedEvent,
    DataKey,
    Escrow,
    EscrowCreatedEvent,
    EscrowExpiredEvent,
    EscrowItem,
    EscrowStatus,
    FeeCapsChangedEvent,
    FeeChangedEvent,
    FeeCollectedEvent,
    FeeExemptionEvent,
    FeesWithdrawnEvent,
    FundsReleasedEvent,
    RefundHistoryEntry,
    RefundReason,
    RefundRequest,
    RefundRequestedEvent,
    RefundStatus,
    StatusChangeEvent,
    MAX_ITEMS_PER_ESCROW,
    MAX_METADATA_SIZE,
    UNFUNDED_EXPIRY_LEDGERS,
};
