#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

// ─── helpers ────────────────────────────────────────────────────────────────

fn make_escrow(env: &Env) -> (Escrow, Address, Address, Address) {
    let buyer = Address::generate(env);
    let seller = Address::generate(env);
    let token = Address::generate(env);
    let escrow = Escrow {
        buyer: buyer.clone(),
        seller: seller.clone(),
        token: token.clone(),
        amount: 5_000_000,
        status: EscrowStatus::Pending,
    };
    (escrow, buyer, seller, token)
}

fn setup() -> (Env, ContractClient<'static>) {
    let env = Env::default();
    let contract_id = env.register(Contract, ());
    let client = ContractClient::new(&env, &contract_id);
    (env, client)
}

// ─── storage tests (from #29) ────────────────────────────────────────────────

#[test]
fn test_store_and_retrieve_escrow() {
    let (env, client) = setup();
    let (escrow, buyer, seller, token) = make_escrow(&env);

    client.store_escrow(&1u64, &escrow);
    let retrieved = client.get_escrow(&1u64);

    assert_eq!(retrieved.buyer, buyer);
    assert_eq!(retrieved.seller, seller);
    assert_eq!(retrieved.token, token);
    assert_eq!(retrieved.amount, 5_000_000);
    assert_eq!(retrieved.status, EscrowStatus::Pending);
}

#[test]
fn test_multiple_escrows_stored_independently() {
    let (env, client) = setup();

    let (escrow_a, buyer_a, _, _) = make_escrow(&env);
    let (mut escrow_b, buyer_b, _, _) = make_escrow(&env);
    escrow_b.amount = 9_999_999;
    escrow_b.status = EscrowStatus::Released;

    client.store_escrow(&1u64, &escrow_a);
    client.store_escrow(&2u64, &escrow_b);

    let r_a = client.get_escrow(&1u64);
    let r_b = client.get_escrow(&2u64);

    assert_eq!(r_a.buyer, buyer_a);
    assert_eq!(r_a.status, EscrowStatus::Pending);
    assert_eq!(r_b.buyer, buyer_b);
    assert_eq!(r_b.amount, 9_999_999);
    assert_eq!(r_b.status, EscrowStatus::Released);
}

#[test]
fn test_try_get_escrow_success() {
    let (env, client) = setup();
    let (escrow, buyer, seller, token) = make_escrow(&env);

    client.store_escrow(&1u64, &escrow);
    let result = client.try_get_escrow(&1u64);

    assert!(result.is_ok());
    let retrieved = result.unwrap();
    assert_eq!(retrieved.buyer, buyer);
    assert_eq!(retrieved.seller, seller);
    assert_eq!(retrieved.token, token);
    assert_eq!(retrieved.amount, 5_000_000);
    assert_eq!(retrieved.status, EscrowStatus::Pending);
}

#[test]
fn test_try_get_escrow_not_found() {
    let (_env, client) = setup();

    let result = client.try_get_escrow(&99u64);
    assert!(result.is_err());
}

#[test]
fn test_escrow_status_variants_round_trip() {
    let (env, client) = setup();

    let statuses = [
        EscrowStatus::Pending,
        EscrowStatus::Released,
        EscrowStatus::Refunded,
        EscrowStatus::Disputed,
    ];

    for (id, status) in statuses.iter().enumerate() {
        let (mut escrow, _, _, _) = make_escrow(&env);
        escrow.status = status.clone();
        client.store_escrow(&(id as u64), &escrow);
        let retrieved = client.get_escrow(&(id as u64));
        assert_eq!(&retrieved.status, status);
    }
}

// ─── valid transitions ───────────────────────────────────────────────────────

#[test]
fn test_pending_to_released() {
    let (env, client) = setup();
    let (escrow, buyer, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    env.mock_auths(&[&buyer]);
    client.transition_status(&1u64, &EscrowStatus::Released);
    assert_eq!(client.get_escrow(&1u64).status, EscrowStatus::Released);
}

#[test]
fn test_pending_to_disputed() {
    let (env, client) = setup();
    let (escrow, buyer, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    env.mock_auths(&[&buyer]);
    client.transition_status(&1u64, &EscrowStatus::Disputed);
    assert_eq!(client.get_escrow(&1u64).status, EscrowStatus::Disputed);
}

#[test]
fn test_pending_to_refunded() {
    let (env, client) = setup();
    let (escrow, buyer, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    env.mock_auths(&[&buyer]);
    client.transition_status(&1u64, &EscrowStatus::Refunded);
    assert_eq!(client.get_escrow(&1u64).status, EscrowStatus::Refunded);
}

#[test]
fn test_disputed_to_released() {
    let (env, client) = setup();
    let (escrow, buyer, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    env.mock_auths(&[&buyer]);
    client.transition_status(&1u64, &EscrowStatus::Disputed);
    client.transition_status(&1u64, &EscrowStatus::Released);
    assert_eq!(client.get_escrow(&1u64).status, EscrowStatus::Released);
}

#[test]
fn test_disputed_to_refunded() {
    let (env, client) = setup();
    let (escrow, buyer, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    env.mock_auths(&[&buyer]);
    client.transition_status(&1u64, &EscrowStatus::Disputed);
    env.mock_auths(&[&buyer]);
    client.transition_status(&1u64, &EscrowStatus::Refunded);
    assert_eq!(client.get_escrow(&1u64).status, EscrowStatus::Refunded);
}

// ─── terminal states (Released, Refunded) ────────────────────────────────────

#[test]
fn test_released_is_terminal() {
    let (env, client) = setup();
    let (escrow, _, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);
    client.transition_status(&1u64, &EscrowStatus::Released);

    for next in [
        EscrowStatus::Pending,
        EscrowStatus::Disputed,
        EscrowStatus::Refunded,
        EscrowStatus::Released,
    ] {
        let result = client.try_transition_status(&1u64, &next);
        assert!(result.is_err(), "Released → {next:?} should be rejected");
    }
}

#[test]
fn test_refunded_is_terminal() {
    let (env, client) = setup();
    let (escrow, _, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);
    client.transition_status(&1u64, &EscrowStatus::Refunded);

    for next in [
        EscrowStatus::Pending,
        EscrowStatus::Disputed,
        EscrowStatus::Released,
        EscrowStatus::Refunded,
    ] {
        let result = client.try_transition_status(&1u64, &next);
        assert!(result.is_err(), "Refunded → {next:?} should be rejected");
    }
}

// ─── self-transitions rejected ────────────────────────────────────────────────

#[test]
fn test_self_transition_pending_rejected() {
    let (env, client) = setup();
    let (escrow, _, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    let result = client.try_transition_status(&1u64, &EscrowStatus::Pending);
    assert!(result.is_err());
}

#[test]
fn test_self_transition_disputed_rejected() {
    let (env, client) = setup();
    let (escrow, _, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);
    client.transition_status(&1u64, &EscrowStatus::Disputed);

    let result = client.try_transition_status(&1u64, &EscrowStatus::Disputed);
    assert!(result.is_err());
}

// ─── backward / skip transitions rejected ─────────────────────────────────────

#[test]
fn test_disputed_to_pending_rejected() {
    let (env, client) = setup();
    let (escrow, _, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);
    client.transition_status(&1u64, &EscrowStatus::Disputed);

    let result = client.try_transition_status(&1u64, &EscrowStatus::Pending);
    assert!(result.is_err());
}

// ─── escrow not found ────────────────────────────────────────────────────────

#[test]
fn test_transition_on_missing_escrow_rejected() {
    let (_env, client) = setup();

    let result = client.try_transition_status(&99u64, &EscrowStatus::Released);
    assert!(result.is_err());
}

#[test]
fn test_initialize_and_get_value() {
    let (env, client) = setup();

    client.initialize(&42u32);
    let value = client.get_initial_value();
    assert_eq!(value, 42);
}
