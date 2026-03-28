#![cfg(test)]
extern crate std;

use arbitrary::{Arbitrary, Unstructured};
use proptest::prelude::*;
use soroban_sdk::{testutils::Address as _, Address, Bytes, Env};
use soroban_sdk::{
    testutils::{storage::Persistent as _, Address as _, Events as _},
    Address, Bytes, Env, Event,
};

use crate::errors::ContractError;
use crate::types::MAX_METADATA_SIZE;
use crate::{Contract, ContractClient, EscrowCreatedEvent, FundsReleasedEvent, StatusChangeEvent};

fn setup<'a>() -> (Env, ContractClient<'a>) {
    let env = Env::default();
    let contract_id = env.register(Contract, ());
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

    client.pause();
    assert!(client.is_paused());

    client.unpause();
    assert!(!client.is_paused());
}

#[test]
fn escrow_actions_blocked_when_paused() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let collector = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &collector, &250);
    client.pause();

    let result = client.try_fund_escrow(&1u64);
    assert_eq!(result, Err(Ok(ContractError::ContractPaused)));
}

#[test]
fn escrow_ids_increment_sequentially() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let id1 = client.create_escrow(
        &Address::generate(&env),
        &seller,
        &token,
        &1000,
        &None,
        &None,
    );
    let id2 = client.create_escrow(
        &Address::generate(&env),
        &seller,
        &token,
        &2000,
        &None,
        &None,
    );
    let id3 = client.create_escrow(
        &Address::generate(&env),
        &seller,
        &token,
        &3000,
        &None,
        &None,
    );

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[test]
fn no_escrow_id_collision() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let mut ids = std::vec::Vec::new();

    for _ in 0..10 {
        let buyer_mock = Address::generate(&env);
        let id = client.create_escrow(&buyer_mock, &seller, &token, &100, &None, &None);
        assert!(!ids.contains(&id));
        ids.push(id);
    }
}

#[test]
fn escrow_counter_overflow_fails() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&crate::types::DataKey::EscrowCounter, &u64::MAX);
    });

    let result = client.try_create_escrow(&buyer, &seller, &token, &100, &None, &None);
    assert_eq!(result, Err(Ok(ContractError::EscrowIdOverflow)));
}

#[test]
fn test_metadata_stored_successfully() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let metadata = Bytes::from_slice(&env, b"order_ref:12345");
    let metadata_opt = Some(metadata.clone());

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &metadata_opt, &None);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.metadata, Some(metadata.clone()));

    let retrieved_metadata = client.get_escrow_metadata(&escrow_id).unwrap();
    assert_eq!(retrieved_metadata, metadata);
}

#[test]
fn test_metadata_none_stored_successfully() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None, &None);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.metadata, None);

    let retrieved_metadata = client.get_escrow_metadata(&escrow_id);
    assert_eq!(retrieved_metadata, None);
}

#[test]
fn test_oversized_metadata_rejected() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let oversized_data = std::vec![0u8; (MAX_METADATA_SIZE + 1) as usize];
    let oversized_metadata = Some(Bytes::from_slice(&env, &oversized_data));

    let result =
        client.try_create_escrow(&buyer, &seller, &token, &1000, &oversized_metadata, &None);
    assert_eq!(result, Err(Ok(ContractError::MetadataTooLarge)));
}

#[test]
fn test_metadata_at_max_size_accepted() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let max_data = std::vec![0u8; MAX_METADATA_SIZE as usize];
    let max_metadata = Some(Bytes::from_slice(&env, &max_data));

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &max_metadata, &None);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert!(escrow.metadata.is_some());
}

#[test]
fn test_get_escrow_metadata_for_nonexistent_escrow() {
    let (env, client) = setup();
    let admin = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let metadata = client.get_escrow_metadata(&999u64);
    assert_eq!(metadata, None);
}

#[test]
fn test_duplicate_escrow_rejected() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let metadata = Some(Bytes::from_slice(&env, b"order_ref:12345"));

    let escrow_id1 = client.create_escrow(&buyer, &seller, &token, &1000, &metadata, &None);
    assert_eq!(escrow_id1, 1);

    let result = client.try_create_escrow(&buyer, &seller, &token, &2000, &metadata, &None);
    assert_eq!(result, Err(Ok(ContractError::DuplicateEscrow)));
}

#[test]
fn test_distinct_escrows_allowed() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let metadata1 = Some(Bytes::from_slice(&env, b"order_ref:12345"));
    let escrow_id1 = client.create_escrow(&buyer, &seller, &token, &1000, &metadata1, &None);
    assert_eq!(escrow_id1, 1);

    let metadata2 = Some(Bytes::from_slice(&env, b"order_ref:67890"));
    let escrow_id2 = client.create_escrow(&buyer, &seller, &token, &2000, &metadata2, &None);
    assert_eq!(escrow_id2, 2);

    let escrow_id3 = client.create_escrow(&buyer, &seller, &token, &3000, &None, &None);
    assert_eq!(escrow_id3, 3);

    let buyer2 = Address::generate(&env);
    let escrow_id4 = client.create_escrow(&buyer2, &seller, &token, &4000, &metadata1, &None);
    assert_eq!(escrow_id4, 4);

    let seller2 = Address::generate(&env);
    let escrow_id5 = client.create_escrow(&buyer, &seller2, &token, &5000, &metadata1, &None);
    assert_eq!(escrow_id5, 5);
}

#[test]
fn test_duplicate_escrow_with_none_metadata() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let escrow_id1 = client.create_escrow(&buyer, &seller, &token, &1000, &None, &None);
    assert_eq!(escrow_id1, 1);

    let result = client.try_create_escrow(&buyer, &seller, &token, &2000, &None, &None);
    assert_eq!(result, Err(Ok(ContractError::DuplicateEscrow)));
}

#[test]
fn test_escrow_hash_stored_correctly() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let metadata = Some(Bytes::from_slice(&env, b"order_ref:unique_hash_test"));

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &metadata, &None);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.buyer, buyer);
    assert_eq!(escrow.seller, seller);
    assert_eq!(escrow.metadata, metadata);
}

#[test]
fn test_analytics_aggregation() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    assert_eq!(client.get_total_escrows(), 0);
    assert_eq!(client.get_total_funded_amount(), 0);

    client.create_escrow(&buyer, &seller, &token, &1000, &None, &None);
    client.create_escrow(
        &buyer,
        &seller,
        &token,
        &2500,
        &Some(Bytes::from_slice(&env, b"meta1")),
        &None,
    );
    client.create_escrow(
        &buyer,
        &seller,
        &token,
        &500,
        &Some(Bytes::from_slice(&env, b"meta2")),
        &None,
    );

    assert_eq!(client.get_total_escrows(), 3);
    assert_eq!(client.get_total_funded_amount(), 4000);
}

#[test]
fn buyer_can_release_escrow() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());
    let token = soroban_sdk::token::Client::new(&env, &token_id.address());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    token_admin.mint(&client.address, &1000);

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None, &None);

    client.release_escrow(&escrow_id);

    assert_eq!(token.balance(&seller), 1000);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.status, crate::types::EscrowStatus::Released);
}

#[test]
fn release_fails_if_not_pending() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None, &None);

    env.as_contract(&client.address, || {
        let mut escrow: crate::types::Escrow = env
            .storage()
            .persistent()
            .get(&crate::types::DataKey::Escrow(escrow_id))
            .unwrap();
        escrow.status = crate::types::EscrowStatus::Released;
        env.storage()
            .persistent()
            .set(&crate::types::DataKey::Escrow(escrow_id), &escrow);
    });

    let result = client.try_release_escrow(&escrow_id);
    assert_eq!(result, Err(Ok(ContractError::InvalidEscrowState)));
}

#[test]
fn release_fails_for_nonexistent_escrow() {
    let (env, client) = setup();
    let admin = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let result = client.try_release_escrow(&999u64);
    assert_eq!(result, Err(Ok(ContractError::EscrowNotFound)));
}

#[test]
fn buyer_can_fund_escrow() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());
    let token = soroban_sdk::token::Client::new(&env, &token_id.address());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    token_admin.mint(&buyer, &1000);
    assert_eq!(token.balance(&buyer), 1000);

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None, &None);

    client.fund_escrow(&escrow_id);

    assert_eq!(token.balance(&buyer), 0);
    assert_eq!(token.balance(&client.address), 1000);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.status, crate::types::EscrowStatus::Pending);
}

#[test]
fn fund_fails_if_not_pending() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);
    token_admin.mint(&buyer, &1000);

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None, &None);

    env.as_contract(&client.address, || {
        let mut escrow: crate::types::Escrow = env
            .storage()
            .persistent()
            .get(&crate::types::DataKey::Escrow(escrow_id))
            .unwrap();
        escrow.status = crate::types::EscrowStatus::Released;
        env.storage()
            .persistent()
            .set(&crate::types::DataKey::Escrow(escrow_id), &escrow);
    });

    let result = client.try_fund_escrow(&escrow_id);
    assert_eq!(result, Err(Ok(ContractError::InvalidEscrowState)));
}

#[test]
fn fund_fails_for_nonexistent_escrow() {
    let (env, client) = setup();
    let admin = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let result = client.try_fund_escrow(&999u64);
    assert_eq!(result, Err(Ok(ContractError::EscrowNotFound)));
}

#[test]
fn fund_fails_if_buyer_has_insufficient_balance() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None, &None);

    let result = client.try_fund_escrow(&escrow_id);
    assert!(result.is_err());
}

// =========================
// ARBITER TESTS
// =========================

#[test]
fn test_create_escrow_stores_arbiter() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);
    let arbiter = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &token,
        &1000,
        &None,
        &Some(arbiter.clone()),
    );

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.arbiter, Some(arbiter));
}

#[test]
fn test_create_escrow_without_arbiter_stores_none() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None, &None);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.arbiter, None);
}

#[test]
fn test_create_escrow_emits_indexable_event() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);
    let arbiter = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &token,
        &1000,
        &Some(Bytes::from_slice(&env, b"order_ref:indexable")),
        &Some(arbiter.clone()),
    );

    let events = env.events().all().filter_by_contract(&client.address);
    let emitted = events.events().to_vec();
    let expected = EscrowCreatedEvent {
        escrow_id,
        buyer,
        seller,
        token,
        amount: 1000,
        status: crate::types::EscrowStatus::Pending,
        arbiter: Some(arbiter),
    };

    assert_eq!(emitted, std::vec![expected.to_xdr(&env, &client.address)]);
}

#[test]
fn test_arbiter_can_resolve_dispute() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let arbiter = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());
    let token = soroban_sdk::token::Client::new(&env, &token_id.address());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    token_admin.mint(&client.address, &1000);

    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &token_id.address(),
        &1000,
        &None,
        &Some(arbiter.clone()),
    );

    env.as_contract(&client.address, || {
        let mut escrow: crate::types::Escrow = env
            .storage()
            .persistent()
            .get(&crate::types::DataKey::Escrow(escrow_id))
            .unwrap();
        escrow.status = crate::types::EscrowStatus::Disputed;
        env.storage()
            .persistent()
            .set(&crate::types::DataKey::Escrow(escrow_id), &escrow);
    });

    client.resolve_dispute(&escrow_id, &true);

    assert_eq!(token.balance(&seller), 1000);
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.status, crate::types::EscrowStatus::Released);
}

#[test]
fn test_release_emits_funds_and_status_change_events() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);
    token_admin.mint(&client.address, &1000);

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None, &None);
    client.release_escrow(&escrow_id);

    let events = env.events().all().filter_by_contract(&client.address);
    let emitted = events.events().to_vec();
    let expected_release = FundsReleasedEvent {
        escrow_id,
        amount: 1000,
    };
    let expected_status = StatusChangeEvent {
        escrow_id,
        from_status: crate::types::EscrowStatus::Pending,
        to_status: crate::types::EscrowStatus::Released,
        actor: buyer,
    };

    assert_eq!(
        emitted,
        std::vec![
            expected_release.to_xdr(&env, &client.address),
            expected_status.to_xdr(&env, &client.address),
        ]
    );
}

#[test]
fn test_arbiter_can_refund_buyer_on_dispute() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let arbiter = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());
    let token = soroban_sdk::token::Client::new(&env, &token_id.address());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    token_admin.mint(&client.address, &1000);

    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &token_id.address(),
        &1000,
        &None,
        &Some(arbiter.clone()),
    );

    env.as_contract(&client.address, || {
        let mut escrow: crate::types::Escrow = env
            .storage()
            .persistent()
            .get(&crate::types::DataKey::Escrow(escrow_id))
            .unwrap();
        escrow.status = crate::types::EscrowStatus::Disputed;
        env.storage()
            .persistent()
            .set(&crate::types::DataKey::Escrow(escrow_id), &escrow);
    });

    client.resolve_dispute(&escrow_id, &false);

    assert_eq!(token.balance(&buyer), 1000);
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.status, crate::types::EscrowStatus::Refunded);
}

#[test]
fn test_resolve_dispute_emits_status_change_with_actor() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let arbiter = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);
    token_admin.mint(&client.address, &1000);

    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &token_id.address(),
        &1000,
        &None,
        &Some(arbiter.clone()),
    );

    env.as_contract(&client.address, || {
        let mut escrow: crate::types::Escrow = env
            .storage()
            .persistent()
            .get(&crate::types::DataKey::Escrow(escrow_id))
            .unwrap();
        escrow.status = crate::types::EscrowStatus::Disputed;
        env.storage()
            .persistent()
            .set(&crate::types::DataKey::Escrow(escrow_id), &escrow);
    });

    client.resolve_dispute(&escrow_id, &1u32);

    let events = env.events().all().filter_by_contract(&client.address);
    let emitted = events.events().to_vec();
    let expected_status = StatusChangeEvent {
        escrow_id,
        from_status: crate::types::EscrowStatus::Disputed,
        to_status: crate::types::EscrowStatus::Refunded,
        actor: arbiter,
    };

    assert_eq!(
        emitted,
        std::vec![expected_status.to_xdr(&env, &client.address)]
    );
}

#[test]
fn test_bump_escrow_extends_ttl_to_maximum() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &token,
        &1000,
        &Some(Bytes::from_slice(&env, b"ttl-test")),
        &None,
    );

    let escrow_key = crate::types::DataKey::Escrow(escrow_id);
    let before_ttl = env.as_contract(&client.address, || {
        env.storage().persistent().get_ttl(&escrow_key)
    });

    client.bump_escrow(&escrow_id);

    let after_ttl = env.as_contract(&client.address, || {
        env.storage().persistent().get_ttl(&escrow_key)
    });

    assert!(after_ttl > before_ttl);
    assert_eq!(after_ttl, env.storage().max_ttl());
}

#[test]
fn test_bump_escrow_rejects_missing_escrow() {
    let (env, client) = setup();
    let admin = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let result = client.try_bump_escrow(&999u64);
    assert_eq!(result, Err(Ok(ContractError::EscrowNotFound)));
}

#[test]
fn test_resolve_dispute_fails_if_not_disputed() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let arbiter = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None, &Some(arbiter));

    let result = client.try_resolve_dispute(&escrow_id, &false);
    assert_eq!(result, Err(Ok(ContractError::InvalidEscrowState)));
}

#[test]
fn test_resolve_dispute_fails_for_nonexistent_escrow() {
    let (env, client) = setup();
    let admin = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let result = client.try_resolve_dispute(&999u64, &0u32);
    assert_eq!(result, Err(Ok(ContractError::EscrowNotFound)));
}

#[test]
fn test_buyer_can_request_refund() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);
    let amount = 1000i128;
    let evidence_hash = Bytes::from_array(&env, &[1, 2, 3, 4]);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    // Create escrow
    let escrow_id = client.create_escrow(&buyer, &seller, &token, &amount, &None, &None);

    // Request refund
    let request_id = client.refund_escrow(&escrow_id, &buyer, &amount, &crate::RefundReason::ProductNotReceived, &evidence_hash);

    // Verify refund request was created
    let refund_request = client.get_refund_request(&request_id).unwrap();
    assert_eq!(refund_request.escrow_id, escrow_id);
    assert_eq!(refund_request.requester, buyer);
    assert_eq!(refund_request.amount, amount);
    assert_eq!(refund_request.reason, crate::RefundReason::ProductNotReceived);
    assert_eq!(refund_request.status, crate::RefundStatus::Pending);

    // Verify escrow status changed to Disputed
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.status, crate::EscrowStatus::Disputed);
}

#[test]
fn test_refund_fails_for_nonexistent_escrow() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let evidence_hash = Bytes::from_array(&env, &[1, 2, 3, 4]);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let result = client.try_refund_escrow(&999, &buyer, &1000, &crate::RefundReason::ProductNotReceived, &evidence_hash);
    assert_eq!(result, Err(Ok(ContractError::EscrowNotFound)));
}

// =========================
// FUZZ / PROPERTY TESTS
// =========================

use arbitrary::{Arbitrary, Unstructured};
use proptest::prelude::*;

#[derive(Debug, Clone, Arbitrary)]
enum FuzzAction {
    Create,
    MintBuyer,
    Fund,
    MintContract,
    Release,
    RefundBuyer,
    ResolveDisputeSeller,
}

fn run_fuzz_actions(actions: &[FuzzAction]) {
    let (env, client) = setup();

    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let arbiter = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());
    let token = soroban_sdk::token::Client::new(&env, &token_id.address());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let mut escrow_id: Option<u64> = None;

    for action in actions {
        match action {
            FuzzAction::Create => {
                if escrow_id.is_none() {
                    let id = client.create_escrow(
                        &buyer,
                        &seller,
                        &token_id.address(),
                        &1000,
                        &None,
                        &Some(arbiter.clone()),
                    );
                    escrow_id = Some(id);
                }
            }
            FuzzAction::MintBuyer => {
                token_admin.mint(&buyer, &1000);
            }
            FuzzAction::Fund => {
                if let Some(id) = escrow_id {
                    let _ = client.try_fund_escrow(&id);
                }
            }
            FuzzAction::MintContract => {
                token_admin.mint(&client.address, &1000);
            }
            FuzzAction::Release => {
                if let Some(id) = escrow_id {
                    let _ = client.try_release_escrow(&id);
                }
            }
            FuzzAction::RefundBuyer => {
                if let Some(id) = escrow_id {
                    let evidence_hash = Bytes::from_array(&env, &[1, 2, 3, 4]);
                    let _ = client.try_refund_escrow(&id, &buyer, &1000, &crate::RefundReason::ProductNotReceived, &evidence_hash);
                }
            }
            FuzzAction::ResolveDisputeSeller => {
                if let Some(id) = escrow_id {
                    let _ = client.try_resolve_dispute(&id, &0u32);
                }
            }
        }

        if let Some(id) = escrow_id {
            let escrow = client.get_escrow(&id);

            if let Some(escrow) = escrow {
                assert!(escrow.amount >= 0);

                match escrow.status {
                    crate::types::EscrowStatus::Pending
                    | crate::types::EscrowStatus::Released
                    | crate::types::EscrowStatus::Refunded
                    | crate::types::EscrowStatus::Disputed => {}
                }

                assert!(token.balance(&buyer) >= 0);
                assert!(token.balance(&seller) >= 0);
                assert!(token.balance(&client.address) >= 0);
            }
        }
    }
}

proptest! {
    #[test]
    fn fuzz_escrow_lifecycle(actions in proptest::collection::vec(0u8..7, 1..25)) {
        let mapped: std::vec::Vec<FuzzAction> = actions
            .into_iter()
            .map(|action| match action {
                0 => FuzzAction::Create,
                1 => FuzzAction::MintBuyer,
                2 => FuzzAction::Fund,
                3 => FuzzAction::MintContract,
                4 => FuzzAction::Release,
                5 => FuzzAction::RefundBuyer,
                _ => FuzzAction::ResolveDisputeSeller,
            })
            .collect();

        run_fuzz_actions(&mapped);
    }
}

#[test]
fn arbitrary_fuzz_lifecycle() {
    let seeds: [&[u8]; 4] = [
        b"marketx-seed-1-create-fund-release",
        b"marketx-seed-2-create-fund-refund",
        b"marketx-seed-3-create-dispute-resolve",
        b"marketx-seed-4-mixed-lifecycle-edge-cases",
    ];

    for seed in seeds {
        let mut u = Unstructured::new(seed);
        let actions =
            std::vec::Vec::<FuzzAction>::arbitrary(&mut u).unwrap_or_else(|_| std::vec::Vec::new());

        run_fuzz_actions(&actions);
    }
}
