//! Unit tests for the Reputation and Rating System Contract.

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, BytesN, Env,
};

fn set_ledger(env: &Env, timestamp: u64) {
    env.ledger().set(LedgerInfo {
        timestamp,
        protocol_version: 23,
        sequence_number: 1,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 3110400,
    });
}

fn setup_contract(env: &Env) -> (Address, ReputationContractClient) {
    let contract_id = env.register(ReputationContract, ());
    let client = ReputationContractClient::new(env, &contract_id);
    (contract_id, client)
}

fn zero_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}

// ─────────────────────────────────────────────────────────────────────────────
// Initialization Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert_eq!(client.get_admin(), admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_initialize_twice_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);
    client.initialize(&admin); // Should panic
}

// ─────────────────────────────────────────────────────────────────────────────
// Review Submission Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_submit_review_success() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);
    let tx_id = 1u128;
    let rating = 5u32;
    let weight = 50u32;

    let review_id = client.submit_review(
        &reviewer,
        &reviewee,
        &tx_id,
        &rating,
        &weight,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );

    assert_eq!(review_id, 1);

    // Check review was stored
    let review = client.get_review(&review_id);
    assert_eq!(review.reviewer, reviewer);
    assert_eq!(review.reviewee, reviewee);
    assert_eq!(review.rating, 5);
    assert_eq!(review.weight, 50);

    // Check reputation was updated
    let rep = client.get_reputation(&reviewee);
    assert_eq!(rep.review_count, 1);
    assert_eq!(rep.positive_count, 1);
    assert_eq!(rep.negative_count, 0);
}

#[test]
fn test_submit_multiple_reviews() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewee = Address::generate(&env);

    // Submit 5 reviews from different reviewers
    for i in 1..=5 {
        let reviewer = Address::generate(&env);
        client.submit_review(
            &reviewer,
            &reviewee,
            &(i as u128),
            &4,
            &50,
            &zero_hash(&env),
            &ReviewType::BuyerToSeller,
        );
    }

    let rep = client.get_reputation(&reviewee);
    assert_eq!(rep.review_count, 5);
    assert_eq!(rep.positive_count, 5); // All 4-star reviews are positive
    assert_eq!(rep.tier, ReputationTier::Bronze); // >= 5 reviews with score >= 60%
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_self_review_fails() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let user = Address::generate(&env);
    client.submit_review(
        &user,
        &user, // Same as reviewer
        &1u128,
        &5,
        &50,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_duplicate_review_fails() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);
    let tx_id = 1u128;

    client.submit_review(
        &reviewer,
        &reviewee,
        &tx_id,
        &5,
        &50,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );

    // Same reviewer, same transaction
    client.submit_review(
        &reviewer,
        &reviewee,
        &tx_id,
        &4,
        &50,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_invalid_rating_zero_fails() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);

    client.submit_review(
        &reviewer,
        &reviewee,
        &1u128,
        &0, // Invalid: 0
        &50,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_invalid_rating_six_fails() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);

    client.submit_review(
        &reviewer,
        &reviewee,
        &1u128,
        &6, // Invalid: > 5
        &50,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_invalid_weight_fails() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);

    client.submit_review(
        &reviewer,
        &reviewee,
        &1u128,
        &5,
        &0, // Invalid: 0
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scoring and Tier Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_weighted_score_calculation() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewee = Address::generate(&env);

    // Submit review: 5 stars, weight 100
    let reviewer1 = Address::generate(&env);
    client.submit_review(
        &reviewer1,
        &reviewee,
        &1u128,
        &5,
        &100,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );

    // Score should be 500 (5.00 stars)
    let score = client.get_score(&reviewee);
    assert_eq!(score, 500);

    // Submit another review: 1 star, weight 100
    let reviewer2 = Address::generate(&env);
    client.submit_review(
        &reviewer2,
        &reviewee,
        &2u128,
        &1,
        &100,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );

    // Score should be (5*100 + 1*100) / 200 * 100 = 300 (3.00 stars)
    let score = client.get_score(&reviewee);
    assert_eq!(score, 300);
}

#[test]
fn test_tier_progression() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewee = Address::generate(&env);

    // Start with New tier (no reviews)
    // Submit 5 reviews with 5 stars each
    for i in 1..=5 {
        let reviewer = Address::generate(&env);
        client.submit_review(
            &reviewer,
            &reviewee,
            &(i as u128),
            &5,
            &50,
            &zero_hash(&env),
            &ReviewType::BuyerToSeller,
        );
    }

    // Should be Bronze now (>= 5 reviews, 100% score)
    assert_eq!(client.get_tier(&reviewee), ReputationTier::Bronze);

    // Add 15 more reviews (total 20)
    for i in 6..=20 {
        let reviewer = Address::generate(&env);
        client.submit_review(
            &reviewer,
            &reviewee,
            &(i as u128),
            &5,
            &50,
            &zero_hash(&env),
            &ReviewType::BuyerToSeller,
        );
    }

    // Should be Silver now (>= 20 reviews, 100% score)
    assert_eq!(client.get_tier(&reviewee), ReputationTier::Silver);
}

#[test]
fn test_negative_reviews_count() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewee = Address::generate(&env);

    // Submit negative reviews (1-2 stars)
    let reviewer1 = Address::generate(&env);
    client.submit_review(
        &reviewer1,
        &reviewee,
        &1u128,
        &1,
        &50,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );

    let reviewer2 = Address::generate(&env);
    client.submit_review(
        &reviewer2,
        &reviewee,
        &2u128,
        &2,
        &50,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );

    // Submit neutral review (3 stars)
    let reviewer3 = Address::generate(&env);
    client.submit_review(
        &reviewer3,
        &reviewee,
        &3u128,
        &3,
        &50,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );

    let rep = client.get_reputation(&reviewee);
    assert_eq!(rep.negative_count, 2);
    assert_eq!(rep.positive_count, 0);
    assert_eq!(rep.review_count, 3);
}

// ─────────────────────────────────────────────────────────────────────────────
// Dispute Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_dispute_review() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);

    let review_id = client.submit_review(
        &reviewer,
        &reviewee,
        &1u128,
        &1, // Bad review
        &50,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );

    // Reviewee disputes the review
    client.dispute_review(&reviewee, &review_id, &zero_hash(&env));

    // Check review is marked as disputed
    let review = client.get_review(&review_id);
    assert!(review.disputed);

    // Check dispute record exists
    let dispute = client.get_dispute(&review_id);
    assert_eq!(dispute.disputer, reviewee);
    assert!(!dispute.resolved);
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_dispute_not_reviewee_fails() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);
    let random = Address::generate(&env);

    let review_id = client.submit_review(
        &reviewer,
        &reviewee,
        &1u128,
        &1,
        &50,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );

    // Random user tries to dispute
    client.dispute_review(&random, &review_id, &zero_hash(&env));
}

// ─────────────────────────────────────────────────────────────────────────────
// Admin Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_admin_remove_review() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);

    // Submit a bad review
    let review_id = client.submit_review(
        &reviewer,
        &reviewee,
        &1u128,
        &1,
        &100,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );

    // Check initial reputation
    let rep_before = client.get_reputation(&reviewee);
    assert_eq!(rep_before.review_count, 1);
    assert_eq!(rep_before.negative_count, 1);

    // Admin removes the review
    client.admin_remove_review(&admin, &review_id);

    // Reputation should be reset
    let rep_after = client.get_reputation(&reviewee);
    assert_eq!(rep_after.review_count, 0);
    assert_eq!(rep_after.negative_count, 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_admin_remove_review_not_admin_fails() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);
    let not_admin = Address::generate(&env);

    let review_id = client.submit_review(
        &reviewer,
        &reviewee,
        &1u128,
        &1,
        &50,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );

    // Non-admin tries to remove
    client.admin_remove_review(&not_admin, &review_id);
}

#[test]
fn test_admin_resolve_dispute_keep_review() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewer = Address::generate(&env);
    let reviewee = Address::generate(&env);

    let review_id = client.submit_review(
        &reviewer,
        &reviewee,
        &1u128,
        &3,
        &50,
        &zero_hash(&env),
        &ReviewType::BuyerToSeller,
    );

    // Dispute the review
    client.dispute_review(&reviewee, &review_id, &zero_hash(&env));

    // Admin resolves without removing
    client.admin_resolve_dispute(&admin, &review_id, &false);

    // Review should still exist and not be disputed
    let review = client.get_review(&review_id);
    assert!(!review.disputed);

    // Dispute should be resolved
    let dispute = client.get_dispute(&review_id);
    assert!(dispute.resolved);
}

#[test]
fn test_get_reviews_pagination() {
    let env = Env::default();
    env.mock_all_auths();
    set_ledger(&env, 1000);
    let (_contract_id, client) = setup_contract(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let reviewee = Address::generate(&env);

    // Submit 10 reviews
    for i in 1..=10 {
        let reviewer = Address::generate(&env);
        client.submit_review(
            &reviewer,
            &reviewee,
            &(i as u128),
            &4,
            &50,
            &zero_hash(&env),
            &ReviewType::BuyerToSeller,
        );
    }

    // Get last 5 reviews
    let reviews = client.get_reviews(&reviewee, &5);
    assert_eq!(reviews.len(), 5);

    // Should be in reverse order (most recent first)
    assert_eq!(reviews.get(0).unwrap().id, 10);
    assert_eq!(reviews.get(4).unwrap().id, 6);
}
