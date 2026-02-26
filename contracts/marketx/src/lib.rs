use soroban_sdk::{contract, contractimpl, Env, Address, Symbol};

#[contract]
pub struct MarketXContract;


#![no_std]

use soroban_sdk::{
    contract, contractimpl, panic_with_error, Address, Env,
};
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};
use soroban_sdk::{contracttype};


mod errors;
mod types;

use errors::ContractError;
use types::DataKey;


pub use errors::ContractError;
pub use types::{
    DataKey, Escrow, EscrowCreatedEvent, EscrowStatus, FundsReleasedEvent, RefundHistoryEntry,
    RefundReason, RefundRequest, RefundStatus, StatusChangeEvent,
};

#[cfg(test)]
mod test;

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

            .get(&DataKey::EscrowIds)
            .unwrap_or(Vec::new(&env));
        escrow_ids.push_back(escrow_id);
        env.storage()
            .persistent()
            .set(&DataKey::EscrowIds, &escrow_ids);

        Ok(())
    }

    #[contractimpl]
impl MarketXContract {
    pub fn init(env: Env, admin: Address) {
        env.storage().instance().set(&Symbol::new(&env, "admin"), &admin);
    }

    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        let admin: Address = env.storage().instance().get(&Symbol::new(&env, "admin")).unwrap();
        admin.require_auth();

        // Update contract code reference
        env.deployer().update_current_contract_wasm(new_wasm_hash);
    }
}

    /// Retrieve an escrow record by ID.
    ///
    /// # Panics
    ///
    /// Panics (contract trap) if no record exists for `escrow_id`. Prefer
    /// [`try_get_escrow`] for cases where the ID may not exist.
    ///
    /// # Arguments
    ///
    /// * `escrow_id` — identifier previously passed to [`store_escrow`].
    pub fn get_escrow(env: Env, escrow_id: u64) -> Escrow {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .unwrap()
    }


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

    /// Get the current admin address.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::Admin)
    }

    // ─── Fee Management ────────────────────────────────────────────────────────

    /// Set the platform fee percentage (basis points).
    ///
    /// Only callable by the admin. Validates that the fee is within the allowed
    /// range (0-1000 bps = 0-10%). Emits an event on successful fee change.
    ///
    /// # Arguments
    ///
    /// * `fee_bps` — new platform fee in basis points (`0..=1000`).
    ///   For example, `250` = 2.5 %.
    ///
    /// # Errors
    ///
    /// - [`ContractError::NotAdmin`] — caller is not the admin.
    /// - [`ContractError::InvalidFeeConfig`] — `fee_bps` exceeds 1000.
    pub fn set_fee_percentage(env: Env, fee_bps: u32) -> Result<(), ContractError> {
        // Verify admin
        let admin = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Admin)
            .ok_or(ContractError::NotAdmin)?;
        admin.require_auth();

        // Validate fee is within allowed range (max 10% = 1000 bps)
        if fee_bps > 1000 {
            return Err(ContractError::InvalidFeeConfig);
        }

        env.storage()
            .persistent()
            .set(&DataKey::FeeBps, &fee_bps);

        env.events().publish(
            (Symbol::new(&env, "fee_changed"),),
            fee_bps,
        );

        Ok(())
    }

    /// Get the current fee percentage in basis points.
    pub fn get_fee_bps(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::FeeBps)
            .unwrap_or(0)
    }


#[contracttype]
pub struct Project {
    pub id: u32,
    pub owner: Address,
    pub created_at: u32,
    pub amount: u64,
}

    // ─── Refund Request Functions ───────────────────────────────────────────

    /// Submit a refund request for an escrow.
    ///
    /// Buyers can request a refund within the specified refund deadline.
    /// Supports both full and partial refunds based on escrow configuration.
    ///
    /// # Arguments
    ///
    /// * `escrow_id` — identifier of the escrow to request refund for.
    /// * `refund_amount` — amount to refund (must be positive and <= escrow amount).
    /// * `reason` — reason for the refund request.
    /// * `description` — additional details about the refund request.
    ///
    /// # Errors
    ///
    /// - [`ContractError::EscrowNotFound`] — no escrow exists for `escrow_id`.
    /// - [`ContractError::RefundAmountExceedsEscrow`] — refund amount exceeds escrow amount.
    /// - [`ContractError::RefundWindowExpired`] — refund deadline has passed.
    /// - [`ContractError::InvalidTransition`] — escrow is not in a refundable state.
    pub fn submit_refund_request(
        env: Env,
        escrow_id: u64,
        refund_amount: i128,
        reason: RefundReason,
        description: String,
    ) -> Result<u64, ContractError> {
        let escrow = env
            .storage()
            .persistent()
            .get::<DataKey, Escrow>(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Validate escrow is in a refundable state
        if escrow.status != EscrowStatus::Pending && escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidTransition);
        }

        // Validate refund amount is positive
        if refund_amount <= 0 {
            return Err(ContractError::InvalidEscrowAmount);
        }

        // Validate refund amount does not exceed escrow amount
        if refund_amount > escrow.amount {
            return Err(ContractError::RefundAmountExceedsEscrow);
        }

        // Validate refund deadline has not passed
        if escrow.refund_deadline > 0 && env.ledger().timestamp() > escrow.refund_deadline {
            return Err(ContractError::RefundWindowExpired);
        }

        // Generate a new request ID
        let request_count: u64 = env
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

        // 🔒 Circuit breaker default
        env.storage().persistent().set(&DataKey::Paused, &false);

        // Optional counters
        env.storage().persistent().set(&DataKey::EscrowCounter, &0u64);
    }

    // =========================
    // 🔒 CIRCUIT BREAKER API
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
        // existing fund logic
        Ok(())
    }

    pub fn release_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        // existing release logic
        Ok(())
    }

    pub fn release_partial(
        env: Env,
        escrow_id: u64,
        amount: i128,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        // existing partial release logic
        Ok(())
    }

    pub fn refund_escrow(
        env: Env,
        escrow_id: u64,
        initiator: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        initiator.require_auth();
        // existing refund logic
        Ok(())
    }

    pub fn resolve_dispute(
        env: Env,
        escrow_id: u64,
        resolution: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        // existing dispute resolution logic
        Ok(())
    }

}
