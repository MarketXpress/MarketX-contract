#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token, Address, Env,
};

// ─── helpers ────────────────────────────────────────────────────────────────

fn make_escrow(env: &Env) -> (Escrow, Address, Address, Address) {
    let buyer = Address::generate(env);
    let seller = Address::generate(env);
    let token_addr = Address::generate(env);
    let escrow = Escrow {
        buyer: buyer.clone(),
        seller: seller.clone(),
        token: token_addr.clone(),
        amount: 5_000_000,
        status: EscrowStatus::Pending,
    };
    (escrow, buyer, seller, token_addr)
}

/// Deploy the MarketX contract and a real Soroban token, mint `amount` to
/// `recipient`, and return the contract client, token address, and token admin client.
fn setup_with_token(
    env: &Env,
    amount: i128,
    recipient: &Address,
) -> (ContractClient<'static>, Address, token::StellarAssetClient<'static>) {
    let contract_id = env.register(Contract, ());
    let client = ContractClient::new(env, &contract_id);

    let token_admin = Address::generate(env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_sac = token::StellarAssetClient::new(env, &token_contract.address());
    token_sac.mint(recipient, &amount);

    (client, token_contract.address(), token_sac)
}

fn setup() -> (Env, ContractClient<'static>) {
    let env = Env::default();
    let contract_id = env.register(Contract, ());
    let client = ContractClient::new(&env, &contract_id);
    (env, client)
}

// ─── initialization tests ───────────────────────────────────────────────────

#[test]
fn test_initialize_stores_fee_config() {
    let (env, client) = setup();
    let collector = Address::generate(&env);
    assert!(client.initialize(&collector, &250u32).is_ok());
}

#[test]
fn test_initialize_fee_bps_boundary_accepted() {
    let (env, client) = setup();
    let collector = Address::generate(&env);
    // 10_000 bps = 100 % — extreme but valid.
    assert!(client.initialize(&collector, &10_000u32).is_ok());
}

#[test]
fn test_initialize_invalid_fee_bps_rejected() {
    let (env, client) = setup();
    let collector = Address::generate(&env);
    let result = client.try_initialize(&collector, &10_001u32);
    assert_eq!(result, Err(Ok(ContractError::InvalidFeeConfig)));
}

// ─── storage tests (from #29) ────────────────────────────────────────────────

#[test]
fn test_store_and_retrieve_escrow() {
    let (env, client) = setup();
    let (escrow, buyer, seller, token_addr) = make_escrow(&env);

    client.store_escrow(&1u64, &escrow);
    let retrieved = client.get_escrow(&1u64);

    assert_eq!(retrieved.buyer, buyer);
    assert_eq!(retrieved.seller, seller);
    assert_eq!(retrieved.token, token_addr);
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
    let (escrow, buyer, seller, token_addr) = make_escrow(&env);

    client.store_escrow(&1u64, &escrow);
    let result = client.try_get_escrow(&1u64);

    assert!(result.is_ok());
    let retrieved = result.unwrap();
    assert_eq!(retrieved.buyer, buyer);
    assert_eq!(retrieved.seller, seller);
    assert_eq!(retrieved.token, token_addr);
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

// ─── status-only transition tests ────────────────────────────────────────────

#[test]
fn test_pending_to_disputed() {
    let (env, client) = setup();
    let (escrow, _, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    env.mock_all_auths();
    client.transition_status(&1u64, &EscrowStatus::Disputed).unwrap();
    assert_eq!(client.get_escrow(&1u64).status, EscrowStatus::Disputed);
}

#[test]
fn test_disputed_to_released_via_transition() {
    let (env, client) = setup();
    let (escrow, _, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    env.mock_all_auths();
    client.transition_status(&1u64, &EscrowStatus::Disputed).unwrap();
    client.transition_status(&1u64, &EscrowStatus::Released).unwrap();
    assert_eq!(client.get_escrow(&1u64).status, EscrowStatus::Released);
}

// ─── terminal states (Released, Refunded) ────────────────────────────────────

#[test]
fn test_released_is_terminal() {
    let (env, client) = setup();
    let (mut escrow, _, _, _) = make_escrow(&env);
    escrow.status = EscrowStatus::Released;
    client.store_escrow(&1u64, &escrow);

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
    let (mut escrow, _, _, _) = make_escrow(&env);
    escrow.status = EscrowStatus::Refunded;
    client.store_escrow(&1u64, &escrow);

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
    let (mut escrow, _, _, _) = make_escrow(&env);
    escrow.status = EscrowStatus::Disputed;
    client.store_escrow(&1u64, &escrow);

    let result = client.try_transition_status(&1u64, &EscrowStatus::Disputed);
    assert!(result.is_err());
}

// ─── backward transitions rejected ───────────────────────────────────────────

#[test]
fn test_disputed_to_pending_rejected() {
    let (env, client) = setup();
    let (mut escrow, _, _, _) = make_escrow(&env);
    escrow.status = EscrowStatus::Disputed;
    client.store_escrow(&1u64, &escrow);

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

// ─── wrong caller tests ──────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "not satisfied")]
fn test_wrong_caller_pending_to_released() {
    let (env, client) = setup();
    let (escrow, _buyer, seller, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    // Seller tries to release (only buyer can)
    env.mock_auths(&[&seller]);
    client.transition_status(&1u64, &EscrowStatus::Released);
}

#[test]
#[should_panic(expected = "not satisfied")]
fn test_wrong_caller_pending_to_disputed() {
    let (env, client) = setup();
    let (escrow, _buyer, seller, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    // Seller tries to dispute (only buyer can)
    env.mock_auths(&[&seller]);
    client.transition_status(&1u64, &EscrowStatus::Disputed);
}

#[test]
#[should_panic(expected = "not satisfied")]
fn test_wrong_caller_pending_to_refunded() {
    let (env, client) = setup();
    let (escrow, _buyer, seller, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    // Seller tries to refund (only buyer can)
    env.mock_auths(&[&seller]);
    client.transition_status(&1u64, &EscrowStatus::Refunded);
}

#[test]
#[should_panic(expected = "not satisfied")]
fn test_wrong_caller_disputed_to_refunded() {
    let (env, client) = setup();
    let (escrow, buyer, seller, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    // Buyer disputes first
    env.mock_auths(&[&buyer]);
    client.transition_status(&1u64, &EscrowStatus::Disputed);

    // Seller tries to refund (only buyer can)
    env.mock_auths(&[&seller]);
    client.transition_status(&1u64, &EscrowStatus::Refunded);
}

#[test]
#[should_panic(expected = "not satisfied")]
fn test_unauthorized_third_party_cannot_transition() {
    let (env, client) = setup();
    let (escrow, _buyer, _seller, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    // Random third party tries to transition
    let third_party = Address::generate(&env);
    env.mock_auths(&[&third_party]);
    client.transition_status(&1u64, &EscrowStatus::Released);
}

// ─── invalid state transitions ───────────────────────────────────────────────

#[test]
fn test_released_to_pending_rejected() {
    let (env, client) = setup();
    let (escrow, buyer, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    env.mock_auths(&[&buyer]);
    client.transition_status(&1u64, &EscrowStatus::Released);

    let result = client.try_transition_status(&1u64, &EscrowStatus::Pending);
    assert!(result.is_err());
}

#[test]
fn test_released_to_disputed_rejected() {
    let (env, client) = setup();
    let (escrow, buyer, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    env.mock_auths(&[&buyer]);
    client.transition_status(&1u64, &EscrowStatus::Released);

    let result = client.try_transition_status(&1u64, &EscrowStatus::Disputed);
    assert!(result.is_err());
}

#[test]
fn test_refunded_to_pending_rejected() {
    let (env, client) = setup();
    let (escrow, buyer, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    env.mock_auths(&[&buyer]);
    client.transition_status(&1u64, &EscrowStatus::Refunded);

    let result = client.try_transition_status(&1u64, &EscrowStatus::Pending);
    assert!(result.is_err());
}

#[test]
fn test_refunded_to_disputed_rejected() {
    let (env, client) = setup();
    let (escrow, buyer, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    env.mock_auths(&[&buyer]);
    client.transition_status(&1u64, &EscrowStatus::Refunded);

    let result = client.try_transition_status(&1u64, &EscrowStatus::Disputed);
    assert!(result.is_err());
}

#[test]
fn test_refunded_to_released_rejected() {
    let (env, client) = setup();
    let (escrow, buyer, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    env.mock_auths(&[&buyer]);
    client.transition_status(&1u64, &EscrowStatus::Refunded);

    let result = client.try_transition_status(&1u64, &EscrowStatus::Released);
    assert!(result.is_err());
}

#[test]
fn test_released_to_refunded_rejected() {
    let (env, client) = setup();
    let (escrow, buyer, _, _) = make_escrow(&env);
    client.store_escrow(&1u64, &escrow);

    env.mock_auths(&[&buyer]);
    client.transition_status(&1u64, &EscrowStatus::Released);

    let result = client.try_transition_status(&1u64, &EscrowStatus::Refunded);
    assert!(result.is_err());
}

#[test]
fn test_refund_escrow_third_party_rejected() {
    let env = Env::default();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let third_party = Address::generate(&env);

    let (client, token_id, _) = setup_with_token(&env, 5_000_000, &buyer);

    let escrow = Escrow {
        buyer: buyer.clone(),
        seller: seller.clone(),
        token: token_id.clone(),
        amount: 5_000_000,
        status: EscrowStatus::Pending,
    };
    client.store_escrow(&1u64, &escrow);

    env.mock_all_auths();
    let result = client.try_refund_escrow(&1u64, &third_party);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn test_refund_escrow_terminal_state_rejected() {
    let (env, client) = setup();
    let (mut escrow, buyer, _, _) = make_escrow(&env);
    escrow.status = EscrowStatus::Released;
    client.store_escrow(&1u64, &escrow);

    let result = client.try_refund_escrow(&1u64, &buyer);
    assert_eq!(result, Err(Ok(ContractError::InvalidTransition)));
}

#[test]
fn test_refund_escrow_not_found() {
    let (env, client) = setup();
    let buyer = Address::generate(&env);

    let result = client.try_refund_escrow(&99u64, &buyer);
    assert_eq!(result, Err(Ok(ContractError::EscrowNotFound)));
}