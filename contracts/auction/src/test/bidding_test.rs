use crate::test::{advance_ledger, setup_test};
use crate::Error;

#[test]
fn test_place_valid_bid() {
    let (_env, client, _, seller, buyer, token_address, _token) = setup_test();
    let auction_id = client.create_auction(&seller, &token_address, &1000, &1500, &Some(5000), &86400, &250);
    client.place_bid(&auction_id, &buyer, &2000);
    let (highest_bidder, highest_bid) = client.get_highest_bid(&auction_id);
    assert_eq!(highest_bidder, Some(buyer.clone()));
    assert_eq!(highest_bid, 2000);
}

#[test]
fn test_reject_low_bid() {
    let (_env, client, _, seller, buyer, token_address, _token) = setup_test();
    let auction_id = client.create_auction(&seller, &token_address, &1000, &1500, &Some(5000), &86400, &250);
    let result = client.try_place_bid(&auction_id, &buyer, &500);
    assert_eq!(result, Err(Ok(Error::BidBelowStartingPrice)));
}

#[test]
fn test_bid_after_end_fails() {
    let (env, client, _, seller, buyer, token_address, _token) = setup_test();
    let auction_id = client.create_auction(&seller, &token_address, &1000, &1500, &Some(5000), &3600, &250);
    advance_ledger(&env, 3601);
    let result = client.try_place_bid(&auction_id, &buyer, &2000);
    assert_eq!(result, Err(Ok(Error::AuctionNotActive)));
}

#[test]
fn test_buy_now_option() {
    let (_env, client, _, seller, buyer, token_address, token) = setup_test();
    let auction_id = client.create_auction(&seller, &token_address, &1000, &1500, &Some(5000), &86400, &250);
    let initial_seller_balance = token.balance(&seller);
    client.buy_now(&auction_id, &buyer);
    let auction = client.get_auction(&auction_id);
    assert_eq!(auction.status, crate::types::AuctionStatus::Settled);
    let fee = (5000 * 250) / 10000;
    let final_seller_balance = token.balance(&seller);
    assert_eq!(final_seller_balance, initial_seller_balance + 5000 - fee);
}
