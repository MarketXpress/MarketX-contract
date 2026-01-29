use crate::test::{advance_ledger, setup_test};
use crate::Error;

#[test]
fn test_settle_with_reserve_met() {
    let (env, client, _, seller, buyer, token_address, token) = setup_test();
    let auction_id = client.create_auction(&seller, &token_address, &1000, &1500, &Some(5000), &3600, &250);
    client.place_bid(&auction_id, &buyer, &2000);
    advance_ledger(&env, 3601);
    let initial_seller_balance = token.balance(&seller);
    client.settle_auction(&auction_id);
    let auction = client.get_auction(&auction_id);
    assert_eq!(auction.status, crate::types::AuctionStatus::Settled);
    let fee = (2000 * 250) / 10000;
    let expected_seller_amount = 2000 - fee;
    let final_seller_balance = token.balance(&seller);
    assert_eq!(final_seller_balance, initial_seller_balance + expected_seller_amount);
}

#[test]
fn test_settle_auction_no_bids() {
    let (env, client, _, seller, _, token_address, _token) = setup_test();
    let auction_id = client.create_auction(&seller, &token_address, &1000, &1500, &Some(5000), &3600, &250);
    advance_ledger(&env, 3601);
    client.settle_auction(&auction_id);
    let auction = client.get_auction(&auction_id);
    assert_eq!(auction.status, crate::types::AuctionStatus::Cancelled);
}
