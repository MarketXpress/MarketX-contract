use crate::types::{Reputation, Review};
use soroban_sdk::{Address, Env, Vec};

pub struct Storage {
    env: Env,
}

impl Storage {
    pub fn new(env: &Env) -> Self {
        Self { env: env.clone() }
    }

    pub fn get_reputation(&self, user: &Address) -> Reputation {
        self.env
            .storage()
            .persistent()
            .get(user)
            .unwrap_or(Reputation::default())
    }

    pub fn set_reputation(&self, user: &Address, reputation: &Reputation) {
        self.env.storage().persistent().set(user, reputation);
    }

    // Using a separate key for reviews to avoid loading them all when just checking reputation
    // Key format: (Address, "reviews")
    pub fn get_reviews(&self, user: &Address) -> Vec<Review> {
        let key = (user.clone(), soroban_sdk::symbol_short!("reviews"));
        self.env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&self.env))
    }

    pub fn set_reviews(&self, user: &Address, reviews: &Vec<Review>) {
        let key = (user.clone(), soroban_sdk::symbol_short!("reviews"));
        self.env.storage().persistent().set(&key, reviews);
    }

    pub fn add_review(&self, user: &Address, review: Review) {
        let mut reviews = self.get_reviews(user);
        reviews.push_back(review);
        self.set_reviews(user, &reviews);
    }
}
