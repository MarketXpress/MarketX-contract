use soroban_sdk::{contracttype, Address, String};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Review {
    pub reviewer: Address,
    pub rating: u32, // 1 to 5
    pub comment: String,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reputation {
    pub average_rating: u32, // Scaled by 100 (e.g., 450 = 4.5)
    pub total_reviews: u32,
    pub total_score: u32,
}

impl Default for Reputation {
    fn default() -> Self {
        Self {
            average_rating: 0,
            total_reviews: 0,
            total_score: 0,
        }
    }
}
