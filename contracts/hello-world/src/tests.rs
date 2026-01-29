#![cfg(test)]

use soroban_sdk::{Env, testutils::Address, String};

use crate::order::{
    create_order, ship_order, deliver_order, cancel_order,
    dispute_order, resolve_dispute, get_order, OrderStatus,
};

#[test]
fn full_order_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let asset = Address::generate(&env);

    let order_id = create_order(
        env.clone(),
        buyer.clone(),
        seller.clone(),
        asset,
        100,
    );

    ship_order(
        env.clone(),
        seller.clone(),
        order_id,
        String::from("TRACK123"),
    );

    deliver_order(env.clone(), buyer.clone(), order_id);

    let order = get_order(env, order_id);
    assert_eq!(order.status, OrderStatus::Delivered);
}

#[test]
fn dispute_and_refund_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let admin = Address::generate(&env);
    let asset = Address::generate(&env);

    let order_id = create_order(
        env.clone(),
        buyer.clone(),
        seller.clone(),
        asset,
        50,
    );

    dispute_order(env.clone(), buyer.clone(), order_id);
    resolve_dispute(env.clone(), admin, order_id, true);

    let order = get_order(env, order_id);
    assert_eq!(order.status, OrderStatus::Refunded);
}
