#![no_std]

use soroban_sdk::{
    contract, contractimpl, panic_with_error, Address, Env,
};

mod errors;
mod types;

use errors::ContractError;
use types::DataKey;

#[contract]
pub struct Contract;

impl Contract {
    // =========================
    // 🔐 INTERNAL GUARDS
    // =========================

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
}

#[contractimpl]
impl Contract {
    // =========================
    // 🚀 INITIALIZATION
    // =========================

    pub fn initialize(
        env: Env,
        admin: Address,
        fee_collector: Address,
        fee_bps: u32,
    ) {
        admin.require_auth();

        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::FeeCollector, &fee_collector);
        env.storage().persistent().set(&DataKey::FeeBps, &fee_bps);
        env.storage().persistent().set(&DataKey::Paused, &false);
        env.storage().persistent().set(&DataKey::EscrowCount, &0u64);
    }

    // =========================
    // 🔒 CIRCUIT BREAKER
    // =========================

    pub fn pause(env: Env) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage().persistent().set(&DataKey::Paused, &true);
        Ok(())
    }

    pub fn unpause(env: Env) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage().persistent().set(&DataKey::Paused, &false);
        Ok(())
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    // =========================
    // 💰 ESCROW ACTIONS
    // =========================

    pub fn fund_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        // existing fund logic here
        Ok(())
    }

    pub fn release_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        // existing release logic here
        Ok(())
    }

    pub fn release_partial(
        env: Env,
        escrow_id: u64,
        amount: i128,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        // existing partial release logic here
        Ok(())
    }

    pub fn refund_escrow(
        env: Env,
        escrow_id: u64,
        initiator: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        initiator.require_auth();
        // existing refund logic here
        Ok(())
    }

    pub fn resolve_dispute(
        env: Env,
        escrow_id: u64,
        resolution: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        // existing dispute resolution logic here
        Ok(())
    }

    impl Contract {
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
}
pub fn initialize(
    env: Env,
    admin: Address,
    fee_collector: Address,
    fee_bps: u32,
) {
    admin.require_auth();

    env.storage().persistent().set(&DataKey::Admin, &admin);
    env.storage().persistent().set(&DataKey::FeeCollector, &fee_collector);
    env.storage().persistent().set(&DataKey::FeeBps, &fee_bps);

    // 🔢 Counter starts at 0
    env.storage().persistent().set(&DataKey::EscrowCounter, &0u64);

    // Circuit breaker default
    env.storage().persistent().set(&DataKey::Paused, &false);
}

}
