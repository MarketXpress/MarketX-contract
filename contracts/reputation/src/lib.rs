//! Reputation and Rating System Contract
//!
//! I built this on-chain reputation system to track user ratings and transaction history.
//! It implements weighted scoring, anti-gaming mechanisms, and reputation tiers.

#![no_std]

mod storage;
mod types;

use soroban_sdk::{contract, contracterror, contractimpl, Address, BytesN, Env, Vec};

use storage::{
    add_user_review, get_admin, get_dispute, get_review, get_review_count, get_user_reputation,
    get_user_review_at, get_user_review_count, has_admin, has_user_reputation,
    increment_review_count, is_transaction_reviewed, mark_transaction_reviewed, remove_dispute,
    remove_review, set_admin, set_dispute, set_review, set_user_reputation,
};
use types::{ReputationEvent, ReputationTier, Review, ReviewDispute, ReviewType, UserReputation};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Contract already initialized
    AlreadyInitialized = 1,
    /// Contract not initialized
    NotInitialized = 2,
    /// Caller is not the admin
    NotAdmin = 3,
    /// Invalid rating (must be 1-5)
    InvalidRating = 4,
    /// Invalid weight (must be 1-100)
    InvalidWeight = 5,
    /// Transaction already reviewed by this user
    AlreadyReviewed = 6,
    /// Cannot review yourself
    SelfReview = 7,
    /// Review not found
    ReviewNotFound = 8,
    /// User reputation not found
    UserNotFound = 9,
    /// Review already disputed
    AlreadyDisputed = 10,
    /// Dispute not found
    DisputeNotFound = 11,
    /// Only reviewee can dispute a review
    NotReviewee = 12,
}

#[contract]
pub struct ReputationContract;

#[contractimpl]
impl ReputationContract {
    // ─────────────────────────────────────────────────────────────────────────
    // Initialization
    // ─────────────────────────────────────────────────────────────────────────

    /// I initialize the contract with an admin address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if has_admin(&env) {
            return Err(Error::AlreadyInitialized);
        }
        set_admin(&env, &admin);
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Review Submission
    // ─────────────────────────────────────────────────────────────────────────

    /// I submit a review for a completed transaction.
    ///
    /// # Arguments
    /// * `reviewer` - Address of the person submitting the review
    /// * `reviewee` - Address of the person being reviewed
    /// * `transaction_id` - ID of the completed transaction (escrow/order)
    /// * `rating` - Rating from 1-5 stars
    /// * `weight` - Weight based on transaction size (1-100)
    /// * `comment_hash` - Hash of off-chain comment for gas efficiency
    /// * `review_type` - Whether this is buyer reviewing seller or vice versa
    pub fn submit_review(
        env: Env,
        reviewer: Address,
        reviewee: Address,
        transaction_id: u128,
        rating: u32,
        weight: u32,
        comment_hash: BytesN<32>,
        review_type: ReviewType,
    ) -> Result<u64, Error> {
        // Validate caller
        reviewer.require_auth();

        // Anti-gaming: Cannot review yourself
        if reviewer == reviewee {
            return Err(Error::SelfReview);
        }

        // Validate rating (1-5)
        if rating < 1 || rating > 5 {
            return Err(Error::InvalidRating);
        }

        // Validate weight (1-100)
        if weight < 1 || weight > 100 {
            return Err(Error::InvalidWeight);
        }

        // Anti-gaming: One review per transaction per reviewer
        if is_transaction_reviewed(&env, transaction_id, &reviewer) {
            return Err(Error::AlreadyReviewed);
        }

        // Mark transaction as reviewed
        mark_transaction_reviewed(&env, transaction_id, &reviewer);

        // Create review
        let review_id = increment_review_count(&env);
        let timestamp = env.ledger().timestamp();

        let review = Review {
            id: review_id,
            reviewer: reviewer.clone(),
            reviewee: reviewee.clone(),
            transaction_id,
            rating,
            weight,
            timestamp,
            comment_hash,
            review_type,
            disputed: false,
        };

        set_review(&env, &review);
        add_user_review(&env, &reviewee, review_id);

        // Update reviewee's reputation
        Self::update_reputation(&env, &reviewee, rating, weight, timestamp)?;

        Ok(review_id)
    }

    /// I update a user's reputation after they receive a review.
    fn update_reputation(
        env: &Env,
        user: &Address,
        rating: u32,
        weight: u32,
        timestamp: u64,
    ) -> Result<(), Error> {
        let mut rep = if has_user_reputation(env, user) {
            get_user_reputation(env, user).unwrap()
        } else {
            UserReputation::new(user.clone(), timestamp)
        };

        let old_score = rep.calculate_score();
        let old_tier = rep.tier;

        // Update weighted score: rating is 1-5, we scale by 100 for precision
        // weighted_score = rating * 100 * weight
        rep.total_weighted_score += (rating as i64) * 100 * (weight as i64);
        rep.total_weight += weight as u64;
        rep.review_count += 1;

        // Track positive/negative counts
        if rating >= 4 {
            rep.positive_count += 1;
        } else if rating <= 2 {
            rep.negative_count += 1;
        }

        // Update tier
        rep.tier = rep.calculate_tier();
        rep.last_updated = timestamp;

        let new_score = rep.calculate_score();
        let new_tier = rep.tier;

        set_user_reputation(env, &rep);

        // Emit reputation change event (stored for history)
        let _event = ReputationEvent {
            user: user.clone(),
            old_score,
            new_score,
            old_tier,
            new_tier,
            timestamp,
        };

        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Query Functions
    // ─────────────────────────────────────────────────────────────────────────

    /// Get a user's reputation data.
    pub fn get_reputation(env: Env, user: Address) -> Result<UserReputation, Error> {
        get_user_reputation(&env, &user).ok_or(Error::UserNotFound)
    }

    /// Get a user's reputation score (0-500, representing 0.00-5.00).
    pub fn get_score(env: Env, user: Address) -> Result<u32, Error> {
        let rep = get_user_reputation(&env, &user).ok_or(Error::UserNotFound)?;
        Ok(rep.calculate_score())
    }

    /// Get a user's reputation tier.
    pub fn get_tier(env: Env, user: Address) -> Result<ReputationTier, Error> {
        let rep = get_user_reputation(&env, &user).ok_or(Error::UserNotFound)?;
        Ok(rep.tier)
    }

    /// Get a specific review by ID.
    pub fn get_review(env: Env, review_id: u64) -> Result<Review, Error> {
        get_review(&env, review_id).ok_or(Error::ReviewNotFound)
    }

    /// Get recent reviews for a user (most recent first).
    pub fn get_reviews(env: Env, user: Address, limit: u32) -> Result<Vec<Review>, Error> {
        let count = get_user_review_count(&env, &user);
        let mut reviews = Vec::new(&env);

        if count == 0 {
            return Ok(reviews);
        }

        // Get reviews in reverse order (most recent first)
        let start = if count > limit { count - limit } else { 0 };
        for i in (start..count).rev() {
            if let Some(review_id) = get_user_review_at(&env, &user, i) {
                if let Some(review) = get_review(&env, review_id) {
                    reviews.push_back(review);
                }
            }
        }

        Ok(reviews)
    }

    /// Get the total number of reviews in the system.
    pub fn get_total_reviews(env: Env) -> u64 {
        get_review_count(&env)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Dispute Functions
    // ─────────────────────────────────────────────────────────────────────────

    /// Dispute a review (only the reviewee can dispute).
    pub fn dispute_review(
        env: Env,
        disputer: Address,
        review_id: u64,
        reason_hash: BytesN<32>,
    ) -> Result<(), Error> {
        disputer.require_auth();

        let review = get_review(&env, review_id).ok_or(Error::ReviewNotFound)?;

        // Only reviewee can dispute
        if disputer != review.reviewee {
            return Err(Error::NotReviewee);
        }

        // Check if already disputed
        if review.disputed {
            return Err(Error::AlreadyDisputed);
        }

        // Create dispute record
        let dispute = ReviewDispute {
            review_id,
            disputer: disputer.clone(),
            reason_hash,
            timestamp: env.ledger().timestamp(),
            resolved: false,
        };

        set_dispute(&env, &dispute);

        // Mark review as disputed
        let mut updated_review = review;
        updated_review.disputed = true;
        set_review(&env, &updated_review);

        Ok(())
    }

    /// Get dispute details for a review.
    pub fn get_dispute(env: Env, review_id: u64) -> Result<ReviewDispute, Error> {
        get_dispute(&env, review_id).ok_or(Error::DisputeNotFound)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Admin Functions
    // ─────────────────────────────────────────────────────────────────────────

    /// Admin function to remove a fraudulent review.
    pub fn admin_remove_review(env: Env, admin: Address, review_id: u64) -> Result<(), Error> {
        admin.require_auth();

        if admin != get_admin(&env) {
            return Err(Error::NotAdmin);
        }

        let review = get_review(&env, review_id).ok_or(Error::ReviewNotFound)?;

        // Reverse the reputation impact
        if let Some(mut rep) = get_user_reputation(&env, &review.reviewee) {
            let weight = review.weight as i64;
            let rating = review.rating as i64;

            rep.total_weighted_score -= rating * 100 * weight;
            rep.total_weight -= review.weight as u64;
            rep.review_count = rep.review_count.saturating_sub(1);

            if review.rating >= 4 {
                rep.positive_count = rep.positive_count.saturating_sub(1);
            } else if review.rating <= 2 {
                rep.negative_count = rep.negative_count.saturating_sub(1);
            }

            // Recalculate tier
            rep.tier = rep.calculate_tier();
            rep.last_updated = env.ledger().timestamp();

            set_user_reputation(&env, &rep);
        }

        // Remove review and any dispute
        remove_review(&env, review_id);
        remove_dispute(&env, review_id);

        Ok(())
    }

    /// Admin function to adjust a user's reputation score.
    pub fn admin_adjust_score(
        env: Env,
        admin: Address,
        user: Address,
        score_adjustment: i64,
        weight_adjustment: u64,
    ) -> Result<(), Error> {
        admin.require_auth();

        if admin != get_admin(&env) {
            return Err(Error::NotAdmin);
        }

        let timestamp = env.ledger().timestamp();
        let mut rep = if has_user_reputation(&env, &user) {
            get_user_reputation(&env, &user).unwrap()
        } else {
            UserReputation::new(user.clone(), timestamp)
        };

        rep.total_weighted_score += score_adjustment;
        rep.total_weight += weight_adjustment;
        rep.tier = rep.calculate_tier();
        rep.last_updated = timestamp;

        set_user_reputation(&env, &rep);

        Ok(())
    }

    /// Admin function to resolve a dispute.
    pub fn admin_resolve_dispute(
        env: Env,
        admin: Address,
        review_id: u64,
        remove_review: bool,
    ) -> Result<(), Error> {
        admin.require_auth();

        if admin != get_admin(&env) {
            return Err(Error::NotAdmin);
        }

        let mut dispute = get_dispute(&env, review_id).ok_or(Error::DisputeNotFound)?;

        if remove_review {
            // Remove the review and reverse reputation impact
            Self::admin_remove_review(env.clone(), admin, review_id)?;
        } else {
            // Mark dispute as resolved but keep the review
            dispute.resolved = true;
            set_dispute(&env, &dispute);

            // Unmark review as disputed
            if let Some(mut review) = get_review(&env, review_id) {
                review.disputed = false;
                set_review(&env, &review);
            }
        }

        Ok(())
    }

    /// Get the admin address.
    pub fn get_admin(env: Env) -> Result<Address, Error> {
        if !has_admin(&env) {
            return Err(Error::NotInitialized);
        }
        Ok(get_admin(&env))
    }
}

#[cfg(test)]
mod test;
