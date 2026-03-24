#![cfg(test)]
extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Bytes, Env};

use crate::errors::ContractError;
// MAX_METADATA_SIZE was warned as unused, but it's used later. Keep it.
use crate::types::MAX_METADATA_SIZE;
use crate::{Contract, ContractClient};

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

// #[test]
// #[should_panic(expected = "NotAdmin")]
// fn non_admin_cannot_pause() {
//     // TODO: Update to use MockAuth for non-admin auth failure check in Soroban SDK v25
// }

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
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    let id1 = client.create_escrow(&Address::generate(&env), &seller, &token, &1000, &None);
    let id2 = client.create_escrow(&Address::generate(&env), &seller, &token, &2000, &None);
    let id3 = client.create_escrow(&Address::generate(&env), &seller, &token, &3000, &None);

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

    let mut ids = std::vec::Vec::new();

    for _ in 0..10 {
        let buyer_mock = Address::generate(&env);
        let id = client.create_escrow(&buyer_mock, &seller, &token, &100, &None);
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

    // force counter to max
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&crate::types::DataKey::EscrowCounter, &u64::MAX);
    });

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
    assert_eq!(escrow.metadata, Some(metadata.clone()));

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
    let oversized_data = std::vec![0u8; (MAX_METADATA_SIZE + 1) as usize];
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
    let max_data = std::vec![0u8; MAX_METADATA_SIZE as usize];
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

// =========================
// ANALYTICS TESTS
// =========================

#[test]
fn test_analytics_aggregation() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &250);

    // Initially zero
    assert_eq!(client.get_total_escrows(), 0);
    assert_eq!(client.get_total_funded_amount(), 0);

    // Create some escrows
    client.create_escrow(&buyer, &seller, &token, &1000, &None);
    client.create_escrow(
        &buyer,
        &seller,
        &token,
        &2500,
        &Some(Bytes::from_slice(&env, b"meta1")),
    );
    client.create_escrow(
        &buyer,
        &seller,
        &token,
        &500,
        &Some(Bytes::from_slice(&env, b"meta2")),
    );

    // Verify analytics
    assert_eq!(client.get_total_escrows(), 3);
    assert_eq!(client.get_total_funded_amount(), 4000);
}


#[test]
fn buyer_can_release_escrow() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    // Register a mock token contract
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());
    let token = soroban_sdk::token::Client::new(&env, &token_id.address());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    // Fund the contract so it can pay out
    token_admin.mint(&client.address, &1000);

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None);

    client.release_escrow(&escrow_id);

    // Seller received funds
    assert_eq!(token.balance(&seller), 1000);

    // Status updated
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

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None);

    // First release succeeds
    // (skipping token setup here — just testing state guard)
    // Force status to Released directly
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

    // Mint tokens to buyer
    token_admin.mint(&buyer, &1000);
    assert_eq!(token.balance(&buyer), 1000);

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None);

    client.fund_escrow(&escrow_id);

    // Buyer balance drained, contract holds the funds
    assert_eq!(token.balance(&buyer), 0);
    assert_eq!(token.balance(&client.address), 1000);

    // Status remains Pending
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

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None);

    // Force status to Released
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
    // Intentionally do NOT mint any tokens to buyer

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None);

    // Should panic/revert because buyer has 0 balance
    let result = client.try_fund_escrow(&escrow_id);
    assert!(result.is_err());
}