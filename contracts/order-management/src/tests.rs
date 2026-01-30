#![cfg(test)]

use soroban_sdk::{Env, String};
use soroban_sdk::testutils::Address as _;

use crate::{OrderManagement, OrderManagementClient};
use crate::order::OrderStatus;

#[test]
fn full_order_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(OrderManagement, ());
    let client = OrderManagementClient::new(&env, &contract_id);

    let buyer = soroban_sdk::Address::generate(&env);
    let seller = soroban_sdk::Address::generate(&env);
    let asset = soroban_sdk::Address::generate(&env);

    let order_id = client.create_order(&buyer, &seller, &asset, &100);

    client.ship_order(&seller, &order_id, &String::from_str(&env, "TRACK123"));

    client.deliver_order(&buyer, &order_id);

    let order = client.get_order(&order_id);
    assert_eq!(order.status, OrderStatus::Delivered);
}

#[test]
fn dispute_and_refund_flow() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(OrderManagement, ());
    let client = OrderManagementClient::new(&env, &contract_id);

    let buyer = soroban_sdk::Address::generate(&env);
    let seller = soroban_sdk::Address::generate(&env);
    let admin = soroban_sdk::Address::generate(&env);
    let asset = soroban_sdk::Address::generate(&env);

    let order_id = client.create_order(&buyer, &seller, &asset, &50);

    // Ship order first (required to dispute)
    client.ship_order(&seller, &order_id, &String::from_str(&env, "TRACK456"));

    client.dispute_order(&buyer, &order_id);
    client.resolve_dispute(&admin, &order_id, &true);

    let order = client.get_order(&order_id);
    assert_eq!(order.status, OrderStatus::Refunded);
}

#[test]
fn cancel_order_flow() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(OrderManagement, ());
    let client = OrderManagementClient::new(&env, &contract_id);

    let buyer = soroban_sdk::Address::generate(&env);
    let seller = soroban_sdk::Address::generate(&env);
    let asset = soroban_sdk::Address::generate(&env);

    let order_id = client.create_order(&buyer, &seller, &asset, &100);

    client.cancel_order(&buyer, &order_id);

    let order = client.get_order(&order_id);
    assert_eq!(order.status, OrderStatus::Cancelled);
}

#[test]
fn dispute_resolved_to_seller() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(OrderManagement, ());
    let client = OrderManagementClient::new(&env, &contract_id);

    let buyer = soroban_sdk::Address::generate(&env);
    let seller = soroban_sdk::Address::generate(&env);
    let admin = soroban_sdk::Address::generate(&env);
    let asset = soroban_sdk::Address::generate(&env);

    let order_id = client.create_order(&buyer, &seller, &asset, &75);

    client.ship_order(&seller, &order_id, &String::from_str(&env, "TRACK789"));

    client.dispute_order(&buyer, &order_id);
    
    // Resolve in favor of seller (no refund)
    client.resolve_dispute(&admin, &order_id, &false);

    let order = client.get_order(&order_id);
    assert_eq!(order.status, OrderStatus::Delivered);
}

#[test]
fn test_multiple_orders() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(OrderManagement, ());
    let client = OrderManagementClient::new(&env, &contract_id);

    let buyer = soroban_sdk::Address::generate(&env);
    let seller = soroban_sdk::Address::generate(&env);
    let asset = soroban_sdk::Address::generate(&env);

    // Create multiple orders
    let order_id_1 = client.create_order(&buyer, &seller, &asset, &100);
    let order_id_2 = client.create_order(&buyer, &seller, &asset, &200);
    let order_id_3 = client.create_order(&buyer, &seller, &asset, &300);

    // Verify orders have sequential IDs
    assert_eq!(order_id_1, 0);
    assert_eq!(order_id_2, 1);
    assert_eq!(order_id_3, 2);
    
    // Verify all orders can be retrieved
    let order1 = client.get_order(&order_id_1);
    let order2 = client.get_order(&order_id_2);
    let order3 = client.get_order(&order_id_3);
    
    assert_eq!(order1.amount, 100);
    assert_eq!(order2.amount, 200);
    assert_eq!(order3.amount, 300);
}

#[test]
fn test_order_amounts() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(OrderManagement, ());
    let client = OrderManagementClient::new(&env, &contract_id);

    let buyer = soroban_sdk::Address::generate(&env);
    let seller = soroban_sdk::Address::generate(&env);
    let asset = soroban_sdk::Address::generate(&env);

    // Test various amounts
    let order_id = client.create_order(&buyer, &seller, &asset, &1_000_000);
    let order = client.get_order(&order_id);
    
    assert_eq!(order.amount, 1_000_000);
    assert_eq!(order.buyer, buyer);
    assert_eq!(order.seller, seller);
    assert_eq!(order.asset, asset);
}

