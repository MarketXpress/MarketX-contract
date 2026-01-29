#![no_std]

mod order;
mod escrow;
pub mod events;

#[cfg(test)]
mod tests;

use soroban_sdk::{contract, contractimpl, Env, Address};
use order::Order;

#[contract]
pub struct OrderManagement;

#[contractimpl]
impl OrderManagement {
    pub fn create_order(
        env: Env,
        buyer: Address,
        seller: Address,
        asset: Address,
        amount: i128,
    ) -> u64 {
        order::create_order(env, buyer, seller, asset, amount)
    }

    pub fn cancel_order(env: Env, buyer: Address, order_id: u64) {
        order::cancel_order(env, buyer, order_id)
    }

    pub fn ship_order(
        env: Env,
        seller: Address,
        order_id: u64,
        shipping_ref: soroban_sdk::String,
    ) {
        order::ship_order(env, seller, order_id, shipping_ref)
    }

    pub fn deliver_order(env: Env, buyer: Address, order_id: u64) {
        order::deliver_order(env, buyer, order_id)
    }

    pub fn dispute_order(env: Env, buyer: Address, order_id: u64) {
        order::dispute_order(env, buyer, order_id)
    }

    pub fn resolve_dispute(env: Env, admin: Address, order_id: u64, refund: bool) {
        order::resolve_dispute(env, admin, order_id, refund)
    }

    pub fn get_order(env: Env, order_id: u64) -> Order {
        order::get_order(env, order_id)
    }
}
