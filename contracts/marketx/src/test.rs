#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, vec, Address, Env};

fn setup() -> (Env, ContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
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
    assert_eq!(stored.released_amount, 0);
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

#[test]
fn test_release_blocked_when_fee_below_minimum() {
    let (env, client) = setup();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    client.initialize(&fee_collector, &100u32, &10i128);
    client.create_escrow(&1u64, &buyer, &seller, &500i128);

    let result = client.try_release_escrow(&1u64);
    assert_eq!(result, Err(Ok(ContractError::FeeBelowMinimum)));
}

#[test]
fn test_seller_authorization_check() {
    let (env, client) = setup();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let attacker = Address::generate(&env);

    client.create_escrow(&1u64, &buyer, &seller, &1_000i128);

    let denied = client.try_seller_refund(&1u64, &attacker);
    assert_eq!(denied, Err(Ok(ContractError::Unauthorized)));

    client.seller_refund(&1u64, &seller);
    let escrow = client.get_escrow(&1u64);
    assert_eq!(escrow.status, EscrowStatus::Refunded);
}

#[test]
fn test_bulk_escrow_creation_is_atomic() {
    let (env, client) = setup();
    let b1 = Address::generate(&env);
    let b2 = Address::generate(&env);
    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);

    let bad = client.try_create_bulk_escrows(
        &vec![&env, b1.clone(), b2.clone()],
        &vec![&env, s1.clone(), s2.clone()],
        &vec![&env, 100i128, 0i128],
    );
    assert_eq!(bad, Err(Ok(ContractError::InvalidEscrowAmount)));

    let ids = client.create_bulk_escrows(
        &vec![&env, b1.clone(), b2.clone()],
        &vec![&env, s1.clone(), s2.clone()],
        &vec![&env, 100i128, 200i128],
    );
    assert_eq!(ids, vec![&env, 1u64, 2u64]);

    let e1 = client.get_escrow(&1u64);
    let e2 = client.get_escrow(&2u64);
    assert_eq!(e1.amount, 100i128);
    assert_eq!(e2.amount, 200i128);
}

#[test]
fn test_bulk_escrow_creation_length_mismatch_rejected() {
    let (env, client) = setup();
    let b1 = Address::generate(&env);
    let s1 = Address::generate(&env);

    let result = client.try_create_bulk_escrows(
        &vec![&env, b1.clone()],
        &vec![&env, s1.clone()],
        &vec![&env, 100i128, 200i128],
    );
    assert_eq!(result, Err(Ok(ContractError::LengthMismatch)));
}

#[test]
fn test_release_partial_tracks_remaining_and_prevents_overrelease() {
    let (env, client) = setup();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    client.initialize(&fee_collector, &100u32, &1i128);
    client.create_escrow(&1u64, &buyer, &seller, &10_000i128);

    client.release_partial(&1u64, &4_000i128);
    let mid = client.get_escrow(&1u64);
    assert_eq!(mid.released_amount, 4_000i128);
    assert_eq!(mid.status, EscrowStatus::Pending);

    let over = client.try_release_partial(&1u64, &7_000i128);
    assert_eq!(over, Err(Ok(ContractError::InvalidReleaseAmount)));

    client.release_partial(&1u64, &6_000i128);
    let end = client.get_escrow(&1u64);
    assert_eq!(end.released_amount, 10_000i128);
    assert_eq!(end.status, EscrowStatus::Released);
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