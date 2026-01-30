#![no_std]

mod reputation;
mod storage;
mod types;

#[cfg(test)]
mod tests;

use soroban_sdk::{contract, contractimpl, Address, Env, String, Vec};
use types::{Reputation, Review};

#[contract]
pub struct ReputationContract;

#[contractimpl]
impl ReputationContract {
    pub fn submit_review(
        env: Env,
        reviewer: Address,
        subject: Address,
        rating: u32,
        comment: String,
    ) -> Reputation {
        reputation::submit_review(&env, reviewer, subject, rating, comment)
    }

    pub fn get_reputation(env: Env, subject: Address) -> Reputation {
        reputation::get_reputation(&env, subject)
    }

    pub fn get_reviews(env: Env, subject: Address) -> Vec<Review> {
        reputation::get_reviews(&env, subject)
    }
}
