use soroban_sdk::{contracttype, Address};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuctionStatus {
    Active = 0,
    Ended = 1,
    Settled = 2,
    Cancelled = 3,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Auction {
    pub auction_id: u64,
    pub seller: Address,
    pub token: Address,
    pub starting_price: i128,
    pub reserve_price: i128,
    pub buy_now_price: Option<i128>,
    pub start_time: u64,
    pub end_time: u64,
    pub status: AuctionStatus,
    pub highest_bid: i128,
    pub highest_bidder: Option<Address>,
    pub fee_bps: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct Bid {
    pub bidder: Address,
    pub amount: i128,
    pub timestamp: u64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    AuctionCounter,
    Auction(u64),
    BidHistory(u64),
    EscrowedFunds(u64, Address),
}
