#![cfg(test)]
#![rustfmt::skip]
extern crate std;

use soroban_sdk::testutils::Events;
use soroban_sdk::{
    testutils::{storage::Persistent as _, Address as _, MockAuth, MockAuthInvoke},
    Address, Bytes, Env, Event, IntoVal, Vec,
};

use crate::errors::ContractError;
// MAX_METADATA_SIZE was warned as unused, but it's used later. Keep it.
use crate::types::{EscrowItem, MAX_METADATA_SIZE};
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
#[should_panic] // SDK panic message for auth trap
fn non_admin_cannot_pause() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let collector = Address::generate(&env);

    // Initialize the contract using MockAuth for the admin
    client
        .mock_auths(&[MockAuth {
            address: &admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "initialize",
                args: (&admin, &collector, 250u32).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .initialize(&admin, &collector, &250);

    // Call pause as non_admin, which expects admin auth.
    // This should trap, because admin.require_auth() fails.
    client
        .mock_auths(&[MockAuth {
            address: &non_admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "pause",
                args: ().into_val(&env),
                sub_invokes: &[],
            },
        }])
        .pause();
}

#[test]
fn admin_rotation_flow() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let collector = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &collector, &250);

    // Transfer and accept admin
    client.transfer_admin(&new_admin);
    client.accept_admin();

    // Verify new admin is active
    assert_eq!(client.get_admin().unwrap(), new_admin);
}

#[test]
fn accept_admin_fails_if_none_proposed() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let collector = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &collector, &250);

    // Attempt to accept without any proposal
    let result = client.try_accept_admin();
    assert_eq!(result, Err(Ok(ContractError::NotProposedAdmin)));
}

#[test]
#[should_panic]
fn transfer_admin_fails_if_not_admin() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let not_admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let collector = Address::generate(&env);

    client
        .mock_auths(&[MockAuth {
            address: &admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "initialize",
                args: (&admin, &collector, 250u32).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .initialize(&admin, &collector, &250);

    // Attempt to transfer as not_admin. It should trap since admin.require_auth() fails.
    client
        .mock_auths(&[MockAuth {
            address: &not_admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "transfer_admin",
                args: (&new_admin,).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .transfer_admin(&new_admin);
}

#[test]
#[should_panic]
fn accept_admin_fails_if_unauthorized() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let not_proposed = Address::generate(&env);
    let collector = Address::generate(&env);

    client
        .mock_auths(&[MockAuth {
            address: &admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "initialize",
                args: (&admin, &collector, 250u32).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .initialize(&admin, &collector, &250);

    client
        .mock_auths(&[MockAuth {
            address: &admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "transfer_admin",
                args: (&new_admin,).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .transfer_admin(&new_admin);

    // Attempt to accept with the wrong person mocked
    client
        .mock_auths(&[MockAuth {
            address: &not_proposed,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "accept_admin",
                args: ().into_val(&env),
                sub_invokes: &[],
            },
        }])
        .accept_admin();
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
        &None,
    );
    let id2 = client.create_escrow(
        &Address::generate(&env),
        &seller,
        &token,
        &2000,
        &None,
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
        let id = client.create_escrow(&buyer_mock, &seller, &token, &100, &None, &None, &None);
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

    let result = client.try_create_escrow(&buyer, &seller, &token, &100, &None, &None, &None);
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

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &metadata_opt, &None, &None);

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

    // Create escrow without metadata
    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None, &None, &None);

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
        client.try_create_escrow(&buyer, &seller, &token, &1000, &oversized_metadata, &None, &None);
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

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &max_metadata, &None, &None);

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

    // First escrow creation should succeed
    let escrow_id1 = client.create_escrow(&buyer, &seller, &token, &1000, &metadata, &None, &None);
    assert_eq!(escrow_id1, 1);

    // Second escrow with same buyer, seller, and metadata should fail
    let result = client.try_create_escrow(&buyer, &seller, &token, &2000, &metadata, &None, &None);
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
    let escrow_id1 = client.create_escrow(&buyer, &seller, &token, &1000, &metadata1, &None, &None);
    assert_eq!(escrow_id1, 1);

    let metadata2 = Some(Bytes::from_slice(&env, b"order_ref:67890"));
    let escrow_id2 = client.create_escrow(&buyer, &seller, &token, &2000, &metadata2, &None, &None);
    assert_eq!(escrow_id2, 2);

    // Create third escrow with no metadata - should succeed
    let escrow_id3 = client.create_escrow(&buyer, &seller, &token, &3000, &None, &None, &None);
    assert_eq!(escrow_id3, 3);

    let buyer2 = Address::generate(&env);
    let escrow_id4 = client.create_escrow(&buyer2, &seller, &token, &4000, &metadata1, &None, &None);
    assert_eq!(escrow_id4, 4);

    let seller2 = Address::generate(&env);
    let escrow_id5 = client.create_escrow(&buyer, &seller2, &token, &5000, &metadata1, &None, &None);
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
    let escrow_id1 = client.create_escrow(&buyer, &seller, &token, &1000, &None, &None, &None);
    assert_eq!(escrow_id1, 1);

    // Second escrow with same buyer, seller, and no metadata should fail
    let result = client.try_create_escrow(&buyer, &seller, &token, &2000, &None, &None, &None);
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

    // Create escrow
    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &metadata, &None, &None);

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

    // Create some escrows
    client.create_escrow(&buyer, &seller, &token, &1000, &None, &None, &None);
    client.create_escrow(
        &buyer,
        &seller,
        &token,
        &2500,
        &Some(Bytes::from_slice(&env, b"meta1")),
        &None,
        &None,
    );
    client.create_escrow(
        &buyer,
        &seller,
        &token,
        &500,
        &Some(Bytes::from_slice(&env, b"meta2")),
        &None,
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

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None, &None, &None);

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

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None, &None, &None);

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

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None, &None, &None);

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

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None, &None, &None);

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

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None, &None, &None);

    let result = client.try_fund_escrow(&escrow_id);
    assert!(result.is_err());
}

#[test]
fn seller_can_accept_buyer_cancellation_and_refund_immediately() {
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
    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None, &None, &None);
    client.fund_escrow(&escrow_id);

    client.propose_cancellation(&escrow_id, &buyer);
    client.accept_cancellation(&escrow_id, &seller);

    assert_eq!(token.balance(&buyer), 1000);
    assert_eq!(token.balance(&client.address), 0);
    assert_eq!(client.get_total_refunded_amount(), 1000);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.status, crate::types::EscrowStatus::Refunded);
    assert_eq!(escrow.cancellation_proposer, None);
}

#[test]
fn accept_cancellation_fails_without_prior_proposal() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None, &None, &None);

    let result = client.try_accept_cancellation(&escrow_id, &seller);
    assert_eq!(result, Err(Ok(ContractError::InvalidEscrowState)));
}

#[test]
fn cancellation_fails_after_partial_item_release() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let mut items = Vec::new(&env);
    items.push_back(EscrowItem {
        amount: 400,
        released: false,
        description: None,
    });
    items.push_back(EscrowItem {
        amount: 600,
        released: false,
        description: None,
    });

    token_admin.mint(&client.address, &1000);
    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &token_id.address(),
        &1000,
        &None,
        &None,
        &Some(items),
    );

    client.release_item(&escrow_id, &0u32);

    let result = client.try_propose_cancellation(&escrow_id, &buyer);
    assert_eq!(result, Err(Ok(ContractError::InvalidEscrowState)));
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
        &None,
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

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None, &None, &None);

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
        &None,
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
        &None,
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

    client.resolve_dispute(&escrow_id, &0u32);

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

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None, &None, &None);
    client.release_escrow(&escrow_id);

    let events = env.events().all().filter_by_contract(&client.address);
    let emitted = events.events().to_vec();
    let expected_release = FundsReleasedEvent {
        escrow_id,
        amount: 1000,
        fee: 0,
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
        &None,
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
        &None,
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

    let escrow_id = client.create_escrow(&buyer, &seller, &token, &1000, &None, &Some(arbiter), &None);

    let result = client.try_resolve_dispute(&escrow_id, &0);
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

// =========================
// NATIVE XLM TOKEN TESTS
// =========================

/// Test that native XLM can be used for escrow funding and release.
/// This demonstrates that the contract works with the native Stellar token.
#[test]
fn test_native_xlm_escrow_funding_and_release() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    // Register the native XLM Stellar Asset Contract
    let xlm_sac = env.register_stellar_asset_contract_v2(admin.clone());
    let xlm_address = xlm_sac.address();
    let xlm_admin = soroban_sdk::token::StellarAssetClient::new(&env, &xlm_address);
    let xlm_token = soroban_sdk::token::Client::new(&env, &xlm_address);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    // Mint native XLM to the buyer (simulating buyer having XLM balance)
    // Amount is in stroops: 10 XLM = 100_000_000 stroops
    let escrow_amount: i128 = 100_000_000; // 10 XLM in stroops
    xlm_admin.mint(&buyer, &escrow_amount);
    assert_eq!(xlm_token.balance(&buyer), escrow_amount);

    // Create escrow with native XLM as the token
    let escrow_id = client.create_escrow(&buyer, &seller, &xlm_address, &escrow_amount, &None, &None, &None);

    // Verify escrow was created with XLM token address
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.token, xlm_address);
    assert_eq!(escrow.amount, escrow_amount);

    // Fund the escrow - buyer transfers XLM to contract
    client.fund_escrow(&escrow_id);

    // Verify XLM was transferred from buyer to contract
    assert_eq!(xlm_token.balance(&buyer), 0);
    assert_eq!(xlm_token.balance(&client.address), escrow_amount);

    // Release escrow - contract transfers XLM to seller
    client.release_escrow(&escrow_id);

    // Verify XLM was transferred from contract to seller
    assert_eq!(xlm_token.balance(&client.address), 0);
    assert_eq!(xlm_token.balance(&seller), escrow_amount);

    // Verify escrow status is Released
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.status, crate::types::EscrowStatus::Released);
}

/// Test that native XLM works with dispute resolution (refund to buyer).
#[test]
fn test_native_xlm_dispute_resolution_refund() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let arbiter = Address::generate(&env);

    // Register the native XLM Stellar Asset Contract
    let xlm_sac = env.register_stellar_asset_contract_v2(admin.clone());
    let xlm_address = xlm_sac.address();
    let xlm_admin = soroban_sdk::token::StellarAssetClient::new(&env, &xlm_address);
    let xlm_token = soroban_sdk::token::Client::new(&env, &xlm_address);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    // Mint native XLM to the contract (simulating funded escrow)
    let escrow_amount: i128 = 50_000_000; // 5 XLM in stroops
    xlm_admin.mint(&client.address, &escrow_amount);

    // Create escrow with native XLM and an arbiter
    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &xlm_address,
        &escrow_amount,
        &None,
        &Some(arbiter.clone()),
        &None,
    );

    // Force status to Disputed (simulating a dispute was raised)
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

    // Arbiter resolves dispute in favor of buyer (resolution = 1)
    client.resolve_dispute(&escrow_id, &1u32);

    // Verify XLM was refunded to buyer
    assert_eq!(xlm_token.balance(&buyer), escrow_amount);
    assert_eq!(xlm_token.balance(&client.address), 0);

    // Verify escrow status is Refunded
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.status, crate::types::EscrowStatus::Refunded);
}

/// Test that native XLM works with dispute resolution (release to seller).
#[test]
fn test_native_xlm_dispute_resolution_release() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let arbiter = Address::generate(&env);

    // Register the native XLM Stellar Asset Contract
    let xlm_sac = env.register_stellar_asset_contract_v2(admin.clone());
    let xlm_address = xlm_sac.address();
    let xlm_admin = soroban_sdk::token::StellarAssetClient::new(&env, &xlm_address);
    let xlm_token = soroban_sdk::token::Client::new(&env, &xlm_address);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    // Mint native XLM to the contract (simulating funded escrow)
    let escrow_amount: i128 = 75_000_000; // 7.5 XLM in stroops
    xlm_admin.mint(&client.address, &escrow_amount);

    // Create escrow with native XLM and an arbiter
    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &xlm_address,
        &escrow_amount,
        &None,
        &Some(arbiter.clone()),
        &None,
    );

    // Force status to Disputed (simulating a dispute was raised)
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

    // Arbiter resolves dispute in favor of seller (resolution = 0)
    client.resolve_dispute(&escrow_id, &0u32);

    // Verify XLM was released to seller
    assert_eq!(xlm_token.balance(&seller), escrow_amount);
    assert_eq!(xlm_token.balance(&client.address), 0);

    // Verify escrow status is Released
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.status, crate::types::EscrowStatus::Released);
}

// =========================
// MULTI-ITEM ESCROW TESTS
// =========================

/// Test creating an escrow with multiple items and releasing them individually.
#[test]
fn test_multi_item_escrow_creation_and_release() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());
    let token = soroban_sdk::token::Client::new(&env, &token_id.address());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    // Create items for a multi-product purchase
    let mut items = Vec::new(&env);
    items.push_back(EscrowItem {
        amount: 30_000_000,
        released: false,
        description: None,
    }); // Product 1: 3 XLM
    items.push_back(EscrowItem {
        amount: 40_000_000,
        released: false,
        description: None,
    }); // Product 2: 4 XLM
    items.push_back(EscrowItem {
        amount: 30_000_000,
        released: false,
        description: None,
    }); // Product 3: 3 XLM

    let total_amount: i128 = 100_000_000; // 10 XLM

    // Mint tokens to the contract for funding
    token_admin.mint(&client.address, &total_amount);

    // Create escrow with items
    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &token_id.address(),
        &total_amount,
        &None,
        &None,
        &Some(items.clone()),
    );

    // Verify escrow was created with items
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.amount, total_amount);
    assert_eq!(escrow.items.len(), 3);

    // Verify items are stored correctly
    let stored_items = client.get_escrow_items(&escrow_id).unwrap();
    assert_eq!(stored_items.len(), 3);
    assert_eq!(stored_items.get(0).unwrap().amount, 30_000_000);
    assert_eq!(stored_items.get(1).unwrap().amount, 40_000_000);
    assert_eq!(stored_items.get(2).unwrap().amount, 30_000_000);

    // Release first item
    client.release_item(&escrow_id, &0u32);

    // Verify first item is released and seller received funds
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert!(escrow.items.get(0).unwrap().released);
    assert!(!escrow.items.get(1).unwrap().released);
    assert!(!escrow.items.get(2).unwrap().released);
    assert_eq!(token.balance(&seller), 30_000_000);
    assert_eq!(escrow.status, crate::types::EscrowStatus::Pending); // Still pending

    // Release second item
    client.release_item(&escrow_id, &1u32);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert!(escrow.items.get(0).unwrap().released);
    assert!(escrow.items.get(1).unwrap().released);
    assert!(!escrow.items.get(2).unwrap().released);
    assert_eq!(token.balance(&seller), 70_000_000);
    assert_eq!(escrow.status, crate::types::EscrowStatus::Pending); // Still pending

    // Release third item - this should complete the escrow
    client.release_item(&escrow_id, &2u32);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert!(escrow.items.get(0).unwrap().released);
    assert!(escrow.items.get(1).unwrap().released);
    assert!(escrow.items.get(2).unwrap().released);
    assert_eq!(token.balance(&seller), 100_000_000);
    assert_eq!(escrow.status, crate::types::EscrowStatus::Released); // Now released
}

/// Test that releasing an already released item fails.
#[test]
fn test_release_item_already_released_fails() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let mut items = Vec::new(&env);
    items.push_back(EscrowItem {
        amount: 50_000_000,
        released: false,
        description: None,
    });

    token_admin.mint(&client.address, &50_000_000);

    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &token_id.address(),
        &50_000_000,
        &None,
        &None,
        &Some(items),
    );

    // Release the item first time
    client.release_item(&escrow_id, &0u32);

    // Try to release the same item again - should fail
    let result = client.try_release_item(&escrow_id, &0u32);
    assert_eq!(result, Err(Ok(ContractError::ItemAlreadyReleased)));
}

/// Test that releasing an invalid item index fails.
#[test]
fn test_release_item_invalid_index_fails() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    let mut items = Vec::new(&env);
    items.push_back(EscrowItem {
        amount: 50_000_000,
        released: false,
        description: None,
    });

    token_admin.mint(&client.address, &50_000_000);

    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &token_id.address(),
        &50_000_000,
        &None,
        &None,
        &Some(items),
    );

    // Try to release item with invalid index - should fail
    let result = client.try_release_item(&escrow_id, &5u32);
    assert_eq!(result, Err(Ok(ContractError::ItemNotFound)));
}

/// Test that item amounts must sum to total escrow amount.
#[test]
fn test_item_amounts_must_sum_to_total() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    // Items that don't sum to total amount
    let mut items = Vec::new(&env);
    items.push_back(EscrowItem {
        amount: 30_000_000,
        released: false,
        description: None,
    });
    items.push_back(EscrowItem {
        amount: 40_000_000,
        released: false,
        description: None,
    });

    // Total is 100_000_000 but items sum to 70_000_000
    let result = client.try_create_escrow(
        &buyer,
        &seller,
        &token,
        &100_000_000,
        &None,
        &None,
        &Some(items),
    );
    assert_eq!(result, Err(Ok(ContractError::ItemAmountInvalid)));
}

/// Test that too many items are rejected.
#[test]
fn test_too_many_items_rejected() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    // Create more than MAX_ITEMS_PER_ESCROW items
    let mut items = Vec::new(&env);
    for _ in 0..=crate::MAX_ITEMS_PER_ESCROW {
        items.push_back(EscrowItem {
            amount: 1,
            released: false,
            description: None,
        });
    }

    let result = client.try_create_escrow(
        &buyer,
        &seller,
        &token,
        &(items.len() as i128),
        &None,
        &None,
        &Some(items),
    );
    assert_eq!(result, Err(Ok(ContractError::TooManyItems)));
}

/// Test that escrow without items works with release_escrow (backward compatibility).
#[test]
fn test_escrow_without_items_uses_full_release() {
    let (env, client) = setup();
    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());
    let token = soroban_sdk::token::Client::new(&env, &token_id.address());

    env.mock_all_auths();
    client.initialize(&admin, &admin, &0);

    // Fund the contract so it can pay out
    token_admin.mint(&client.address, &1000);

    // Create escrow without items (old behavior)
    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &token_id.address(),
        &1000,
        &None,
        &None,
        &None, // No items
    );

    // Verify items is empty
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.items.len(), 0);

    // Use the old release_escrow function
    client.release_escrow(&escrow_id);

    // Verify full amount was released
    assert_eq!(token.balance(&seller), 1000);
    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.status, crate::types::EscrowStatus::Released);
}

#[test]
fn test_contract_balance_invariant() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let arbiter = Address::generate(&env);
    
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_id.address());
    let token = soroban_sdk::token::Client::new(&env, &token_id.address());

    let contract_id = env.register_contract(None, MarketXContract);
    let client = MarketXContractClient::new(&env, &contract_id);
    
    // Fee collector is admin, fee is 5%
    client.initialize(&admin, &admin, &500);

    token_admin.mint(&buyer, &10000);

    let mut expected_contract_balance = client.get_total_funded_amount() - client.get_total_released_amount();
    assert!(token.balance(&contract_id) >= expected_contract_balance);
    assert_eq!(expected_contract_balance, 0);

    let escrow_id1 = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None, &None, &None);
    client.fund_escrow(&escrow_id1);

    expected_contract_balance = client.get_total_funded_amount() - client.get_total_released_amount();
    assert_eq!(token.balance(&contract_id), expected_contract_balance);
    assert_eq!(expected_contract_balance, 1000);

    let escrow_id2 = client.create_escrow(&buyer, &seller, &token_id.address(), &2000, &None, &Some(arbiter.clone()), &None);
    client.fund_escrow(&escrow_id2);

    expected_contract_balance = client.get_total_funded_amount() - client.get_total_released_amount();
    assert_eq!(token.balance(&contract_id), expected_contract_balance);
    assert_eq!(expected_contract_balance, 3000);

    client.release_escrow(&escrow_id1);

    expected_contract_balance = client.get_total_funded_amount() - client.get_total_released_amount();
    assert!(token.balance(&contract_id) >= expected_contract_balance);
    assert_eq!(expected_contract_balance, 2000);

    let reason = crate::types::RefundReason::ItemNotDelivered;
    let evidence_hash = Bytes::from_slice(&env, b"evidence_hash_1234567890123");
    client.refund_escrow(&escrow_id2, &buyer, &2000, &reason, &evidence_hash);
    
    // Resolve dispute with refund to buyer (1)
    client.resolve_dispute(&escrow_id2, &1);

    expected_contract_balance = client.get_total_funded_amount() - client.get_total_released_amount() - client.get_total_refunded_amount();
    assert_eq!(token.balance(&contract_id), expected_contract_balance);
    assert_eq!(expected_contract_balance, 0);
}

#[test]
fn test_upgrade_auth_failure() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let collector = Address::generate(&env);

    let contract_id = env.register_contract(None, MarketXContract);
    let client = MarketXContractClient::new(&env, &contract_id);

    client.mock_all_auths();
    client.initialize(&admin, &collector, &250);

    let new_wasm_hash = soroban_sdk::BytesN::from_array(&env, &[0; 32]);
    let result = client
        .mock_auths(&[MockAuth {
            address: &non_admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "upgrade",
                args: (&new_wasm_hash,).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .try_upgrade(&new_wasm_hash);

    assert!(result.is_err());
}

#[test]
fn test_upgrade_state_persistence() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let contract_id = env.register_contract(None, MarketXContract);
    let client = MarketXContractClient::new(&env, &contract_id);
    
    client.initialize(&admin, &admin, &500);

    let escrow_id = client.create_escrow(&buyer, &seller, &token_id.address(), &1000, &None, &None, &None);

    let escrow = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow.amount, 1000);
    
    let new_wasm_hash = soroban_sdk::BytesN::from_array(&env, &[0; 32]);
    let _ = client.try_upgrade(&new_wasm_hash);
    
    let escrow_after = client.get_escrow(&escrow_id).unwrap();
    assert_eq!(escrow_after.amount, 1000);
}
