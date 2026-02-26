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
fn admin_can_pause_and_unpause() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let collector = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &collector, &250);

    assert!(!client.is_paused());
    client.pause().unwrap();
    assert!(client.is_paused());

    client.unpause().unwrap();
    assert!(!client.is_paused());
}

#[test]
#[should_panic(expected = "NotAdmin")]
fn non_admin_cannot_pause() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let collector = Address::generate(&env);

    env.mock_auths(&[&admin]);
    client.initialize(&admin, &collector, &250);

    env.mock_auths(&[&user]);
    client.pause().unwrap();
}

#[test]
fn escrow_actions_blocked_when_paused() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let collector = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &collector, &250);
    client.pause().unwrap();

    let res = client.try_fund_escrow(&1u64);
    assert_eq!(res, Err(Ok(ContractError::ContractPaused)));
}
