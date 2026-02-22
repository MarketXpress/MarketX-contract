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
}

mod test;
