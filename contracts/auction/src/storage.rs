use crate::types::{Auction, Bid, DataKey};
use soroban_sdk::{Address, Env, Vec};

pub fn get_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap()
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn has_admin(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

pub fn get_auction_counter(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::AuctionCounter)
        .unwrap_or(0)
}

pub fn increment_auction_counter(env: &Env) -> u64 {
    let counter = get_auction_counter(env) + 1;
    env.storage()
        .instance()
        .set(&DataKey::AuctionCounter, &counter);
    counter
}

pub fn get_auction(env: &Env, auction_id: u64) -> Option<Auction> {
    let key = DataKey::Auction(auction_id);
    env.storage().persistent().get(&key)
}

pub fn save_auction(env: &Env, auction: &Auction) {
    let key = DataKey::Auction(auction.auction_id);
    env.storage().persistent().set(&key, auction);
}

pub fn get_bid_history(env: &Env, auction_id: u64) -> Vec<Bid> {
    let key = DataKey::BidHistory(auction_id);
    env.storage().persistent().get(&key).unwrap_or(Vec::new(env))
}

pub fn add_bid_to_history(env: &Env, auction_id: u64, bid: Bid) {
    let key = DataKey::BidHistory(auction_id);
    let mut history = get_bid_history(env, auction_id);
    history.push_back(bid);
    env.storage().persistent().set(&key, &history);
}

pub fn get_escrowed_funds(env: &Env, auction_id: u64, bidder: &Address) -> i128 {
    let key = DataKey::EscrowedFunds(auction_id, bidder.clone());
    env.storage().persistent().get(&key).unwrap_or(0)
}

pub fn set_escrowed_funds(env: &Env, auction_id: u64, bidder: &Address, amount: i128) {
    let key = DataKey::EscrowedFunds(auction_id, bidder.clone());
    env.storage().persistent().set(&key, &amount);
}

pub fn remove_escrowed_funds(env: &Env, auction_id: u64, bidder: &Address) {
    let key = DataKey::EscrowedFunds(auction_id, bidder.clone());
    env.storage().persistent().remove(&key);
}
