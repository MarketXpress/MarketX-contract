#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

mod types;
pub use types::{DataKey, Escrow, EscrowStatus};

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    /// Persist an escrow record under the given ID.
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
}

mod test;
