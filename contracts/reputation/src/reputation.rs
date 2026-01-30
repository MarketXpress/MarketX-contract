use crate::storage::Storage;
use crate::types::{Reputation, Review};
use soroban_sdk::{Address, Env, String};

pub fn submit_review(
    env: &Env,
    reviewer: Address,
    subject: Address,
    rating: u32,
    comment: String,
) -> Reputation {
    reviewer.require_auth();

    if reviewer == subject {
        panic!("Self-review is not allowed");
    }

    if rating < 1 || rating > 5 {
        panic!("Rating must be between 1 and 5");
    }

    let storage = Storage::new(env);
    let mut reputation = storage.get_reputation(&subject);

    // Update score
    reputation.total_reviews += 1;
    reputation.total_score += rating;
    // Calculate new average scaled by 100
    // e.g. total_score = 9, reviews = 2 => 4.5 => 450
    reputation.average_rating = (reputation.total_score * 100) / reputation.total_reviews;

    // Store Review
    let review = Review {
        reviewer,
        rating,
        comment,
        timestamp: env.ledger().timestamp(),
    };

    storage.add_review(&subject, review);
    storage.set_reputation(&subject, &reputation);

    reputation
}

pub fn get_reputation(env: &Env, subject: Address) -> Reputation {
    let storage = Storage::new(env);
    storage.get_reputation(&subject)
}

pub fn get_reviews(env: &Env, subject: Address) -> soroban_sdk::Vec<Review> {
    let storage = Storage::new(env);
    storage.get_reviews(&subject)
}
