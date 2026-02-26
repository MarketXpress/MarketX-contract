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
