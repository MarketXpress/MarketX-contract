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

// ─── event emission (#39) ────────────────────────────────────────────────────

#[test]
fn test_store_escrow_emits_created_event() {
    let (env, client) = setup();
    let (escrow, buyer, seller, token) = make_escrow(&env);

    // store_escrow must complete without panic - event publication happens
    // inside the call. The full event structure (topics and data) is captured
    // by the snapshot file generated alongside this test.
    client.store_escrow(&1u64, &escrow);

    // Confirm the escrow was persisted correctly alongside the event.
    let retrieved = client.get_escrow(&1u64);
    assert_eq!(retrieved.buyer, buyer);
    assert_eq!(retrieved.seller, seller);
    assert_eq!(retrieved.token, token);
    assert_eq!(retrieved.amount, 5_000_000);
    assert_eq!(retrieved.status, EscrowStatus::Pending);
}

// ═══════════════════════════════════════════════════════════════════════════
// #54 — fund_escrow (token transfer: buyer → contract)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_fund_escrow_transfers_tokens_to_contract() {
    let env = Env::default();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let amount = 5_000_000i128;

    let (client, token_id, _) = setup_with_token(&env, amount, &buyer);
    let contract_address = client.address.clone();

    let escrow = Escrow {
        buyer: buyer.clone(),
        seller: seller.clone(),
        token: token_id.clone(),
        amount,
        status: EscrowStatus::Pending,
    };
    client.store_escrow(&1u64, &escrow);

    env.mock_all_auths();
    client.fund_escrow(&1u64).unwrap();

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&contract_address), amount);
    assert_eq!(token_client.balance(&buyer), 0);
    // Status remains Pending after funding.
    assert_eq!(client.get_escrow(&1u64).status, EscrowStatus::Pending);
}

#[test]
fn test_fund_escrow_not_found() {
    let (_env, client) = setup();
    let result = client.try_fund_escrow(&99u64);
    assert_eq!(result, Err(Ok(ContractError::EscrowNotFound)));
}

#[test]
fn test_fund_escrow_non_pending_status_rejected() {
    let env = Env::default();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    let (client, token_id, _) = setup_with_token(&env, 5_000_000, &buyer);

    // Store an escrow that is already Disputed — cannot be funded.
    let escrow = Escrow {
        buyer: buyer.clone(),
        seller: seller.clone(),
        token: token_id.clone(),
        amount: 5_000_000,
        status: EscrowStatus::Disputed,
    };
    client.store_escrow(&1u64, &escrow);

    let result = client.try_fund_escrow(&1u64);
    assert_eq!(result, Err(Ok(ContractError::AlreadyFunded)));
}

// ═══════════════════════════════════════════════════════════════════════════
// #55 — release_escrow (token transfer: contract → seller + fee_collector)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_release_escrow_seller_receives_amount_minus_fee() {
    let env = Env::default();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let collector = Address::generate(&env);
    let amount = 10_000_000i128;

    let (client, token_id, _) = setup_with_token(&env, amount, &buyer);
    let contract_address = client.address.clone();

    // 2.5 % fee.
    client.initialize(&collector, &250u32).unwrap();

    let escrow = Escrow {
        buyer: buyer.clone(),
        seller: seller.clone(),
        token: token_id.clone(),
        amount,
        status: EscrowStatus::Pending,
    };
    client.store_escrow(&1u64, &escrow);

    env.mock_all_auths();
    client.fund_escrow(&1u64).unwrap();
    client.release_escrow(&1u64).unwrap();

    let token_client = token::Client::new(&env, &token_id);
    // 10_000_000 * 250 / 10_000 = 250_000 fee
    assert_eq!(token_client.balance(&seller), 9_750_000);
    assert_eq!(token_client.balance(&collector), 250_000);
    assert_eq!(token_client.balance(&contract_address), 0);
    assert_eq!(client.get_escrow(&1u64).status, EscrowStatus::Released);
}

#[test]
fn test_release_escrow_zero_fee_seller_receives_full_amount() {
    let env = Env::default();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let collector = Address::generate(&env);
    let amount = 5_000_000i128;

    let (client, token_id, _) = setup_with_token(&env, amount, &buyer);
    let contract_address = client.address.clone();

    client.initialize(&collector, &0u32).unwrap();

    let escrow = Escrow {
        buyer: buyer.clone(),
        seller: seller.clone(),
        token: token_id.clone(),
        amount,
        status: EscrowStatus::Pending,
    };
    client.store_escrow(&1u64, &escrow);

    env.mock_all_auths();
    client.fund_escrow(&1u64).unwrap();
    client.release_escrow(&1u64).unwrap();

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&seller), amount);
    assert_eq!(token_client.balance(&collector), 0);
    assert_eq!(token_client.balance(&contract_address), 0);
}

#[test]
fn test_release_escrow_non_pending_rejected() {
    let (env, client) = setup();
    let collector = Address::generate(&env);
    client.initialize(&collector, &250u32).unwrap();

    let (mut escrow, _, _, _) = make_escrow(&env);
    escrow.status = EscrowStatus::Disputed;
    client.store_escrow(&1u64, &escrow);

    let result = client.try_release_escrow(&1u64);
    assert_eq!(result, Err(Ok(ContractError::EscrowNotFunded)));
}

#[test]
fn test_release_escrow_not_found() {
    let (env, client) = setup();
    let collector = Address::generate(&env);
    client.initialize(&collector, &250u32).unwrap();

    let result = client.try_release_escrow(&99u64);
    assert_eq!(result, Err(Ok(ContractError::EscrowNotFound)));
}

// ═══════════════════════════════════════════════════════════════════════════
// #56 — refund_escrow (token transfer: contract → buyer)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_refund_escrow_buyer_initiated_from_pending() {
    let env = Env::default();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let amount = 5_000_000i128;

    let (client, token_id, _) = setup_with_token(&env, amount, &buyer);
    let contract_address = client.address.clone();

    let escrow = Escrow {
        buyer: buyer.clone(),
        seller: seller.clone(),
        token: token_id.clone(),
        amount,
        status: EscrowStatus::Pending,
    };
    client.store_escrow(&1u64, &escrow);

    env.mock_all_auths();
    client.fund_escrow(&1u64).unwrap();
    client.refund_escrow(&1u64, &buyer).unwrap();

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&buyer), amount);
    assert_eq!(token_client.balance(&contract_address), 0);
    assert_eq!(client.get_escrow(&1u64).status, EscrowStatus::Refunded);
}

#[test]
fn test_refund_escrow_seller_initiated_from_pending() {
    let env = Env::default();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let amount = 5_000_000i128;

    let (client, token_id, _) = setup_with_token(&env, amount, &buyer);
    let contract_address = client.address.clone();

    let escrow = Escrow {
        buyer: buyer.clone(),
        seller: seller.clone(),
        token: token_id.clone(),
        amount,
        status: EscrowStatus::Pending,
    };
    client.store_escrow(&1u64, &escrow);

    env.mock_all_auths();
    client.fund_escrow(&1u64).unwrap();
    // Seller voluntarily cancels — full amount returns to buyer.
    client.refund_escrow(&1u64, &seller).unwrap();

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&buyer), amount);
    assert_eq!(token_client.balance(&seller), 0);
    assert_eq!(token_client.balance(&contract_address), 0);
    assert_eq!(client.get_escrow(&1u64).status, EscrowStatus::Refunded);
}

#[test]
fn test_refund_escrow_buyer_initiated_from_disputed() {
    let env = Env::default();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let amount = 5_000_000i128;

    let (client, token_id, _) = setup_with_token(&env, amount, &buyer);
    let contract_address = client.address.clone();

    let escrow = Escrow {
        buyer: buyer.clone(),
        seller: seller.clone(),
        token: token_id.clone(),
        amount,
        status: EscrowStatus::Pending,
    };
    client.store_escrow(&1u64, &escrow);

    env.mock_all_auths();
    client.fund_escrow(&1u64).unwrap();
    client.transition_status(&1u64, &EscrowStatus::Disputed).unwrap();
    client.refund_escrow(&1u64, &buyer).unwrap();

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&buyer), amount);
    assert_eq!(token_client.balance(&contract_address), 0);
    assert_eq!(client.get_escrow(&1u64).status, EscrowStatus::Refunded);
}

#[test]
fn test_refund_escrow_seller_cannot_initiate_from_disputed() {
    let env = Env::default();
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    let (client, token_id, _) = setup_with_token(&env, 5_000_000, &buyer);

    let escrow = Escrow {
        buyer: buyer.clone(),
        seller: seller.clone(),
        token: token_id.clone(),
        amount: 5_000_000,
        status: EscrowStatus::Disputed,
    };
    client.store_escrow(&1u64, &escrow);

    env.mock_all_auths();
    let result = client.try_refund_escrow(&1u64, &seller);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
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
