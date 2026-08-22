//! fees operations for the MarketX escrow contract.

use crate::*;
use soroban_sdk::*;

#[contractimpl]
impl Contract {
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

    /// Get the currently configured fee collector.
    pub fn get_fee_collector(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::FeeCollector)
    }

    /// Rotate the fee collector to a new treasury address.
    ///
    /// This is admin-controlled and only affects future fee accruals.
    pub fn set_fee_collector(env: Env, fee_collector: Address) -> Result<(), ContractError> {
        let admin = Self::assert_admin(&env)?;
        let old_collector: Address = env
            .storage()
            .persistent()
            .get(&DataKey::FeeCollector)
            .ok_or(ContractError::InvalidFeeConfig)?;

        if old_collector == fee_collector {
            return Ok(());
        }

        env.storage()
            .persistent()
            .set(&DataKey::FeeCollector, &fee_collector);

        FeeCollectorRotatedEvent {
            old_collector,
            new_collector: fee_collector,
            actor: admin,
        }
        .publish(&env);

        Ok(())
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

    pub fn get_total_fees_collected(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalFeesCollected)
            .unwrap_or(0)
    }

    pub fn get_buyer_volume(env: Env, buyer: Address) -> i128 {
        let config: VolumeTierConfig = env
            .storage()
            .persistent()
            .get(&DataKey::VolumeTiers)
            .unwrap_or_default();
        Self::buyer_volume_internal(&env, &buyer, &config)
    }

    pub fn get_buyer_tier(env: Env, buyer: Address) -> u32 {
        let config: VolumeTierConfig = env
            .storage()
            .persistent()
            .get(&DataKey::VolumeTiers)
            .unwrap_or_default();
        config.tier(Self::buyer_volume_internal(&env, &buyer, &config))
    }

    pub fn get_volume_tiers(env: Env) -> VolumeTierConfig {
        env.storage()
            .persistent()
            .get(&DataKey::VolumeTiers)
            .unwrap_or_default()
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

    // =========================
    // 💰 BATCH FEE COLLECTION (#171)
    // =========================

    /// Collect fees from multiple escrows in a single transaction.
    /// This is more efficient than collecting fees one-by-one.
    ///
    /// Each escrow's fee was already credited into `PendingFee(collector, token)`
    /// when it released (see `withdraw_fees`'s pull-pattern bookkeeping). This
    /// function transfers that real, already-accrued balance to `collector` in
    /// one batch, itemized against the requested `escrow_ids`, rather than
    /// fabricating a new amount from scratch. Each escrow is flagged once
    /// collected so the same fee can never be paid out twice.
    ///
    /// # Arguments
    /// * `escrow_ids` - Vector of escrow IDs to collect fees from (max `MAX_ESCROWS_PER_BATCH`)
    ///
    /// # Returns
    /// Total amount of fees actually transferred to `collector`.
    ///
    /// # Errors
    /// * `EscrowNotFound` - If any escrow doesn't exist
    /// * `TooManyItems` - If `escrow_ids` exceeds `MAX_ESCROWS_PER_BATCH`
    pub fn batch_collect_fees(
        env: Env,
        collector: Address,
        token: Address,
        escrow_ids: Vec<u64>,
    ) -> Result<i128, ContractError> {
        collector.require_auth();

        if escrow_ids.len() > MAX_ESCROWS_PER_BATCH {
            return Err(ContractError::TooManyItems);
        }

        let pending_key = DataKey::PendingFee(collector.clone(), token.clone());
        let mut pending_balance: i128 = env.storage().persistent().get(&pending_key).unwrap_or(0);

        let mut total_fees: i128 = 0;
        let mut count: u32 = 0;
        let mut collected_ids: Vec<u64> = Vec::new(&env);

        for escrow_id in escrow_ids.iter() {
            let escrow: Escrow = env
                .storage()
                .persistent()
                .get(&DataKey::Escrow(escrow_id))
                .ok_or(ContractError::EscrowNotFound)?;

            // Only collect from released escrows with a matching token that
            // haven't already had their fee collected via this path.
            if escrow.status != EscrowStatus::Released || escrow.token != token {
                continue;
            }

            let collected_key = DataKey::EscrowFeeCollected(escrow_id);
            if env
                .storage()
                .persistent()
                .get(&collected_key)
                .unwrap_or(false)
            {
                continue;
            }

            let fee =
                Self::calculate_fee_internal(&env, escrow.amount, &escrow.token, &escrow.buyer);

            // The pending-fee ledger for this collector/token pair must
            // actually hold this fee (it was credited there at release
            // time). If it doesn't — e.g. fee config changed afterwards —
            // skip rather than overdraw funds that aren't really there.
            if fee <= 0 || fee > pending_balance {
                continue;
            }

            pending_balance -= fee;
            total_fees += fee;
            count += 1;
            collected_ids.push_back(escrow_id);
        }

        if total_fees > 0 {
            env.storage()
                .persistent()
                .set(&pending_key, &pending_balance);
            for id in collected_ids.iter() {
                env.storage()
                    .persistent()
                    .set(&DataKey::EscrowFeeCollected(id), &true);
            }

            let token_client = soroban_sdk::token::Client::new(&env, &token);
            token_client.transfer(&env.current_contract_address(), &collector, &total_fees);

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
}
