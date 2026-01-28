use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, String, Symbol};

use crate::escrow;

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
        asset,
        amount,
        status: OrderStatus::Created,
        shipping_ref: None,
    };

    let mut orders = load_orders(&env);
    orders.set(id, order);
    save_orders(&env, orders);

    escrow::lock_funds(&env, buyer, seller); // lock funds at creation
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
    escrow::refund_buyer(&env, buyer.clone());
    orders.set(order_id, order);
    save_orders(&env, orders);
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
    order.shipping_ref = Some(shipping_ref);

    orders.set(order_id, order);
    save_orders(&env, orders);
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
    escrow::release_funds(&env, order.seller.clone());
    orders.set(order_id, order);
    save_orders(&env, orders);
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
}

pub fn resolve_dispute(env: Env, _admin: Address, order_id: u64, refund: bool) {
    // Admin auth can be added here
    let mut orders = load_orders(&env);
    let mut order = orders.get(order_id).expect("Order not found");

    if order.status != OrderStatus::Disputed {
        panic!("Invalid state");
    }

    if refund {
        order.status = OrderStatus::Refunded;
        escrow::refund_buyer(&env, order.buyer.clone());
    } else {
        order.status = OrderStatus::Delivered;
        escrow::release_funds(&env, order.seller.clone());
    }

    orders.set(order_id, order);
    save_orders(&env, orders);
}

pub fn get_order(env: Env, order_id: u64) -> Order {
    let orders = load_orders(&env);
    orders.get(order_id).expect("Order not found")
}
