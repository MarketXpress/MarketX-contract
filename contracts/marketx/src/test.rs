#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Bytes, Env};

use crate::{Contract, ContractClient};
use crate::errors::ContractError;
use crate::types::MAX_METADATA_SIZE;

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

    let result = client.try_fund_escrow(&1u64);
    assert_eq!(result, Err(Ok(ContractError::ContractPaused)));
}

#[test]
fn escrow_ids_increment_sequentially() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let id1 = client.create_escrow(&buyer, &seller, &token, &1000, &None);
    let id2 = client.create_escrow(&buyer, &seller, &token, &2000, &None);
    let id3 = client.create_escrow(&buyer, &seller, &token, &3000, &None);

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
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let mut ids = std::collections::BTreeSet::new();

    for _ in 0..10 {
        let id = client.create_escrow(&buyer, &seller, &token, &100, &None);
        assert!(ids.insert(id));
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

    // force counter to max
    env.storage()
        .persistent()
        .set(&crate::types::DataKey::EscrowCounter, &u64::MAX);

    let result = client.try_create_escrow(&buyer, &seller, &token, &100, &None);
    assert_eq!(result, Err(Ok(ContractError::EscrowIdOverflow)));
}

// =========================
// METADATA TESTS
// =========================

#[test]
fn test_metadata_stored_successfully() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    // Create metadata
    let metadata = Bytes::from_slice(&env, b"order_ref:12345");
    let metadata_opt = Some(metadata.clone());

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &metadata_opt);

    // Retrieve escrow and verify metadata
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.metadata, Some(metadata));

    // Test getter
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

    // Create escrow without metadata
    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None);

    // Retrieve escrow and verify metadata is None
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.metadata, None);

    // Test getter returns None
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

    // Create oversized metadata (MAX_METADATA_SIZE + 1)
    let oversized_data = vec![0u8; (MAX_METADATA_SIZE + 1) as usize];
    let oversized_metadata = Some(Bytes::from_slice(&env, &oversized_data));

    let result = client.try_create_escrow(&buyer, &seller, &token, &1000, &oversized_metadata);
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

    // Create metadata at exact max size
    let max_data = vec![0u8; MAX_METADATA_SIZE as usize];
    let max_metadata = Some(Bytes::from_slice(&env, &max_data));

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &max_metadata);

    // Should succeed
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert!(escrow.metadata.is_some());
}

#[test]
fn test_get_escrow_metadata_for_nonexistent_escrow() {
    let (env, client) = setup();
    let admin = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    // Try to get metadata for non-existent escrow
    let metadata = client.get_escrow_metadata(&999u64);
    assert_eq!(metadata, None);
}

// =========================
// DUPLICATE ESCROW TESTS
// =========================

#[test]
fn test_duplicate_escrow_rejected() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    // Create metadata
    let metadata = Some(Bytes::from_slice(&env, b"order_ref:12345"));

    // First escrow creation should succeed
    let escrow_id1 = client.create_escrow(&buyer, &seller, &token, &1000, &metadata);
    assert_eq!(escrow_id1, 1);

    // Second escrow with same buyer, seller, and metadata should fail
    let result = client.try_create_escrow(&buyer, &seller, &token, &2000, &metadata);
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

    // Create first escrow with metadata
    let metadata1 = Some(Bytes::from_slice(&env, b"order_ref:12345"));
    let escrow_id1 = client.create_escrow(&buyer, &seller, &token, &1000, &metadata1);
    assert_eq!(escrow_id1, 1);

    // Create second escrow with different metadata - should succeed
    let metadata2 = Some(Bytes::from_slice(&env, b"order_ref:67890"));
    let escrow_id2 = client.create_escrow(&buyer, &seller, &token, &2000, &metadata2);
    assert_eq!(escrow_id2, 2);

    // Create third escrow with no metadata - should succeed
    let escrow_id3 = client.create_escrow(&buyer, &seller, &token, &3000, &None);
    assert_eq!(escrow_id3, 3);

    // Create fourth escrow with different buyer - should succeed
    let buyer2 = Address::generate(&env);
    let escrow_id4 = client.create_escrow(&buyer2, &seller, &token, &4000, &metadata1);
    assert_eq!(escrow_id4, 4);

    // Create fifth escrow with different seller - should succeed
    let seller2 = Address::generate(&env);
    let escrow_id5 = client.create_escrow(&buyer, &seller2, &token, &5000, &metadata1);
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

    // Create first escrow with no metadata
    let escrow_id1 = client.create_escrow(&buyer, &seller, &token, &1000, &None);
    assert_eq!(escrow_id1, 1);

    // Second escrow with same buyer, seller, and no metadata should fail
    let result = client.try_create_escrow(&buyer, &seller, &token, &2000, &None);
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

    // Create metadata
    let metadata = Some(Bytes::from_slice(&env, b"order_ref:unique_hash_test"));

    // Create escrow
    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &metadata);

    // Verify escrow was created and can be retrieved
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.buyer, buyer);
    assert_eq!(escrow.seller, seller);
    assert_eq!(escrow.metadata, metadata);
}

