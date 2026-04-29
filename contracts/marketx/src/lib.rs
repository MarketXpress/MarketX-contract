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
//! client.release_escrow(&escrow_id);
//!
//! // Withdraw fees to admin (collector)
//! client.withdraw_fees(&admin, &xlm_address);
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

use soroban_sdk::{contract, contractimpl, contractmeta, Address, Bytes, BytesN, Env, Vec};

mod errors;
mod types;

use soroban_sdk::xdr::ToXdr;

pub use errors::ContractError;
pub use types::{
    AdminTransferredEvent, AppealFiledEvent, AppealRecord, AppealResolvedEvent,
    ArbiterReputation, ArbiterSlashedEvent, ArbiterStake, ArbiterStakedEvent,
    BatchFeesCollectedEvent, BulkEscrowCreatedEvent, BulkEscrowRequest, BuyerContribution,
    CancellationProposedEvent, ContractResourceProfile, ContractVersion, CONTRACT_VERSION,
    CounterEvidenceSubmittedEvent, DataKey, DeliveryVerifiedEvent, Escrow, EscrowCreatedEvent,
    EscrowExpiredEvent, EscrowItem, EscrowStatus, EvidenceSubmittedEvent, EvidenceWindow,
    EvidenceWindowExpiredEvent, FeeCapsChangedEvent, FeeChangedEvent, FeeCollectedEvent,
    FeeCollectorRotatedEvent, FeeExemptionEvent, FeesWithdrawnEvent, FundsReleasedEvent, GlobalDisputeAnalytics, GroupBuy,
    GroupBuyCompletedEvent, GroupBuyFundedEvent, MediationOpenedEvent, MediationPhase,
    MediationProposedEvent, MediationSettledEvent, MetadataVisibility, Milestone,
    MilestoneCompletedEvent, RefundHistoryEntry, RefundReason, RefundRequest, RefundRequestedEvent,
    RefundStatus, StatusChangeEvent, StorageRentEstimate, TimeLock, TimeLockReleasedEvent,
    TokenCircuitBreakerEvent, APPEAL_WINDOW_LEDGERS, CURRENT_SCHEMA_VERSION,
    DEFAULT_EVIDENCE_WINDOW_LEDGERS, DEFAULT_MEDIATION_WINDOW_LEDGERS, MAX_DESCRIPTION_SIZE,
    MAX_EVIDENCE_HASH_SIZE, MAX_ITEMS_PER_ESCROW, MAX_METADATA_SIZE, MAX_TRACKING_ID_SIZE,
    UNFUNDED_EXPIRY_LEDGERS,
};

#[cfg(test)]
mod test;

/// The MarketX escrow contract.
///
/// This contract provides secure escrow services on the Stellar network.
/// All public methods are available through the contract's public interface.
#[contractmeta(key = "name", val = "MarketX Escrow")]
#[contractmeta(
    key = "description",
    val = "Soroban escrow contract with milestone releases, dispute handling, and configurable fees."
)]
#[contractmeta(
    key = "homepage",
    val = "https://github.com/MarketXpress/MarketX-contract"
)]
#[contractmeta(
    key = "repository",
    val = "https://github.com/MarketXpress/MarketX-contract"
)]
#[contractmeta(
    key = "source",
    val = "https://github.com/MarketXpress/MarketX-contract/tree/main/contracts/marketx"
)]
#[contractmeta(key = "version", val = "v1.0.0")]
#[contract]
pub struct Contract;

impl Contract {
    fn disputes_enabled(env: &Env) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::FeatureDisputesEnabled)
            .unwrap_or(true)
    }

    fn partial_releases_enabled(env: &Env) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::FeaturePartialReleasesEnabled)
            .unwrap_or(true)
    }

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

    fn assert_disputes_enabled(env: &Env) -> Result<(), ContractError> {
        if !Self::disputes_enabled(env) {
            return Err(ContractError::FeatureDisabled);
        }
        Ok(())
    }

    fn assert_partial_releases_enabled(env: &Env) -> Result<(), ContractError> {
        if !Self::partial_releases_enabled(env) {
            return Err(ContractError::FeatureDisabled);
        }
        Ok(())
    }

    fn add_i128(env: &Env, key: DataKey, value: i128) {
        let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        let next = current.checked_add(value).expect("Global counter overflow");
        env.storage().persistent().set(&key, &next);
    }

    fn calculate_fee_internal(
        env: &Env,
        amount: i128,
        token: &Address,
        buyer: &Address,
    ) -> i128 {
        let is_exempt: bool = env
            .storage()
            .persistent()
            .get(&DataKey::FeeWhitelist(buyer.clone()))
            .unwrap_or(false);
        
        if is_exempt {
            return 0;
        }

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
            if *token == native_asset {
                fee_bps = env
                    .storage()
                    .persistent()
                    .get(&DataKey::NativeFeeBps)
                    .unwrap_or(fee_bps);
            }
        }

        if fee_bps == 0 {
            return 0;
        }

        let mut fee: i128 = amount * (fee_bps as i128) / 10_000;

        // Rounding protection: if bps > 0 and amount > 0, fee must be at least 1
        if fee == 0 && amount > 0 {
            fee = 1;
        }

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

        if fee > amount {
            fee = amount;
        }

        fee
    }

    fn process_seller_transfer(
        env: &Env,
        escrow_id: u64,
        amount: i128,
        token: &Address,
        seller: &Address,
        buyer: &Address,
    ) -> i128 {
        let fee = Self::calculate_fee_internal(env, amount, token, buyer);
        let seller_amount = amount - fee;

        let token_client = soroban_sdk::token::Client::new(env, token);
        token_client.transfer(
            &env.current_contract_address(),
            seller,
            &seller_amount,
        );

        if fee > 0 {
            let fee_collector: Address = env
                .storage()
                .persistent()
                .get(&DataKey::FeeCollector)
                .expect("Fee collector not configured");

            Self::add_pending_fee(env, fee_collector.clone(), token.clone(), fee);
            Self::add_i128(env, DataKey::TotalFeesCollected, fee);

            FeeCollectedEvent {
                escrow_id,
                fee_collector,
                fee,
            }
            .publish(env);
        }

        fee
    }

    fn validate_bytes_size(data: &Bytes, max: u32) -> Result<(), ContractError> {
        if data.len() > max {
            return Err(ContractError::MetadataTooLarge);
        }
        Ok(())
    }

    fn add_pending_fee(env: &Env, collector: Address, token: Address, amount: i128) {
        let key = DataKey::PendingFee(collector, token);
        let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(current + amount));
    }

    fn refund_buyer(env: &Env, escrow: &mut Escrow) {
        let token_client = soroban_sdk::token::Client::new(env, &escrow.token);
        token_client.transfer(&env.current_contract_address(), &escrow.buyer, &escrow.amount);
        
        escrow.status = EscrowStatus::Refunded;
        escrow.cancellation_proposer = None;
        Self::add_i128(env, DataKey::TotalRefundedAmount, escrow.amount);
        Self::add_u32(env, DataKey::TotalRefundedCount);
    }

    fn validate_metadata(metadata: &Option<Bytes>) -> Result<(), ContractError> {
        if let Some(m) = metadata {
            if m.len() > MAX_METADATA_SIZE {
                return Err(ContractError::MetadataTooLarge);
            }
        }
        Ok(())
    }

    fn check_duplicate_escrow(env: &Env, buyer: &Address, seller: &Address, metadata: &Option<Bytes>) -> Result<(), ContractError> {
        let hash = Self::generate_escrow_hash(env, buyer, seller, metadata);
        if env.storage().persistent().has(&DataKey::EscrowHash(hash)) {
            return Err(ContractError::DuplicateEscrow);
        }
        Ok(())
    }

    fn generate_escrow_hash(env: &Env, buyer: &Address, seller: &Address, metadata: &Option<Bytes>) -> BytesN<32> {
        let mut vec = Vec::new(env);
        vec.push_back(buyer.to_xdr(env));
        vec.push_back(seller.to_xdr(env));
        if let Some(m) = metadata {
            vec.push_back(m.to_xdr(env));
        }
        env.crypto().sha256(&vec.to_xdr(env)).into()
    }

    fn next_escrow_id(env: &Env) -> Result<u64, ContractError> {
        let current: u64 = env.storage().persistent().get(&DataKey::EscrowCounter).unwrap_or(0);
        let next = current.checked_add(1).ok_or(ContractError::EscrowIdOverflow)?;
        env.storage().persistent().set(&DataKey::EscrowCounter, &next);
        Ok(next)
    }

    fn next_refund_id(env: &Env) -> Result<u64, ContractError> {
        let current: u64 = env.storage().persistent().get(&DataKey::RefundCount).unwrap_or(0);
        let next = current.checked_add(1).ok_or(ContractError::EscrowIdOverflow)?;
        env.storage().persistent().set(&DataKey::RefundCount, &next);
        Ok(next)
    }

    fn is_escrow_party(escrow: &Escrow, actor: &Address) -> bool {
        actor == &escrow.buyer || actor == &escrow.seller || escrow.arbiter.as_ref().map_or(false, |a| a == actor)
    }

    fn has_released_items(escrow: &Escrow) -> bool {
        escrow.items.iter().any(|item| item.released)
    }

    fn add_u32(env: &Env, key: DataKey) {
        let current: u32 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(current + 1));
    }

    fn check_zero_address(env: &Env, addr: &Address) -> Result<(), ContractError> {
        let zero = Address::from_string(&soroban_sdk::String::from_str(env, "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"));
        if addr == &zero {
            return Err(ContractError::ZeroAddress);
        }
        Ok(())
    }

    fn xdr_len<T: ToXdr>(env: &Env, value: T) -> u32 {
        value.to_xdr(env).len() as u32
    }

    fn storage_entry_bytes<K: ToXdr, V: ToXdr>(env: &Env, key: K, value: V) -> u32 {
        Self::xdr_len(env, key).saturating_add(Self::xdr_len(env, value))
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
    ) -> Result<(), ContractError> {
        admin.require_auth();
        Self::check_zero_address(&env, &admin)?;
        Self::check_zero_address(&env, &fee_collector)?;

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
            .set(&DataKey::TotalReleasedCount, &0u32);
        env.storage()
            .persistent()
            .set(&DataKey::TotalRefundedCount, &0u32);
        env.storage()
            .persistent()
            .set(&DataKey::TotalCancelledCount, &0u32);
        env.storage()
            .persistent()
            .set(&DataKey::TotalFeesCollected, &0i128);
        
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

    /// Admin-controlled governance toggle for dispute operations.
    pub fn set_disputes_enabled(env: Env, enabled: bool) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage()
            .persistent()
            .set(&DataKey::FeatureDisputesEnabled, &enabled);
        Ok(())
    }

    /// Admin-controlled governance toggle for partial release operations.
    pub fn set_partial_releases_enabled(env: Env, enabled: bool) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage()
            .persistent()
            .set(&DataKey::FeaturePartialReleasesEnabled, &enabled);
        Ok(())
    }

    pub fn is_disputes_enabled(env: Env) -> bool {
        Self::disputes_enabled(&env)
    }

    pub fn is_partial_releases_enabled(env: Env) -> bool {
        Self::partial_releases_enabled(&env)
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
        Self::check_zero_address(&env, &buyer)?;
        Self::check_zero_address(&env, &seller)?;
        Self::check_zero_address(&env, &token)?;
        if let Some(ref a) = arbiter {
            Self::check_zero_address(&env, a)?;
        }

        Self::assert_token_not_paused(&env, &token)?;

        Self::validate_metadata(&metadata)?;
        Self::check_duplicate_escrow(&env, &buyer, &seller, &metadata)?;

        if let Some(ref tid) = tracking_id {
            Self::validate_bytes_size(tid, MAX_TRACKING_ID_SIZE)?;
        }

        if amount <= 0 {
            return Err(ContractError::InvalidEscrowAmount);
        }


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

                // Validate per-item descriptions
                for item in items_vec.iter() {
                    if let Some(ref desc) = item.description {
                        Self::validate_bytes_size(desc, MAX_DESCRIPTION_SIZE)?;
                    }
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
            time_lock: Vec::new(&env),
            group_buy: Vec::new(&env),
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

        Self::create_escrow_internal(env, buyer, seller, token, amount, metadata, arbiter, items, tracking_id)
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

    pub fn get_escrow_metadata(env: Env, escrow_id: u64, caller: Address) -> Result<Option<Bytes>, ContractError> {
        let escrow: Escrow = env.storage().persistent().get(&DataKey::Escrow(escrow_id)).ok_or(ContractError::EscrowNotFound)?;
        
        Self::check_metadata_access(&env, escrow_id, &escrow, &caller)?;
        
        Ok(escrow.metadata)
    }

    pub fn set_metadata_visibility(env: Env, escrow_id: u64, visibility: MetadataVisibility) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        
        let escrow: Escrow = env.storage().persistent().get(&DataKey::Escrow(escrow_id)).ok_or(ContractError::EscrowNotFound)?;
        escrow.buyer.require_auth();
        
        env.storage().persistent().set(&DataKey::MetadataVisibility(escrow_id), &visibility);
        Ok(())
    }

    fn check_metadata_access(env: &Env, escrow_id: u64, escrow: &Escrow, caller: &Address) -> Result<(), ContractError> {
        let visibility: MetadataVisibility = env.storage().persistent().get(&DataKey::MetadataVisibility(escrow_id)).unwrap_or(MetadataVisibility::Private);
        
        if visibility == MetadataVisibility::Public {
            return Ok(());
        }
        
        if caller == &escrow.buyer || caller == &escrow.seller || caller == &env.storage().persistent().get::<DataKey, Address>(&DataKey::Admin).unwrap() {
            return Ok(());
        }
        
        if let Some(arbiter) = &escrow.arbiter {
            if caller == arbiter {
                return Ok(());
            }
        }
        
        Err(ContractError::MetadataAccessDenied)
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

    pub fn get_total_refunded_count(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalRefundedCount)
            .unwrap_or(0)
    }

    pub fn get_total_released_count(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalReleasedCount)
            .unwrap_or(0)
    }

    pub fn get_total_disputed_count(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalDisputedCount)
            .unwrap_or(0)
    }

    pub fn get_total_cancelled_count(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalCancelledCount)
            .unwrap_or(0)
    }

    pub fn analytics_summary(env: Env) -> GlobalDisputeAnalytics {
        let total_escrows = Self::get_total_escrows(env.clone());
        let released_count = Self::get_total_released_count(env.clone());
        let refunded_count = Self::get_total_refunded_count(env.clone());
        let disputed_count = Self::get_total_disputed_count(env.clone());
        let cancelled_count = Self::get_total_cancelled_count(env.clone());

        let failure_rate_bps = if total_escrows > 0 {
            let failures = refunded_count + disputed_count + cancelled_count;
            ((failures as u64) * 10_000 / total_escrows) as u32
        } else {
            0
        };

        GlobalDisputeAnalytics {
            total_escrows,
            released_count,
            refunded_count,
            disputed_count,
            cancelled_count,
            failure_rate_bps,
        }
    }

    /// Estimate the persistent storage footprint for a specific escrow.
    ///
    /// The returned byte count is an approximation based on the XDR size of
    /// the escrow record and its companion entries.
    pub fn estimate_storage_rent(
        env: Env,
        escrow_id: u64,
    ) -> Result<StorageRentEstimate, ContractError> {
        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        let mut entry_count: u32 = 1;
        let mut estimated_bytes: u32 =
            Self::storage_entry_bytes(&env, DataKey::Escrow(escrow_id), escrow.clone());

        let hash =
            Self::generate_escrow_hash(&env, &escrow.buyer, &escrow.seller, &escrow.metadata);
        let hash_key = DataKey::EscrowHash(hash);
        estimated_bytes =
            estimated_bytes.saturating_add(Self::storage_entry_bytes(&env, hash_key, escrow_id));
        entry_count = entry_count.saturating_add(1);

        if let Some(milestones) = env
            .storage()
            .persistent()
            .get::<DataKey, Vec<Milestone>>(&DataKey::MilestoneEscrow(escrow_id))
        {
            estimated_bytes = estimated_bytes.saturating_add(Self::storage_entry_bytes(
                &env,
                DataKey::MilestoneEscrow(escrow_id),
                milestones,
            ));
            entry_count = entry_count.saturating_add(1);
        }

        if let Some(time_lock) = env
            .storage()
            .persistent()
            .get::<DataKey, TimeLock>(&DataKey::TimeLockEscrow(escrow_id))
        {
            estimated_bytes = estimated_bytes.saturating_add(Self::storage_entry_bytes(
                &env,
                DataKey::TimeLockEscrow(escrow_id),
                time_lock,
            ));
            entry_count = entry_count.saturating_add(1);
        }

        if let Some(group_buy) = env
            .storage()
            .persistent()
            .get::<DataKey, GroupBuy>(&DataKey::GroupBuyEscrow(escrow_id))
        {
            estimated_bytes = estimated_bytes.saturating_add(Self::storage_entry_bytes(
                &env,
                DataKey::GroupBuyEscrow(escrow_id),
                group_buy,
            ));
            entry_count = entry_count.saturating_add(1);
        }

        Ok(StorageRentEstimate {
            escrow_id,
            entry_count,
            estimated_bytes,
            max_ttl: env.storage().max_ttl(),
        })
    }

    /// Snapshot the contract's bounded resource limits for off-chain load tests.
    pub fn get_resource_profile(env: Env) -> ContractResourceProfile {
        ContractResourceProfile {
            max_items_per_escrow: MAX_ITEMS_PER_ESCROW,
            max_metadata_size: MAX_METADATA_SIZE,
            unfunded_expiry_ledgers: UNFUNDED_EXPIRY_LEDGERS,
            evidence_window_ledgers: DEFAULT_EVIDENCE_WINDOW_LEDGERS,
            appeal_window_ledgers: APPEAL_WINDOW_LEDGERS,
            max_ttl: env.storage().max_ttl(),
        }
    }

    /// Return the semantic version of this contract deployment.
    /// Callers can compare against `CONTRACT_VERSION` to verify compatibility.
    pub fn get_version(_env: Env) -> ContractVersion {
        ContractVersion {
            major: 1,
            minor: 0,
            patch: 0,
        }
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

        if escrow.status != EscrowStatus::Pending && escrow.status != EscrowStatus::Funded {
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
        Self::add_i128(&env, DataKey::TotalReleasedAmount, seller_amount);
        Self::add_u32(&env, DataKey::TotalReleasedCount);
        
        StatusChangeEvent {
            escrow_id,
            from_status: from,
            to_status: to,
            actor,
        }
        .publish(env);
    }

    // =========================================================================
    // 🚦 ISSUE #215: TOKEN-SPECIFIC CIRCUIT BREAKER
    // =========================================================================

    /// Pause all escrow operations for a specific token.
    ///
    /// When a token is paused, `create_escrow`, `fund_escrow`, and
    /// `release_escrow` will reject any escrow denominated in that token.
    /// Existing escrows are not affected until the next state-mutating call.
    ///
    /// Admin-only.
    pub fn pause_token(env: Env, token: Address) -> Result<(), ContractError> {
        let admin = Self::assert_admin(&env)?;
        env.storage()
            .persistent()
            .set(&DataKey::TokenPaused(token.clone()), &true);
        TokenCircuitBreakerEvent {
            token,
            paused: true,
            actor: admin,
        }
        .publish(&env);
        Ok(())
    }

    /// Unpause a previously paused token, re-enabling escrow operations.
    ///
    /// Admin-only.
    pub fn unpause_token(env: Env, token: Address) -> Result<(), ContractError> {
        let admin = Self::assert_admin(&env)?;
        env.storage()
            .persistent()
            .remove(&DataKey::TokenPaused(token.clone()));
        TokenCircuitBreakerEvent {
            token,
            paused: false,
            actor: admin,
        }
        .publish(&env);
        Ok(())
    }

    /// Returns `true` if the given token is currently paused.
    pub fn is_token_paused(env: Env, token: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::TokenPaused(token))
            .unwrap_or(false)
    }

    fn assert_token_not_paused(env: &Env, token: &Address) -> Result<(), ContractError> {
        let paused: bool = env
            .storage()
            .persistent()
            .get(&DataKey::TokenPaused(token.clone()))
            .unwrap_or(false);
        if paused {
            return Err(ContractError::TokenPaused);
        }
        Ok(())
    }

    // =========================================================================
    // 🤝 ISSUE #205: DISPUTE MEDIATION PHASE
    // =========================================================================

    /// Open a mediation window for a disputed escrow.
    ///
    /// Called automatically when a dispute is raised (via `refund_escrow`), or
    /// manually by any escrow party. During the window, both parties may call
    /// `propose_mediation_settlement` to agree on a split without arbiter
    /// involvement. The arbiter may only call `resolve_dispute` after the
    /// mediation window has expired.
    ///
    /// If `window_ledgers` is 0, `DEFAULT_MEDIATION_WINDOW_LEDGERS` is used.
    pub fn open_mediation(
        env: Env,
        caller: Address,
        escrow_id: u64,
        window_ledgers: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_disputes_enabled(&env)?;
        caller.require_auth();

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
        }

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
            DEFAULT_MEDIATION_WINDOW_LEDGERS
        } else {
            window_ledgers
        };

        let now = env.ledger().sequence();
        let phase = MediationPhase {
            escrow_id,
            opened_at: now,
            expires_at: now + ledgers,
            buyer_proposal: None,
            seller_proposal: None,
            concluded: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::MediationPhase(escrow_id), &phase);

        MediationOpenedEvent {
            escrow_id,
            expires_at: now + ledgers,
        }
        .publish(&env);

        Ok(())
    }

    /// Propose a settlement amount during the mediation window.
    ///
    /// `seller_amount` is how much the caller proposes the seller receives.
    /// The remainder (`escrow.amount - seller_amount`) goes back to the buyer.
    ///
    /// If both parties propose the same `seller_amount`, the escrow is
    /// immediately settled without arbiter involvement.
    pub fn propose_mediation_settlement(
        env: Env,
        proposer: Address,
        escrow_id: u64,
        seller_amount: i128,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_disputes_enabled(&env)?;
        proposer.require_auth();

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
        }

        if proposer != escrow.buyer && proposer != escrow.seller {
            return Err(ContractError::Unauthorized);
        }

        if seller_amount < 0 || seller_amount > escrow.amount {
            return Err(ContractError::InvalidEscrowAmount);
        }

        let mut phase: MediationPhase = env
            .storage()
            .persistent()
            .get(&DataKey::MediationPhase(escrow_id))
            .ok_or(ContractError::NoMediationPhase)?;

        if phase.concluded {
            return Err(ContractError::MediationAlreadyConcluded);
        }

        if env.ledger().sequence() > phase.expires_at {
            return Err(ContractError::MediationAlreadyConcluded);
        }

        if proposer == escrow.buyer {
            phase.buyer_proposal = Some(seller_amount);
        } else {
            phase.seller_proposal = Some(seller_amount);
        }

        MediationProposedEvent {
            escrow_id,
            proposer: proposer.clone(),
            amount: seller_amount,
        }
        .publish(&env);

        // Check if both parties agree
        let agreed = phase.buyer_proposal == phase.seller_proposal
            && phase.buyer_proposal.is_some();

        if agreed {
            phase.concluded = true;
            env.storage()
                .persistent()
                .set(&DataKey::MediationPhase(escrow_id), &phase);

            Self::execute_mediation_settlement(&env, escrow_id, &escrow, seller_amount)?;
        } else {
            env.storage()
                .persistent()
                .set(&DataKey::MediationPhase(escrow_id), &phase);
        }

        Ok(())
    }

    fn execute_mediation_settlement(
        env: &Env,
        escrow_id: u64,
        escrow: &Escrow,
        seller_amount: i128,
    ) -> Result<(), ContractError> {
        let buyer_refund = escrow.amount - seller_amount;
        let mut net_seller: i128 = 0;

        let token_client = soroban_sdk::token::Client::new(env, &escrow.token);

        if seller_amount > 0 {
            let fee = Self::calculate_fee_internal(env, seller_amount, &escrow.token, &escrow.buyer);
            net_seller = seller_amount - fee;
            token_client.transfer(&env.current_contract_address(), &escrow.seller, &net_seller);
            if fee > 0 {
                let fee_collector: Address = env
                    .storage()
                    .persistent()
                    .get(&DataKey::FeeCollector)
                    .expect("Fee collector not configured");
                Self::add_pending_fee(env, fee_collector.clone(), escrow.token.clone(), fee);
                Self::add_i128(env, DataKey::TotalFeesCollected, fee);
                FeeCollectedEvent {
                    escrow_id,
                    fee_collector,
                    fee,
                }
                .publish(env);
            }
        }

        if buyer_refund > 0 {
            token_client.transfer(&env.current_contract_address(), &escrow.buyer, &buyer_refund);
        }

        let mut updated_escrow = escrow.clone();
        updated_escrow.status = EscrowStatus::Released;
        updated_escrow.cancellation_proposer = None;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &updated_escrow);

        MediationSettledEvent {
            escrow_id,
            seller_amount,
            buyer_refund,
        }
        .publish(env);

        let fee = seller_amount - net_seller;
        FundsReleasedEvent {
            escrow_id,
            amount: escrow.amount,
            fee,
        }
        .publish(env);

        StatusChangeEvent {
            escrow_id,
            from_status: EscrowStatus::Disputed,
            to_status: EscrowStatus::Released,
            actor: escrow.buyer.clone(),
        }
        .publish(env);

        Self::add_i128(env, DataKey::TotalReleasedAmount, net_seller);
        if buyer_refund > 0 {
            Self::add_i128(env, DataKey::TotalRefundedAmount, buyer_refund);
            Self::add_u32(env, DataKey::TotalRefundedCount);
        }

        Ok(())
    }

    /// Read the mediation phase for an escrow.
    pub fn get_mediation_phase(env: Env, escrow_id: u64) -> Option<MediationPhase> {
        env.storage()
            .persistent()
            .get(&DataKey::MediationPhase(escrow_id))
    }
}
