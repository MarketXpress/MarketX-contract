use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, String, Symbol};

use crate::escrow;
use crate::events::{
    emit_order_created, emit_order_cancelled, emit_order_shipped,
    emit_order_delivered, emit_order_disputed, emit_dispute_resolved,
    emit_user_action, OrderCancelReason, UserAction,
};

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum OrderStatus {
    Created,
    Shipped,
    Delivered,
    Cancelled,
    Disputed,
    Refunded,
}

#[contracttype]
#[derive(Clone)]
pub struct Order {
    pub id: u64,
    pub buyer: Address,
    pub seller: Address,
    pub asset: Address,
    pub amount: i128,
    pub status: OrderStatus,
    pub shipping_ref: Option<String>,
}

const ORDERS: Symbol = symbol_short!("ORDERS");
const NEXT_ID: Symbol = symbol_short!("NEXT_ID");

fn next_id(env: &Env) -> u64 {
    let id: u64 = env.storage().persistent().get(&NEXT_ID).unwrap_or(0);
    env.storage().persistent().set(&NEXT_ID, &(id + 1));
    id
}

fn load_orders(env: &Env) -> Map<u64, Order> {
    env.storage().persistent().get(&ORDERS).unwrap_or(Map::new(env))
}

fn save_orders(env: &Env, orders: Map<u64, Order>) {
    env.storage().persistent().set(&ORDERS, &orders);
}

pub fn create_order(
    env: Env,
    buyer: Address,
    seller: Address,
    asset: Address,
    amount: i128,
) -> u64 {
    buyer.require_auth();

    let id = next_id(&env);

    let order = Order {
        id,
        buyer: buyer.clone(),
        seller: seller.clone(),
        asset: asset.clone(),
        amount,
        status: OrderStatus::Created,
        shipping_ref: None,
    };

    let mut orders = load_orders(&env);
    orders.set(id, order);
    save_orders(&env, orders);

    escrow::lock_funds(&env, buyer.clone(), seller.clone(), asset.clone(), amount, id);
    
    // Emit order created event
    emit_order_created(&env, id, buyer.clone(), seller.clone(), asset, amount);
    emit_user_action(&env, buyer, UserAction::CreateOrder, id);
    
    id
}

pub fn cancel_order(env: Env, buyer: Address, order_id: u64) {
    buyer.require_auth();

    let mut orders = load_orders(&env);
    let mut order = orders.get(order_id).expect("Order not found");

    if buyer != order.buyer {
        panic!("Unauthorized");
    }

    if order.status != OrderStatus::Created {
        panic!("Invalid state");
    }

    order.status = OrderStatus::Cancelled;
    escrow::refund_buyer(&env, buyer.clone(), order.asset.clone(), order.amount, order_id);
    orders.set(order_id, order);
    save_orders(&env, orders);
    
    // Emit order cancelled event
    emit_order_cancelled(&env, order_id, buyer.clone(), OrderCancelReason::BuyerRequested);
    emit_user_action(&env, buyer, UserAction::CancelOrder, order_id);
}

pub fn ship_order(env: Env, seller: Address, order_id: u64, shipping_ref: String) {
    seller.require_auth();

    let mut orders = load_orders(&env);
    let mut order = orders.get(order_id).expect("Order not found");

    if seller != order.seller {
        panic!("Unauthorized");
    }

    if order.status != OrderStatus::Created {
        panic!("Invalid state");
    }

    order.status = OrderStatus::Shipped;
    order.shipping_ref = Some(shipping_ref.clone());

    orders.set(order_id, order);
    save_orders(&env, orders);
    
    // Emit order shipped event
    emit_order_shipped(&env, order_id, seller.clone(), shipping_ref);
    emit_user_action(&env, seller, UserAction::ShipOrder, order_id);
}

pub fn deliver_order(env: Env, buyer: Address, order_id: u64) {
    buyer.require_auth();

    let mut orders = load_orders(&env);
    let mut order = orders.get(order_id).expect("Order not found");

    if buyer != order.buyer {
        panic!("Unauthorized");
    }

    if order.status != OrderStatus::Shipped {
        panic!("Invalid state");
    }

    order.status = OrderStatus::Delivered;
    escrow::release_funds(&env, order.seller.clone(), order.asset.clone(), order.amount, order_id);
    orders.set(order_id, order.clone());
    save_orders(&env, orders);
    
    // Emit order delivered event
    emit_order_delivered(&env, order_id, buyer.clone(), order.seller.clone(), order.amount);
    emit_user_action(&env, buyer, UserAction::ConfirmDelivery, order_id);
}

pub fn dispute_order(env: Env, buyer: Address, order_id: u64) {
    buyer.require_auth();

    let mut orders = load_orders(&env);
    let mut order = orders.get(order_id).expect("Order not found");

    if buyer != order.buyer {
        panic!("Unauthorized");
    }

    if order.status != OrderStatus::Shipped {
        panic!("Invalid state");
    }

    order.status = OrderStatus::Disputed;
    orders.set(order_id, order);
    save_orders(&env, orders);
    
    // Emit order disputed event
    emit_order_disputed(&env, order_id, buyer.clone());
    emit_user_action(&env, buyer, UserAction::RaiseDispute, order_id);
}

pub fn resolve_dispute(env: Env, admin: Address, order_id: u64, refund: bool) {
    // Admin auth can be added here
    let mut orders = load_orders(&env);
    let mut order = orders.get(order_id).expect("Order not found");

    if order.status != OrderStatus::Disputed {
        panic!("Invalid state");
    }

    if refund {
        order.status = OrderStatus::Refunded;
        escrow::refund_buyer(&env, order.buyer.clone(), order.asset.clone(), order.amount, order_id);
    } else {
        order.status = OrderStatus::Delivered;
        escrow::release_funds(&env, order.seller.clone(), order.asset.clone(), order.amount, order_id);
    }

    orders.set(order_id, order);
    save_orders(&env, orders);
    
    // Emit dispute resolved event
    emit_dispute_resolved(&env, order_id, admin, refund);
}

pub fn get_order(env: Env, order_id: u64) -> Order {
    let orders = load_orders(&env);
    orders.get(order_id).expect("Order not found")
}
