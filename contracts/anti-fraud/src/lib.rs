#![no_std]

mod events;
mod storage;
mod types;

#[cfg(test)]
mod test;

use crate::events::*;
use crate::storage::*;
use crate::types::{FraudError, RiskConfig, UserStats, UserStatus};
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol};

#[contract]
pub struct AntiFraudContract;

#[contractimpl]
impl AntiFraudContract {
    /// Initialize the contract with default risk parameters.
    pub fn initialize(e: Env, admin: Address) -> Result<(), FraudError> {
        if is_initialized(&e) {
            return Err(FraudError::AlreadyInitialized);
        }

        admin.require_auth();
        set_admin(&e, &admin);

        // Default risk config
        let default_config = RiskConfig {
            max_tx_amount: 10_000_000_000,      // Example: 1000 units
            daily_volume_limit: 50_000_000_000, // Example: 5000 units
            max_daily_tx_count: 20,
            updated_at: e.ledger().timestamp(),
        };
        set_risk_config(&e, &default_config);

        extend_instance_ttl(&e);
        Ok(())
    }

    /// Check if a transaction complies with fraud rules.
    /// Returns Ok(()) if allowed, Err(FraudError) if blocked.
    pub fn check_transaction(e: Env, user: Address, amount: i128) -> Result<(), FraudError> {
        extend_instance_ttl(&e);

        // 1. Check User Status (Blacklist)
        let status = get_user_status(&e, &user);
        if status == UserStatus::Blacklisted {
            publish_transaction_blocked(&e, user.clone(), amount, Symbol::new(&e, "blacklisted"));
            return Err(FraudError::UserBlacklisted);
        }

        // 2. Load Risk Config
        let config = get_risk_config(&e).ok_or(FraudError::NotInitialized)?;

        // 3. Check Transaction Limit
        if amount > config.max_tx_amount {
            publish_transaction_blocked(&e, user.clone(), amount, Symbol::new(&e, "amount_limit"));
            return Err(FraudError::LimitExceeded);
        }

        // 4. Check Velocity (Daily Volume & Count)
        let mut stats = get_user_stats(&e, &user);
        let current_time = e.ledger().timestamp();
        let one_day = 86400;

        // Reset stats if entering a new day window (simple approach: reset if > 24h since last tx)
        // Note: A rolling window would be more precise but more expensive.
        // Here we use a simplified "reset if user hasn't transacted in 24h" or "if last tx was previous day" logic?
        // For simplicity and to match common patterns, checks often reset at 00:00 UTC or relative to first tx of "day".
        // We will simple check if (current_time - last_timestamp) > one_day -> reset.
        if current_time.saturating_sub(stats.last_tx_timestamp) >= one_day {
            stats.daily_volume = 0;
            stats.daily_tx_count = 0;
        }

        if stats.daily_volume + amount > config.daily_volume_limit {
            publish_transaction_blocked(&e, user.clone(), amount, Symbol::new(&e, "daily_volume"));
            return Err(FraudError::LimitExceeded);
        }

        if stats.daily_tx_count >= config.max_daily_tx_count {
            publish_transaction_blocked(&e, user.clone(), amount, Symbol::new(&e, "daily_count"));
            return Err(FraudError::LimitExceeded);
        }

        Ok(())
    }

    /// Report activity to update user stats.
    /// Should be called after a successful transaction.
    pub fn report_activity(e: Env, user: Address, amount: i128) -> Result<(), FraudError> {
        extend_instance_ttl(&e);
        let config = get_risk_config(&e).ok_or(FraudError::NotInitialized)?; // Ensure initialized

        let mut stats = get_user_stats(&e, &user);
        let current_time = e.ledger().timestamp();
        let one_day = 86400;

        if current_time.saturating_sub(stats.last_tx_timestamp) >= one_day {
            stats.daily_volume = 0;
            stats.daily_tx_count = 0;
        }

        stats.last_tx_timestamp = current_time;
        stats.daily_volume += amount;
        stats.daily_tx_count += 1;
        stats.total_tx_count += 1;

        set_user_stats(&e, &user, &stats);

        Ok(())
    }

    /// Admin: Set Risk Parameters
    pub fn set_risk_params(
        e: Env,
        admin: Address,
        max_tx_amount: i128,
        daily_volume_limit: i128,
        max_daily_tx_count: u32,
    ) -> Result<(), FraudError> {
        admin.require_auth();
        let stored_admin = get_admin(&e).ok_or(FraudError::NotInitialized)?;
        if admin != stored_admin {
            return Err(FraudError::Unauthorized);
        }

        let old_config = get_risk_config(&e).unwrap();
        let new_config = RiskConfig {
            max_tx_amount,
            daily_volume_limit,
            max_daily_tx_count,
            updated_at: e.ledger().timestamp(),
        };

        set_risk_config(&e, &new_config);
        publish_risk_config_updated(&e, admin, old_config, new_config);
        extend_instance_ttl(&e);
        Ok(())
    }

    /// Admin: Manage User Lists (Blacklist/Whitelist/Suspicious)
    pub fn set_user_status(
        e: Env,
        admin: Address,
        user: Address,
        status: UserStatus,
    ) -> Result<(), FraudError> {
        admin.require_auth();
        let stored_admin = get_admin(&e).ok_or(FraudError::NotInitialized)?;
        if admin != stored_admin {
            return Err(FraudError::Unauthorized);
        }

        set_user_status(&e, &user, status);

        if status == UserStatus::Suspicious {
            publish_user_flagged(&e, user, Symbol::new(&e, "admin_flagged"));
        }

        extend_instance_ttl(&e);
        Ok(())
    }

    /// Admin/Provider: Set KYC Status
    pub fn set_kyc_status(
        e: Env,
        admin: Address,
        user: Address,
        is_verified: bool,
    ) -> Result<(), FraudError> {
        admin.require_auth();
        let stored_admin = get_admin(&e).ok_or(FraudError::NotInitialized)?;
        if admin != stored_admin {
            return Err(FraudError::Unauthorized);
        }

        // Ideally, we might want a separate "KYC Provider" role, but for now Admin controls it.
        set_kyc_status(&e, &user, is_verified);

        // If KYC verified, we might auto-verify permissions, but keeping them separate is flexible.
        // For example, update UserStatus to Verified if it was Unverified.
        let current_status = get_user_status(&e, &user);
        if is_verified && current_status == UserStatus::Unverified {
            set_user_status(&e, &user, UserStatus::Verified);
        }

        extend_instance_ttl(&e);
        Ok(())
    }

    pub fn get_user_status(e: Env, user: Address) -> UserStatus {
        get_user_status(&e, &user)
    }

    pub fn get_user_stats(e: Env, user: Address) -> UserStats {
        get_user_stats(&e, &user)
    }

    pub fn get_risk_config(e: Env) -> Result<RiskConfig, FraudError> {
        get_risk_config(&e).ok_or(FraudError::NotInitialized)
    }
}
