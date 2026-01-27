use crate::test::{advance_ledger, setup_test};
use crate::Error;

#[test]
fn test_initialize() {
    let (_, client, admin, _, _, _, _) = setup_test();
    assert!(client.try_initialize(&admin).is_err());
}

#[test]
fn test_create_auction() {
    let (_env, client, _, seller, _, token_address, _token) = setup_test();
    
    let auction_id = client.create_auction(
        &seller,
        &token_address,
        &1000,
        &1500,
        &Some(5000),
        &86400,
        &250,
    );

    assert_eq!(auction_id, 1);

    let auction = client.get_auction(&auction_id);
    assert_eq!(auction.seller, seller);
    assert_eq!(auction.starting_price, 1000);
    assert_eq!(auction.reserve_price, 1500);
    assert_eq!(auction.buy_now_price, Some(5000));
    assert_eq!(auction.fee_bps, 250);
}

#[test]
fn test_create_auction_invalid_reserve_price() {
    let (_, client, _, seller, _, token_address, _) = setup_test();
    
    let result = client.try_create_auction(
        &seller,
        &token_address,
        &2000,
        &1000,
        &Some(5000),
        &86400,
        &250,
    );

    assert_eq!(result, Err(Ok(Error::InvalidReservePrice)));
}

#[test]
fn test_create_auction_invalid_buy_now_price() {
    let (_, client, _, seller, _, token_address, _) = setup_test();
    
    let result = client.try_create_auction(
        &seller,
        &token_address,
        &1000,
        &2000,
        &Some(1500),
        &86400,
        &250,
    );

    assert_eq!(result, Err(Ok(Error::InvalidBuyNowPrice)));
}

#[test]
fn test_create_auction_zero_duration() {
    let (_, client, _, seller, _, token_address, _) = setup_test();
    
    let result = client.try_create_auction(
        &seller,
        &token_address,
        &1000,
        &1500,
        &Some(5000),
        &0,
        &250,
    );

    assert_eq!(result, Err(Ok(Error::InvalidDuration)));
}

#[test]
fn test_cancel_auction_no_bids() {
    let (_, client, _, seller, _, token_address, _) = setup_test();
    
    let auction_id = client.create_auction(
        &seller,
        &token_address,
        &1000,
        &1500,
        &Some(5000),
        &86400,
        &250,
    );

    client.cancel_auction(&auction_id, &seller);

    let auction = client.get_auction(&auction_id);
    assert_eq!(auction.status, crate::types::AuctionStatus::Cancelled);
}

#[test]
fn test_get_auction_not_found() {
    let (_, client, _, _, _, _, _) = setup_test();
    
    let result = client.try_get_auction(&999);
    assert_eq!(result, Err(Ok(Error::AuctionNotFound)));
}
