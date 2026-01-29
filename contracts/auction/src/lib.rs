#![no_std]

mod admin;
mod storage;
mod types;

use soroban_sdk::{contract, contracterror, contractimpl, token, Address, Env, Vec};
use types::{Auction, AuctionStatus, Bid};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    AuctionNotFound = 4,
    AuctionNotActive = 5,
    AuctionEnded = 6,
    BidTooLow = 7,
    BidBelowStartingPrice = 8,
    ReservePriceNotMet = 9,
    InvalidReservePrice = 10,
    InvalidDuration = 11,
    NoBidsPlaced = 12,
    AuctionAlreadySettled = 13,
    CannotCancelWithBids = 14,
    InvalidBuyNowPrice = 15,
    InsufficientBalance = 16,
}

#[contract]
pub struct AuctionContract;

#[contractimpl]
impl AuctionContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if storage::has_admin(&env) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        storage::set_admin(&env, &admin);
        Ok(())
    }

    pub fn create_auction(
        env: Env,
        seller: Address,
        token: Address,
        starting_price: i128,
        reserve_price: i128,
        buy_now_price: Option<i128>,
        duration_seconds: u64,
        fee_bps: u32,
    ) -> Result<u64, Error> {
        seller.require_auth();

        if duration_seconds == 0 {
            return Err(Error::InvalidDuration);
        }

        if reserve_price < starting_price {
            return Err(Error::InvalidReservePrice);
        }

        if let Some(buy_now) = buy_now_price {
            if buy_now < reserve_price {
                return Err(Error::InvalidBuyNowPrice);
            }
        }

        let current_time = env.ledger().timestamp();
        let auction_id = storage::increment_auction_counter(&env);

        let auction = Auction {
            auction_id,
            seller,
            token,
            starting_price,
            reserve_price,
            buy_now_price,
            start_time: current_time,
            end_time: current_time + duration_seconds,
            status: AuctionStatus::Active,
            highest_bid: 0,
            highest_bidder: None,
            fee_bps,
        };

        storage::save_auction(&env, &auction);
        Ok(auction_id)
    }

    pub fn place_bid(
        env: Env,
        auction_id: u64,
        bidder: Address,
        amount: i128,
    ) -> Result<(), Error> {
        bidder.require_auth();

        let mut auction = storage::get_auction(&env, auction_id)
            .ok_or(Error::AuctionNotFound)?;

        if !check_auction_active(&env, &auction) {
            return Err(Error::AuctionNotActive);
        }

        if amount < auction.starting_price {
            return Err(Error::BidBelowStartingPrice);
        }

        if amount <= auction.highest_bid {
            return Err(Error::BidTooLow);
        }

        if let Some(previous_bidder) = &auction.highest_bidder {
            refund_bidder(&env, &auction, previous_bidder)?;
        }

        escrow_bid(&env, &bidder, &auction.token, amount, auction_id)?;

        auction.highest_bid = amount;
        auction.highest_bidder = Some(bidder.clone());

        let bid = Bid {
            bidder,
            amount,
            timestamp: env.ledger().timestamp(),
        };
        storage::add_bid_to_history(&env, auction_id, bid);
        storage::save_auction(&env, &auction);

        Ok(())
    }

    pub fn buy_now(
        env: Env,
        auction_id: u64,
        buyer: Address,
    ) -> Result<(), Error> {
        buyer.require_auth();

        let mut auction = storage::get_auction(&env, auction_id)
            .ok_or(Error::AuctionNotFound)?;

        if !check_auction_active(&env, &auction) {
            return Err(Error::AuctionNotActive);
        }

        let buy_now_price = auction.buy_now_price.ok_or(Error::InvalidBuyNowPrice)?;

        if let Some(previous_bidder) = &auction.highest_bidder {
            refund_bidder(&env, &auction, previous_bidder)?;
        }

        let token_client = token::TokenClient::new(&env, &auction.token);
        token_client.transfer(&buyer, &env.current_contract_address(), &buy_now_price);

        auction.highest_bid = buy_now_price;
        auction.highest_bidder = Some(buyer.clone());
        auction.status = AuctionStatus::Ended;
        auction.end_time = env.ledger().timestamp();

        let bid = Bid {
            bidder: buyer,
            amount: buy_now_price,
            timestamp: env.ledger().timestamp(),
        };
        storage::add_bid_to_history(&env, auction_id, bid);
        storage::save_auction(&env, &auction);

        Self::settle_auction(env, auction_id)?;

        Ok(())
    }

    pub fn settle_auction(env: Env, auction_id: u64) -> Result<(), Error> {
        let mut auction = storage::get_auction(&env, auction_id)
            .ok_or(Error::AuctionNotFound)?;

        if auction.status == AuctionStatus::Settled {
            return Err(Error::AuctionAlreadySettled);
        }

        if auction.status == AuctionStatus::Cancelled {
            return Err(Error::Unauthorized);
        }

        if !check_auction_ended(&env, &auction) && auction.status != AuctionStatus::Ended {
            return Err(Error::AuctionNotActive);
        }

        if auction.highest_bidder.is_none() {
            auction.status = AuctionStatus::Cancelled;
            storage::save_auction(&env, &auction);
            return Ok(());
        }

        if auction.highest_bid < auction.reserve_price {
            let bidder = auction.highest_bidder.as_ref().unwrap();
            refund_bidder(&env, &auction, bidder)?;
            auction.status = AuctionStatus::Cancelled;
            storage::save_auction(&env, &auction);
            return Err(Error::ReservePriceNotMet);
        }

        let fee = calculate_fee(auction.highest_bid, auction.fee_bps);
        let seller_amount = auction.highest_bid - fee;

        let token_client = token::TokenClient::new(&env, &auction.token);
        let contract_address = env.current_contract_address();

        token_client.transfer(&contract_address, &auction.seller, &seller_amount);

        if fee > 0 {
            let admin = storage::get_admin(&env);
            token_client.transfer(&contract_address, &admin, &fee);
        }

        let bidder = auction.highest_bidder.as_ref().unwrap();
        storage::remove_escrowed_funds(&env, auction_id, bidder);

        auction.status = AuctionStatus::Settled;
        storage::save_auction(&env, &auction);

        Ok(())
    }

    pub fn cancel_auction(
        env: Env,
        auction_id: u64,
        seller: Address,
    ) -> Result<(), Error> {
        seller.require_auth();

        let mut auction = storage::get_auction(&env, auction_id)
            .ok_or(Error::AuctionNotFound)?;

        if auction.seller != seller {
            return Err(Error::Unauthorized);
        }

        if auction.highest_bidder.is_some() {
            return Err(Error::CannotCancelWithBids);
        }

        auction.status = AuctionStatus::Cancelled;
        storage::save_auction(&env, &auction);

        Ok(())
    }

    pub fn get_auction(env: Env, auction_id: u64) -> Result<Auction, Error> {
        storage::get_auction(&env, auction_id).ok_or(Error::AuctionNotFound)
    }

    pub fn get_bid_history(env: Env, auction_id: u64) -> Result<Vec<Bid>, Error> {
        if storage::get_auction(&env, auction_id).is_none() {
            return Err(Error::AuctionNotFound);
        }
        Ok(storage::get_bid_history(&env, auction_id))
    }

    pub fn get_highest_bid(env: Env, auction_id: u64) -> Result<(Option<Address>, i128), Error> {
        let auction = storage::get_auction(&env, auction_id)
            .ok_or(Error::AuctionNotFound)?;
        Ok((auction.highest_bidder, auction.highest_bid))
    }
}

fn check_auction_ended(env: &Env, auction: &Auction) -> bool {
    let current_time = env.ledger().timestamp();
    current_time >= auction.end_time
}

fn check_auction_active(env: &Env, auction: &Auction) -> bool {
    if auction.status != AuctionStatus::Active {
        return false;
    }
    let current_time = env.ledger().timestamp();
    current_time >= auction.start_time && current_time < auction.end_time
}

fn calculate_fee(amount: i128, fee_bps: u32) -> i128 {
    (amount * fee_bps as i128) / 10000
}

fn escrow_bid(
    env: &Env,
    bidder: &Address,
    token: &Address,
    amount: i128,
    auction_id: u64,
) -> Result<(), Error> {
    let token_client = token::TokenClient::new(env, token);
    token_client.transfer(bidder, &env.current_contract_address(), &amount);
    storage::set_escrowed_funds(env, auction_id, bidder, amount);
    Ok(())
}

fn refund_bidder(
    env: &Env,
    auction: &Auction,
    bidder: &Address,
) -> Result<(), Error> {
    let escrowed_amount = storage::get_escrowed_funds(env, auction.auction_id, bidder);
    if escrowed_amount > 0 {
        let token_client = token::TokenClient::new(env, &auction.token);
        token_client.transfer(
            &env.current_contract_address(),
            bidder,
            &escrowed_amount,
        );
        storage::remove_escrowed_funds(env, auction.auction_id, bidder);
    }
    Ok(())
}

#[cfg(test)]
mod test;

