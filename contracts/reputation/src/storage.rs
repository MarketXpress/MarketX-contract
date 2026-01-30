//! Storage keys and helper functions for the Reputation contract.

use soroban_sdk::{contracttype, Address, Env};

use crate::types::{Review, ReviewDispute, UserReputation};

/// Storage keys for the reputation contract.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    /// Admin address
    Admin,
    /// User reputation data: DataKey::UserRep(user_address)
    UserRep(Address),
    /// Review by ID: DataKey::Review(review_id)
    Review(u64),
    /// Global review counter
    ReviewCount,
    /// Marks a transaction as already reviewed: DataKey::TxReviewed(tx_id, reviewer)
    TxReviewed(u128, Address),
    /// Review dispute: DataKey::Dispute(review_id)
    Dispute(u64),
    /// User's review history index: DataKey::UserReviewIdx(user, index)
    UserReviewIdx(Address, u32),
    /// Count of reviews for a user
    UserReviewCount(Address),
}

// ─────────────────────────────────────────────────────────────────────────────
// Admin Storage
// ─────────────────────────────────────────────────────────────────────────────

pub fn has_admin(env: &Env) -> bool {
    env.storage().persistent().has(&DataKey::Admin)
}

pub fn get_admin(env: &Env) -> Address {
    env.storage()
        .persistent()
        .get(&DataKey::Admin)
        .expect("admin not set")
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().persistent().set(&DataKey::Admin, admin);
}

// ─────────────────────────────────────────────────────────────────────────────
// User Reputation Storage
// ─────────────────────────────────────────────────────────────────────────────

pub fn has_user_reputation(env: &Env, user: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::UserRep(user.clone()))
}

pub fn get_user_reputation(env: &Env, user: &Address) -> Option<UserReputation> {
    env.storage()
        .persistent()
        .get(&DataKey::UserRep(user.clone()))
}

pub fn set_user_reputation(env: &Env, rep: &UserReputation) {
    env.storage()
        .persistent()
        .set(&DataKey::UserRep(rep.user.clone()), rep);
}

// ─────────────────────────────────────────────────────────────────────────────
// Review Storage
// ─────────────────────────────────────────────────────────────────────────────

pub fn get_review_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::ReviewCount)
        .unwrap_or(0)
}

pub fn increment_review_count(env: &Env) -> u64 {
    let count = get_review_count(env) + 1;
    env.storage()
        .persistent()
        .set(&DataKey::ReviewCount, &count);
    count
}

pub fn get_review(env: &Env, review_id: u64) -> Option<Review> {
    env.storage().persistent().get(&DataKey::Review(review_id))
}

pub fn set_review(env: &Env, review: &Review) {
    env.storage()
        .persistent()
        .set(&DataKey::Review(review.id), review);
}

pub fn remove_review(env: &Env, review_id: u64) {
    env.storage()
        .persistent()
        .remove(&DataKey::Review(review_id));
}

// ─────────────────────────────────────────────────────────────────────────────
// Transaction Review Tracking (Anti-Gaming)
// ─────────────────────────────────────────────────────────────────────────────

pub fn is_transaction_reviewed(env: &Env, tx_id: u128, reviewer: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::TxReviewed(tx_id, reviewer.clone()))
}

pub fn mark_transaction_reviewed(env: &Env, tx_id: u128, reviewer: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::TxReviewed(tx_id, reviewer.clone()), &true);
}

// ─────────────────────────────────────────────────────────────────────────────
// Dispute Storage
// ─────────────────────────────────────────────────────────────────────────────

pub fn get_dispute(env: &Env, review_id: u64) -> Option<ReviewDispute> {
    env.storage().persistent().get(&DataKey::Dispute(review_id))
}

pub fn set_dispute(env: &Env, dispute: &ReviewDispute) {
    env.storage()
        .persistent()
        .set(&DataKey::Dispute(dispute.review_id), dispute);
}

pub fn remove_dispute(env: &Env, review_id: u64) {
    env.storage()
        .persistent()
        .remove(&DataKey::Dispute(review_id));
}

// ─────────────────────────────────────────────────────────────────────────────
// User Review History
// ─────────────────────────────────────────────────────────────────────────────

pub fn get_user_review_count(env: &Env, user: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::UserReviewCount(user.clone()))
        .unwrap_or(0)
}

pub fn add_user_review(env: &Env, user: &Address, review_id: u64) {
    let count = get_user_review_count(env, user);
    env.storage()
        .persistent()
        .set(&DataKey::UserReviewIdx(user.clone(), count), &review_id);
    env.storage()
        .persistent()
        .set(&DataKey::UserReviewCount(user.clone()), &(count + 1));
}

pub fn get_user_review_at(env: &Env, user: &Address, index: u32) -> Option<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::UserReviewIdx(user.clone(), index))
}
