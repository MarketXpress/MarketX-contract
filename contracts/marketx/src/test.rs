#![cfg(test)]
extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Bytes, Env};

use crate::errors::ContractError;
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

    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&crate::types::DataKey::EscrowCounter, &u64::MAX);
    });

    let result = client.try_create_escrow(&buyer, &seller, &token, &100, &None);
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

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &metadata_opt);

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

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None);

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

    let max_data = std::vec![0u8; MAX_METADATA_SIZE as usize];
    let max_metadata = Some(Bytes::from_slice(&env, &max_data));

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &max_metadata);

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

    let escrow_id1 = client.create_escrow(&buyer, &seller, &token, &1000, &metadata);
    assert_eq!(escrow_id1, 1);

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

    let metadata1 = Some(Bytes::from_slice(&env, b"order_ref:12345"));
    let escrow_id1 = client.create_escrow(&buyer, &seller, &token, &1000, &metadata1);
    assert_eq!(escrow_id1, 1);

    let metadata2 = Some(Bytes::from_slice(&env, b"order_ref:67890"));
    let escrow_id2 = client.create_escrow(&buyer, &seller, &token, &2000, &metadata2);
    assert_eq!(escrow_id2, 2);

    let escrow_id3 = client.create_escrow(&buyer, &seller, &token, &3000, &None);
    assert_eq!(escrow_id3, 3);

    let buyer2 = Address::generate(&env);
    let escrow_id4 = client.create_escrow(&buyer2, &seller, &token, &4000, &metadata1);
    assert_eq!(escrow_id4, 4);

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

    let escrow_id1 = client.create_escrow(&buyer, &seller, &token, &1000, &None);
    assert_eq!(escrow_id1, 1);

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

    let metadata = Some(Bytes::from_slice(&env, b"order_ref:unique_hash_test"));

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &metadata);

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

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None);

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

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None);

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

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None);

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

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None);

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

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None);

    let result = client.try_fund_escrow(&escrow_id);
    assert!(result.is_err());
}

#[test]
fn buyer_can_open_refund_with_evidence() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None);

    let evidence_hash = Bytes::from_slice(&env, b"buyer-evidence-cid");
    let request_id = client.refund_escrow(
        &escrow_id,
        &buyer,
        &1000,
        &crate::types::RefundReason::ProductDefective,
        &evidence_hash,
    );

    let refund_request: crate::types::RefundRequest = env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .get(&crate::types::DataKey::RefundRequest(request_id))
            .unwrap()
    });

    assert_eq!(refund_request.escrow_id, escrow_id);
    assert_eq!(refund_request.requester, buyer);
    assert_eq!(refund_request.status, crate::types::RefundStatus::Pending);
    assert_eq!(refund_request.evidence_hash, Some(evidence_hash));
    assert_eq!(refund_request.counter_evidence_hash, None);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.status, crate::types::EscrowStatus::Disputed);
}

#[test]
fn seller_can_submit_counter_evidence() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None);

    let evidence_hash = Bytes::from_slice(&env, b"buyer-evidence");
    let request_id = client.refund_escrow(
        &escrow_id,
        &buyer,
        &1000,
        &crate::types::RefundReason::ProductNotReceived,
        &evidence_hash,
    );

    let counter_hash = Bytes::from_slice(&env, b"seller-counter-evidence");
    client.submit_counter_evidence(&request_id, &seller, &counter_hash);

    let refund_request: crate::types::RefundRequest = env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .get(&crate::types::DataKey::RefundRequest(request_id))
            .unwrap()
    });

    assert_eq!(refund_request.counter_evidence_hash, Some(counter_hash));
}

#[test]
fn non_buyer_cannot_open_refund() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let stranger = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None);

    let evidence_hash = Bytes::from_slice(&env, b"fake");
    let result = client.try_refund_escrow(
        &escrow_id,
        &stranger,
        &1000,
        &crate::types::RefundReason::Other,
        &evidence_hash,
    );

    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn non_seller_cannot_submit_counter_evidence() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let stranger = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None);

    let evidence_hash = Bytes::from_slice(&env, b"buyer-evidence");
    let request_id = client.refund_escrow(
        &escrow_id,
        &buyer,
        &1000,
        &crate::types::RefundReason::WrongProduct,
        &evidence_hash,
    );

    let counter_hash = Bytes::from_slice(&env, b"bad-counter");
    let result = client.try_submit_counter_evidence(&request_id, &stranger, &counter_hash);

    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}
