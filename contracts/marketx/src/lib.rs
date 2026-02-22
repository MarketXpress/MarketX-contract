#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

mod errors;
mod types;
pub use errors::ContractError;
pub use types::{DataKey, Escrow, EscrowStatus};

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    /// Persist a new escrow record under the given ID.
    pub fn store_escrow(env: Env, escrow_id: u64, escrow: Escrow) {
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);
    }

    /// Retrieve an escrow record by ID. Panics if not found.
    pub fn get_escrow(env: Env, escrow_id: u64) -> Escrow {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .unwrap()
    }

    /// Safely retrieve an escrow record by ID, returning a Result.
    ///
    /// Returns the Escrow struct data if found, or EscrowNotFound error otherwise.
    /// This is the recommended method for safer error handling.
    ///
    /// Errors:
    /// - `ContractError::EscrowNotFound` — no escrow exists for the given ID
    pub fn try_get_escrow(env: Env, escrow_id: u64) -> Result<Escrow, ContractError> {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)
    }

    /// Transition an escrow to a new status, enforcing the valid state graph.
    ///
    /// Errors:
    /// - `ContractError::EscrowNotFound`   — no record for `escrow_id`
    /// - `ContractError::InvalidTransition` — move not permitted from current state
    pub fn transition_status(
        env: Env,
        escrow_id: u64,
        new_status: EscrowStatus,
    ) -> Result<(), ContractError> {
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Require buyer authorization for buyer-initiated transitions
        if matches!(
            (&escrow.status, &new_status),
            (EscrowStatus::Pending, EscrowStatus::Released)
                | (EscrowStatus::Pending, EscrowStatus::Disputed)
                | (EscrowStatus::Pending, EscrowStatus::Refunded)
                | (EscrowStatus::Disputed, EscrowStatus::Refunded)
        ) {
            escrow.buyer.require_auth();
        }

        if !escrow.status.can_transition_to(&new_status) {
            return Err(ContractError::InvalidTransition);
        }

        escrow.status = new_status;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Ok(())
    }

    /// Initialize the contract with an initial value.
    pub fn initialize(env: Env, initial_value: u32) {
        env.storage()
            .persistent()
            .set(&DataKey::InitialValue, &initial_value);
    }

    /// Get the initial value.
    pub fn get_initial_value(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::InitialValue)
            .unwrap_or(0)
    }

    /// Release funds to the seller.
    ///
    /// Validates that the escrow is in Pending (funded) state and transitions
    /// it to Released. Prevents double release by checking current status.
    ///
    /// Errors:
    /// - `ContractError::EscrowNotFound`   — no record for `escrow_id`
    /// - `ContractError::EscrowNotFunded`  — escrow is not in Pending state
    /// - `ContractError::InvalidTransition` — transition not allowed
    pub fn release_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Validate escrow is funded (in Pending state)
        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::EscrowNotFunded);
        }

        // Use existing transition logic to update status to Released
        // This ensures authorization checks and prevents double release
        Self::transition_status(env, escrow_id, EscrowStatus::Released)
    }
}
