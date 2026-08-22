//! time locks operations for the MarketX escrow contract.

use crate::*;
use soroban_sdk::*;

#[contractimpl]
impl Contract {
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

        // Only buyer can set time lock in this version (restored for API compatibility)
        escrow.buyer.require_auth();

        Self::assert_escrow_funded(&escrow)?;

        let time_lock = TimeLock {
            release_ledger,
            enabled: true,
        };

        let mut tl_vec = Vec::new(&env);
        tl_vec.push_back(time_lock.clone());
        escrow.time_lock = tl_vec;
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

        Self::assert_escrow_funded(&escrow)?;

        let time_lock = escrow
            .time_lock
            .get(0)
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

        let fee = Self::process_seller_transfer(
            &env,
            escrow_id,
            escrow.amount,
            &escrow.token,
            &escrow.seller,
            &escrow.buyer,
        )?;

        escrow.status = EscrowStatus::Released;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        TimeLockReleasedEvent {
            escrow_id,
            amount: escrow.amount,
        }
        .publish(&env);

        FundsReleasedEvent {
            escrow_id,
            amount: escrow.amount,
            fee,
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
        escrow.and_then(|e| e.time_lock.get(0))
    }
}
