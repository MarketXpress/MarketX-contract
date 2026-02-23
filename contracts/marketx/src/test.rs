#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, ContractClient<'static>) {
    let env = Env::default();
    let contract_id = env.register(Contract, ());
    let client = ContractClient::new(&env, &contract_id);
    (env, client)
}

#[test]
fn test_create_escrow_stores_values() {
    let (env, client) = setup();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    client.create_escrow(&1u64, &buyer, &seller, &5_000_000i128);

    let stored = client.get_escrow(&1u64);
    assert_eq!(stored.buyer, buyer);
    assert_eq!(stored.seller, seller);
    assert_eq!(stored.amount, 5_000_000i128);
    assert_eq!(stored.status, EscrowStatus::Pending);
}

#[test]
fn test_create_escrow_rejects_non_positive_amount() {
    let (env, client) = setup();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    let zero = client.try_create_escrow(&2u64, &buyer, &seller, &0i128);
    assert_eq!(zero, Err(Ok(ContractError::InvalidEscrowAmount)));

    let negative = client.try_create_escrow(&3u64, &buyer, &seller, &-1i128);
    assert_eq!(negative, Err(Ok(ContractError::InvalidEscrowAmount)));
}
