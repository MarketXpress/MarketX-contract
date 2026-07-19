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
//!      EscrowItem { amount: 500, released: false, description: None },
//!      EscrowItem { amount: 500, released: false, description: None },
//! ];
//!
//! let escrow_id = contract.create_escrow(
//!      &buyer, &seller, &token_address, &1000, &None, &None, &Some(items)
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
    AdminTransferredEvent, AppealFiledEvent, AppealRecord, AppealResolvedEvent, ArbiterReputation,
    ArbiterSlashedEvent, ArbiterStake, ArbiterStakedEvent, ArbiterVoteCastEvent, ArbiterVoteRecord,
    ArbitersConfig, ArbitersConfiguredEvent, BatchFeesCollectedEvent, BulkEscrowCreatedEvent,
    BulkEscrowRequest, BuyerContribution, CancellationProposedEvent, ContractResourceProfile,
    ContractVersion, CounterEvidenceSubmittedEvent, DataKey, DeliveryVerifiedEvent,
    DisputeConsensusReachedEvent, DisputeVotingRecord, Escrow, EscrowCreatedEvent,
    EscrowExpiredEvent, EscrowItem, EscrowStatus, EvidenceSubmittedEvent, EvidenceWindow,
    EvidenceWindowExpiredEvent, FeeCapsChangedEvent, FeeChangedEvent, FeeCollectedEvent,
    FeeCollectorRotatedEvent, FeeExemptionEvent, FeesWithdrawnEvent, FundsReleasedEvent,
    GlobalDisputeAnalytics, GroupBuy, GroupBuyCompletedEvent, GroupBuyFundedEvent,
    MediationOpenedEvent, MediationPhase, MediationProposedEvent, MediationSettledEvent,
    MetadataVisibility, Milestone, MilestoneCompletedEvent, RefundHistoryEntry, RefundReason,
    RefundRequest, RefundRequestedEvent, RefundStatus, StatusChangeEvent, StorageRentEstimate,
    TimeLock, TimeLockReleasedEvent, TokenCircuitBreakerEvent, APPEAL_WINDOW_LEDGERS,
    CONTRACT_VERSION, CURRENT_SCHEMA_VERSION, DEFAULT_ARBITER_QUORUM_PERCENTAGE,
    DEFAULT_EVIDENCE_WINDOW_LEDGERS, DEFAULT_MAX_ARBITERS_PER_ESCROW,
    DEFAULT_MEDIATION_WINDOW_LEDGERS, DEFAULT_MIN_ARBITERS_REQUIRED, MAX_DESCRIPTION_SIZE,
    MAX_EVIDENCE_HASH_SIZE, MAX_ITEMS_PER_ESCROW, MAX_METADATA_SIZE, MAX_TRACKING_ID_SIZE,
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

soroban_sdk::contractmeta!(key = "name", val = "MarketX Escrow");
soroban_sdk::contractmeta!(
    key = "description",
    val =
        "Soroban escrow contract with milestone releases, dispute handling, and configurable fees."
);
soroban_sdk::contractmeta!(
    key = "homepage",
    val = "https://github.com/MarketXpress/MarketX-contract"
);
soroban_sdk::contractmeta!(
    key = "repository",
    val = "https://github.com/MarketXpress/MarketX-contract"
);
soroban_sdk::contractmeta!(
    key = "source",
    val = "https://github.com/MarketXpress/MarketX-contract/tree/main/contracts/marketx"
);
soroban_sdk::contractmeta!(key = "version", val = "v1.0.0");

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

    fn assert_token_not_paused(env: &Env, token: &Address) -> Result<(), ContractError> {
        let is_paused: bool = env
            .storage()
            .persistent()
            .get(&DataKey::TokenCircuitBreaker(token.clone()))
            .unwrap_or(false);

        if is_paused {
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

    fn calculate_fee_internal(env: &Env, amount: i128, token: &Address, buyer: &Address) -> i128 {
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
        token_client.transfer(&env.current_contract_address(), seller, &seller_amount);

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
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.buyer,
            &escrow.amount,
        );

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

    fn check_duplicate_escrow(
        env: &Env,
        buyer: &Address,
        seller: &Address,
        metadata: &Option<Bytes>,
    ) -> Result<(), ContractError> {
        let hash = Self::generate_escrow_hash(env, buyer, seller, metadata);
        if env.storage().persistent().has(&DataKey::EscrowHash(hash)) {
            return Err(ContractError::DuplicateEscrow);
        }
        Ok(())
    }

    fn generate_escrow_hash(
        env: &Env,
        buyer: &Address,
        seller: &Address,
        metadata: &Option<Bytes>,
    ) -> BytesN<32> {
        let mut vec = Vec::new(env);
        vec.push_back(buyer.to_xdr(env));
        vec.push_back(seller.to_xdr(env));
        if let Some(m) = metadata {
            vec.push_back(m.to_xdr(env));
        }
        env.crypto().sha256(&vec.to_xdr(env)).into()
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

    fn is_escrow_party(escrow: &Escrow, actor: &Address) -> bool {
        actor == &escrow.buyer || actor == &escrow.seller || escrow.arbiter.as_ref() == Some(actor)
    }

    fn has_released_items(escrow: &Escrow) -> bool {
        escrow.items.iter().any(|item| item.released)
    }

    fn add_u32(env: &Env, key: DataKey) {
        let current: u32 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(current + 1));
    }

    fn check_zero_address(env: &Env, addr: &Address) -> Result<(), ContractError> {
        let zero = Address::from_string(&soroban_sdk::String::from_str(
            env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ));
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
    pub fn pause(env: Env) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage().persistent().set(&DataKey::Paused, &true);
        Ok(())
    }

    /// Unpause the contract, re-enabling all operations.
    pub fn unpause(env: Env) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage().persistent().set(&DataKey::Paused, &false);
        Ok(())
    }

    /// Check if the contract is currently paused.
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
                if items_vec.len() > MAX_ITEMS_PER_ESCROW {
                    return Err(ContractError::TooManyItems);
                }

                let items_sum: i128 = items_vec.iter().map(|item| item.amount).sum();
                if items_sum != amount {
                    return Err(ContractError::ItemAmountInvalid);
                }

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

        Self::check_metadata_access(&env, escrow_id, &escrow, &caller)?;

        Ok(escrow.metadata)
    }

    pub fn set_metadata_visibility(
        env: Env,
        escrow_id: u64,
        visibility: MetadataVisibility,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

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

    fn check_metadata_access(
        env: &Env,
        escrow_id: u64,
        escrow: &Escrow,
        caller: &Address,
    ) -> Result<(), ContractError> {
        let visibility: MetadataVisibility = env
            .storage()
            .persistent()
            .get(&DataKey::MetadataVisibility(escrow_id))
            .unwrap_or(MetadataVisibility::Private);

        if visibility == MetadataVisibility::Public {
            return Ok(());
        }

        if caller == &escrow.buyer
            || caller == &escrow.seller
            || caller
                == &env
                    .storage()
                    .persistent()
                    .get::<DataKey, Address>(&DataKey::Admin)
                    .unwrap()
        {
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
    pub fn get_escrows(env: Env, start: u64, limit: u32) -> Vec<Option<Escrow>> {
        let counter: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0);

        let mut result = Vec::new(&env);

        if counter == 0 || start == 0 || start > counter {
            return result;
        }

        let end = (start + limit as u64 - 1).min(counter);

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

    /// Returns a structured summary containing comprehensive contract state metrics.
    pub fn analytics_summary(env: Env) -> GlobalDisputeAnalytics {
        GlobalDisputeAnalytics {
            total_escrows: Self::get_total_escrows(env.clone()),
            total_funded_amount: Self::get_total_funded_amount(env.clone()),
            total_released_amount: Self::get_total_released_amount(env.clone()),
            total_refunded_amount: env.storage().persistent().get(&DataKey::TotalRefundedAmount).unwrap_or(0),
            total_released_count: Self::get_total_released_count(env.clone()),
            total_refunded_count: Self::get_total_refunded_count(env.clone()),
            total_disputed_count: Self::get_total_disputed_count(env.clone()),
            total_cancelled_count: Self::get_total_cancelled_count(env.clone()),
            total_fees_collected: env.storage().persistent().get(&DataKey::TotalFeesCollected).unwrap_or(0),
        }
    }
}