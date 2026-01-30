//! I define the core data types for the Reputation and Rating System Contract.

use soroban_sdk::{contracttype, Address, BytesN};

/// Represents a user's aggregated reputation data.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct UserReputation {
    /// The user's address
    pub user: Address,
    /// Sum of all weighted ratings (scaled by 100 for precision)
    pub total_weighted_score: i64,
    /// Sum of all weights (for weighted average calculation)
    pub total_weight: u64,
    /// Total number of reviews received
    pub review_count: u32,
    /// Number of positive reviews (rating >= 4)
    pub positive_count: u32,
    /// Number of negative reviews (rating <= 2)
    pub negative_count: u32,
    /// Current reputation tier
    pub tier: ReputationTier,
    /// Timestamp of last reputation update
    pub last_updated: u64,
}

/// Represents a single review submitted for a transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Review {
    /// Unique review ID
    pub id: u64,
    /// Address of the reviewer
    pub reviewer: Address,
    /// Address of the user being reviewed
    pub reviewee: Address,
    /// Transaction ID this review is linked to (escrow/order ID)
    pub transaction_id: u128,
    /// Rating from 1-5 stars
    pub rating: u32,
    /// Weight based on transaction size (1-100)
    pub weight: u32,
    /// Timestamp when review was submitted
    pub timestamp: u64,
    /// Hash of off-chain comment (for gas efficiency)
    pub comment_hash: BytesN<32>,
    /// Type of review (buyer reviewing seller or vice versa)
    pub review_type: ReviewType,
    /// Whether this review is disputed
    pub disputed: bool,
}

/// Reputation tiers based on review count and score.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[contracttype]
#[repr(u32)]
pub enum ReputationTier {
    /// New user with < 5 reviews
    New = 0,
    /// >= 5 reviews, score >= 60%
    Bronze = 1,
    /// >= 20 reviews, score >= 75%
    Silver = 2,
    /// >= 50 reviews, score >= 85%
    Gold = 3,
    /// >= 100 reviews, score >= 90%
    Platinum = 4,
}

/// Type of review based on the reviewer's role in the transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
#[repr(u32)]
pub enum ReviewType {
    /// Buyer reviewing a seller
    BuyerToSeller = 0,
    /// Seller reviewing a buyer
    SellerToBuyer = 1,
}

/// Event logged when reputation changes.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReputationEvent {
    /// User whose reputation changed
    pub user: Address,
    /// Previous score (weighted average * 100)
    pub old_score: u32,
    /// New score (weighted average * 100)
    pub new_score: u32,
    /// Previous tier
    pub old_tier: ReputationTier,
    /// New tier
    pub new_tier: ReputationTier,
    /// Timestamp of the event
    pub timestamp: u64,
}

/// Dispute record for a review under investigation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReviewDispute {
    /// Review ID being disputed
    pub review_id: u64,
    /// Address of the user who filed the dispute
    pub disputer: Address,
    /// Reason hash (off-chain reference)
    pub reason_hash: BytesN<32>,
    /// Timestamp when dispute was filed
    pub timestamp: u64,
    /// Whether dispute has been resolved
    pub resolved: bool,
}

impl UserReputation {
    /// I create a new empty reputation for a user.
    pub fn new(user: Address, timestamp: u64) -> Self {
        Self {
            user,
            total_weighted_score: 0,
            total_weight: 0,
            review_count: 0,
            positive_count: 0,
            negative_count: 0,
            tier: ReputationTier::New,
            last_updated: timestamp,
        }
    }

    /// I calculate the weighted average score (0-500, representing 0.00-5.00).
    pub fn calculate_score(&self) -> u32 {
        if self.total_weight == 0 {
            return 0;
        }
        // total_weighted_score is already scaled by 100, so I just divide by total_weight
        (self.total_weighted_score / (self.total_weight as i64)) as u32
    }

    /// Calculates the percentage score (0-100).
    pub fn calculate_percentage(&self) -> u32 {
        let score = self.calculate_score();
        // Convert 0-500 scale to 0-100 percentage
        score / 5
    }

    /// Determines the appropriate tier based on review count and score.
    pub fn calculate_tier(&self) -> ReputationTier {
        let percentage = self.calculate_percentage();

        if self.review_count >= 100 && percentage >= 90 {
            ReputationTier::Platinum
        } else if self.review_count >= 50 && percentage >= 85 {
            ReputationTier::Gold
        } else if self.review_count >= 20 && percentage >= 75 {
            ReputationTier::Silver
        } else if self.review_count >= 5 && percentage >= 60 {
            ReputationTier::Bronze
        } else {
            ReputationTier::New
        }
    }
}
