//! utilities operations for the MarketX escrow contract.

use crate::*;
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::*;

impl Contract {
    pub(crate) fn disputes_enabled(env: &Env) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::FeatureDisputesEnabled)
            .unwrap_or(true)
    }

    pub(crate) fn partial_releases_enabled(env: &Env) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::FeaturePartialReleasesEnabled)
            .unwrap_or(true)
    }

    pub(crate) fn assert_admin(env: &Env) -> Result<Address, ContractError> {
        let admin = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Admin)
            .ok_or(ContractError::NotAdmin)?;

        admin.require_auth();
        Ok(admin)
    }

    pub(crate) fn assert_not_paused(env: &Env) -> Result<(), ContractError> {
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

    pub(crate) fn assert_disputes_enabled(env: &Env) -> Result<(), ContractError> {
        if !Self::disputes_enabled(env) {
            return Err(ContractError::FeatureDisabled);
        }
        Ok(())
    }

    pub(crate) fn assert_partial_releases_enabled(env: &Env) -> Result<(), ContractError> {
        if !Self::partial_releases_enabled(env) {
            return Err(ContractError::FeatureDisabled);
        }
        Ok(())
    }

    pub(crate) fn add_i128(env: &Env, key: DataKey, value: i128) {
        let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        // These are contract-wide analytics counters (total funded/released/
        // fees, etc.), incremented by per-escrow amounts that are themselves
        // bounded by i128 token balances. Reaching i128::MAX would require
        // aggregate volume many orders of magnitude beyond any real token
        // supply, so this is unreachable in practice; `expect` documents the
        // invariant instead of silently wrapping or truncating a stats value.
        let next = current.checked_add(value).expect("Global counter overflow");
        env.storage().persistent().set(&key, &next);
    }

    pub(crate) fn calculate_fee_internal(
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

        let volume_config: VolumeTierConfig = env
            .storage()
            .persistent()
            .get(&DataKey::VolumeTiers)
            .unwrap_or_default();
        let volume = Self::buyer_volume_internal(env, buyer, &volume_config);
        let discount = volume_config
            .discount_bps(volume_config.tier(volume))
            .min(500);
        let effective_fee_bps = fee_bps.saturating_sub(discount);

        let whole = amount / 10_000;
        let remainder = amount % 10_000;
        let mut fee: i128 = whole
            .saturating_mul(effective_fee_bps as i128)
            .saturating_add(remainder.saturating_mul(effective_fee_bps as i128) / 10_000);

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

    pub(crate) fn process_seller_transfer(
        env: &Env,
        escrow_id: u64,
        amount: i128,
        token: &Address,
        seller: &Address,
        buyer: &Address,
    ) -> Result<i128, ContractError> {
        let fee = Self::calculate_fee_internal(env, amount, token, buyer);
        let seller_amount = amount - fee;

        let token_client = soroban_sdk::token::Client::new(env, token);
        token_client.transfer(&env.current_contract_address(), seller, &seller_amount);

        if fee > 0 {
            let fee_collector: Address = env
                .storage()
                .persistent()
                .get(&DataKey::FeeCollector)
                .ok_or(ContractError::InvalidFeeConfig)?;

            Self::add_pending_fee(env, fee_collector.clone(), token.clone(), fee);
            Self::add_i128(env, DataKey::TotalFeesCollected, fee);

            FeeCollectedEvent {
                escrow_id,
                fee_collector,
                fee,
            }
            .publish(env);
        }

        Self::update_buyer_volume(env, buyer, amount);

        Ok(fee)
    }

    pub(crate) fn buyer_volume_internal(
        env: &Env,
        buyer: &Address,
        config: &VolumeTierConfig,
    ) -> i128 {
        if env.ledger().sequence().saturating_sub(config.reset_ledger) >= VOLUME_RESET_INTERVAL {
            0
        } else {
            env.storage()
                .persistent()
                .get(&DataKey::BuyerVolume(buyer.clone()))
                .unwrap_or(0)
        }
    }

    pub(crate) fn update_buyer_volume(env: &Env, buyer: &Address, amount: i128) {
        let mut config: VolumeTierConfig = env
            .storage()
            .persistent()
            .get(&DataKey::VolumeTiers)
            .unwrap_or_default();
        let current = Self::buyer_volume_internal(env, buyer, &config);
        if env.ledger().sequence().saturating_sub(config.reset_ledger) >= VOLUME_RESET_INTERVAL {
            config.reset_ledger = env.ledger().sequence();
            env.storage()
                .persistent()
                .set(&DataKey::VolumeTiers, &config);
        }
        let new_volume = current.saturating_add(amount);
        env.storage()
            .persistent()
            .set(&DataKey::BuyerVolume(buyer.clone()), &new_volume);
    }

    pub(crate) fn validate_bytes_size(data: &Bytes, max: u32) -> Result<(), ContractError> {
        if data.len() > max {
            return Err(ContractError::MetadataTooLarge);
        }
        Ok(())
    }

    pub(crate) fn add_pending_fee(env: &Env, collector: Address, token: Address, amount: i128) {
        let key = DataKey::PendingFee(collector, token);
        let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(current + amount));
    }

    pub(crate) fn settle_to_buyer(env: &Env, escrow: &mut Escrow, status: EscrowStatus) {
        let token_client = soroban_sdk::token::Client::new(env, &escrow.token);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.buyer,
            &escrow.amount,
        );

        escrow.status = status.clone();
        escrow.cancellation_proposer = None;
        if status == EscrowStatus::Cancelled {
            Self::add_i128(env, DataKey::TotalCancelledAmount, escrow.amount);
            Self::add_u32(env, DataKey::TotalCancelledCount);
        } else {
            Self::add_i128(env, DataKey::TotalRefundedAmount, escrow.amount);
            Self::add_u32(env, DataKey::TotalRefundedCount);
        }
    }

    pub(crate) fn refund_buyer(env: &Env, escrow: &mut Escrow) {
        Self::settle_to_buyer(env, escrow, EscrowStatus::Refunded);
    }

    pub(crate) fn cancel_to_buyer(env: &Env, escrow: &mut Escrow) {
        Self::settle_to_buyer(env, escrow, EscrowStatus::Cancelled);
    }

    pub(crate) fn validate_metadata(metadata: &Option<Bytes>) -> Result<(), ContractError> {
        if let Some(m) = metadata {
            if m.len() > MAX_METADATA_SIZE {
                return Err(ContractError::MetadataTooLarge);
            }
        }
        Ok(())
    }

    pub(crate) fn check_duplicate_escrow(
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

    pub(crate) fn generate_escrow_hash(
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

    pub(crate) fn next_escrow_id(env: &Env) -> Result<u64, ContractError> {
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

    pub(crate) fn next_refund_id(env: &Env) -> Result<u64, ContractError> {
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

    pub(crate) fn is_escrow_party(escrow: &Escrow, actor: &Address) -> bool {
        actor == &escrow.buyer || actor == &escrow.seller || escrow.arbiter.as_ref() == Some(actor)
    }

    pub(crate) fn has_released_items(escrow: &Escrow) -> bool {
        escrow.items.iter().any(|item| item.released)
    }

    pub(crate) fn assert_escrow_funded(escrow: &Escrow) -> Result<(), ContractError> {
        if escrow.status != EscrowStatus::Funded {
            return Err(ContractError::InvalidEscrowState);
        }

        Ok(())
    }

    pub(crate) fn add_u32(env: &Env, key: DataKey) {
        let current: u32 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(current + 1));
    }

    pub(crate) fn check_zero_address(env: &Env, addr: &Address) -> Result<(), ContractError> {
        let zero = Address::from_string(&soroban_sdk::String::from_str(
            env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ));
        if addr == &zero {
            return Err(ContractError::ZeroAddress);
        }
        Ok(())
    }

    pub(crate) fn xdr_len<T: ToXdr>(env: &Env, value: T) -> u32 {
        value.to_xdr(env).len() as u32
    }

    pub(crate) fn storage_entry_bytes<K: ToXdr, V: ToXdr>(env: &Env, key: K, value: V) -> u32 {
        Self::xdr_len(env, key).saturating_add(Self::xdr_len(env, value))
    }
}
