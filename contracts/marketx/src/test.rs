#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{Contract, ContractClient};
use crate::errors::ContractError;

fn setup() -> (Env, ContractClient) {
    let env = Env::default();
    let contract_id = env.register_contract(None, Contract);
    let client = ContractClient::new(&env, &contract_id);
    (env, client)
}

#[test]
fn escrow_ids_increment_sequentially() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let id1 = client.create_escrow(&buyer, &seller, &1000);
    let id2 = client.create_escrow(&buyer, &seller, &2000);
    let id3 = client.create_escrow(&buyer, &seller, &3000);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[test]
fn no_escrow_id_collision() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let mut ids = std::collections::BTreeSet::new();

    for _ in 0..10 {
        let id = client.create_escrow(&buyer, &seller, &100);
        assert!(ids.insert(id));
    }
}

#[test]
fn escrow_counter_overflow_fails() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    // force counter to max
    env.storage()
        .persistent()
        .set(&crate::types::DataKey::EscrowCounter, &u64::MAX);

    let result = client.try_create_escrow(&buyer, &seller, &100);
    assert_eq!(result, Err(Ok(ContractError::EscrowIdOverflow)));
}

#[test]
fn test_reentrancy_guard_blocks_nested_release() {
    let (env, client) = setup();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    client.initialize(&fee_collector, &100u32, &1i128);
    client.create_escrow(&1u64, &buyer, &seller, &10_000i128);

    let result = client.try_simulate_reentrant_release(&1u64);
    assert_eq!(result, Err(Ok(ContractError::ReentrancyDetected)));
}

#[test]
fn test_project_storage_size() {
    use std::mem::size_of;
    assert!(size_of::<Project>() <= 32, "Project struct too large");
}

#[test]
fn test_project_creation() {
    let project = Project {
        id: 1,
        owner: Address::random(),
        created_at: 1_700_000_000,
        amount: 1000,
    };
    assert_eq!(project.amount, 1000);
}

fn test_upgrade_preserves_state() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::random(&env);
    MarketXContract::init(env.clone(), admin.clone());

    // Set state
    MarketXContract::set_project(env.clone(), 1, admin.clone());
    assert_eq!(MarketXContract::get_project(env.clone(), 1), Some(admin.clone()));

    // Upgrade contract
    let new_wasm_hash = BytesN::<32>::random(&env);
    MarketXContract::upgrade(env.clone(), new_wasm_hash);

    // State should still be intact
    assert_eq!(MarketXContract::get_project(env.clone(), 1), Some(admin.clone()));
}

