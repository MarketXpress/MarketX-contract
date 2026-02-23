#![no_std]

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env};

mod errors;
mod types;

pub use errors::ContractError;
pub use types::{
    DataKey, Escrow, EscrowStatus, RefundHistoryEntry, RefundReason, RefundRequest, RefundStatus,
};

#[cfg(test)]
mod test;

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    /// Store a caller-constructed escrow record under a specific ID.
    #[allow(deprecated)]
    pub fn store_escrow(env: Env, escrow_id: u64, escrow: Escrow) -> Result<(), ContractError> {
        if escrow.amount <= 0 {
            return Err(ContractError::InvalidEscrowAmount);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        env.events()
            .publish((symbol_short!("escrow_cr"), escrow_id), escrow);

        Ok(())
    }

    /// Create a new escrow using the required fields for issue #30.
    ///
    /// Accepts buyer, seller, and amount; creates an escrow in `Pending` state;
    /// and stores it in persistent contract storage.
    #[allow(deprecated)]
    pub fn create_escrow(
        env: Env,
        escrow_id: u64,
        buyer: Address,
        seller: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        if amount <= 0 {
            return Err(ContractError::InvalidEscrowAmount);
        }

        let escrow = Escrow {
            buyer,
            seller,
            arbiter: env.current_contract_address(),
            token: env.current_contract_address(),
            amount,
            status: EscrowStatus::Pending,
            refund_deadline: 0,
            allow_partial_refund: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        let escrow_count: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowCount)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::EscrowCount, &(escrow_count + 1));

        env.events()
            .publish((symbol_short!("escrow_cr"), escrow_id), escrow);

        Ok(())
    }

    /// Retrieve an escrow by ID, trapping if missing.
    pub fn get_escrow(env: Env, escrow_id: u64) -> Escrow {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .unwrap()
    }
}
