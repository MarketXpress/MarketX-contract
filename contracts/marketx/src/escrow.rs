//! escrow operations for the MarketX escrow contract.

use crate::*;
use soroban_sdk::*;

#[contractimpl]
impl Contract {
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
            .set(&DataKey::TotalReleasedAmount, &0i128);
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
            .set(&DataKey::TotalCancelledAmount, &0i128);
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

    pub(crate) fn create_escrow_internal(
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
            if *a == buyer || *a == seller {
                return Err(ContractError::ArbiterConflictOfInterest);
            }
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
    ///
    /// # Errors
    /// * `TooManyItems` - If `requests` exceeds `MAX_BULK_ESCROWS_PER_CALL`
    pub fn create_bulk_escrows(
        env: Env,
        buyer: Address,
        token: Address,
        requests: Vec<BulkEscrowRequest>,
    ) -> Result<Vec<u64>, ContractError> {
        Self::assert_not_paused(&env)?;
        buyer.require_auth();

        if requests.len() > MAX_BULK_ESCROWS_PER_CALL {
            return Err(ContractError::TooManyItems);
        }

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

    pub(crate) fn check_metadata_access(
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

        let is_admin = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Admin)
            .is_some_and(|admin| caller == &admin);

        if caller == &escrow.buyer || caller == &escrow.seller || is_admin {
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
    /// `limit` is clamped to `MAX_PAGE_SIZE` so a caller cannot force an
    /// unbounded storage scan in a single call (#260).
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

        let limit = limit.min(MAX_PAGE_SIZE);
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

    pub fn get_total_cancelled_amount(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalCancelledAmount)
            .unwrap_or(0)
    }

    /// Returns a structured summary containing comprehensive contract state metrics.
    pub fn analytics_summary(env: Env) -> GlobalDisputeAnalytics {
        let total_escrows = Self::get_total_escrows(env.clone());
        let released_count = Self::get_total_released_count(env.clone());
        let refunded_count = Self::get_total_refunded_count(env.clone());
        let disputed_count = Self::get_total_disputed_count(env.clone());
        let cancelled_count = Self::get_total_cancelled_count(env.clone());

        let failures = refunded_count + disputed_count + cancelled_count;
        let failure_rate_bps = ((failures as u64) * 10_000)
            .checked_div(total_escrows)
            .unwrap_or(0) as u32;

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

    /// Record oracle intent to release an escrow's funds (#244).
    ///
    /// A single oracle attestation no longer moves funds directly. This only
    /// records a `PendingOracleRelease`; the buyer has
    /// `DEFAULT_ORACLE_CHALLENGE_WINDOW_LEDGERS` to raise a dispute via
    /// `refund_escrow` (which moves the escrow to `Disputed`) before anyone
    /// may call `execute_oracle_release` to finalize the transfer. If the
    /// buyer disputes in time, the pending release is voided instead of
    /// executed — the oracle alone can no longer drain the escrow.
    pub fn verify_delivery(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        let oracle: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Oracle)
            .ok_or(ContractError::NotOracle)?;

        oracle.require_auth();

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        Self::assert_escrow_funded(&escrow)?;

        let tracking_id = escrow
            .tracking_id
            .clone()
            .ok_or(ContractError::Unauthorized)?;

        if env
            .storage()
            .persistent()
            .has(&DataKey::PendingOracleRelease(escrow_id))
        {
            return Err(ContractError::OracleReleasePending);
        }

        let now = env.ledger().sequence();
        let release_at = now + DEFAULT_ORACLE_CHALLENGE_WINDOW_LEDGERS;

        let pending = PendingOracleRelease {
            escrow_id,
            oracle,
            verified_at: now,
            release_at,
        };
        env.storage()
            .persistent()
            .set(&DataKey::PendingOracleRelease(escrow_id), &pending);

        DeliveryVerifiedEvent {
            escrow_id,
            tracking_id,
            release_at,
        }
        .publish(&env);

        Ok(())
    }

    /// Read the pending oracle release recorded for an escrow, if any (#244).
    pub fn get_pending_oracle_release(env: Env, escrow_id: u64) -> Option<PendingOracleRelease> {
        env.storage()
            .persistent()
            .get(&DataKey::PendingOracleRelease(escrow_id))
    }

    /// Finalize an oracle-triggered release once its challenge window has elapsed (#244).
    ///
    /// Permissionless — anyone may call this to execute a verified delivery
    /// once `release_at` has passed. Fails (and voids the pending release)
    /// if the buyer disputed the escrow in the meantime, since the escrow
    /// will no longer be `Pending`/`Funded`.
    pub fn execute_oracle_release(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        let pending: PendingOracleRelease = env
            .storage()
            .persistent()
            .get(&DataKey::PendingOracleRelease(escrow_id))
            .ok_or(ContractError::NoPendingOracleRelease)?;

        let now = env.ledger().sequence();
        if now < pending.release_at {
            return Err(ContractError::OracleChallengeWindowOpen);
        }

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // The buyer may have raised a dispute (or otherwise moved the escrow
        // out of Pending/Funded) during the challenge window. In that case
        // the oracle's release intent is void — the dispute flow now owns
        // this escrow's outcome, and this call permanently fails (the escrow
        // can never return to Pending/Funded from a terminal or disputed
        // state, so this pending release can never execute).
        Self::assert_escrow_funded(&escrow)?;

        let from_status = escrow.status.clone();
        let actor = pending.oracle.clone();

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
        env.storage()
            .persistent()
            .remove(&DataKey::PendingOracleRelease(escrow_id));

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
            from_status,
            to_status: escrow.status.clone(),
            actor,
        }
        .publish(&env);

        Ok(())
    }
}
