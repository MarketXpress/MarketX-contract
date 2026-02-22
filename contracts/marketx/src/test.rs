#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn make_escrow(env: &Env) -> (Escrow, Address, Address, Address) {
    let buyer = Address::generate(env);
    let seller = Address::generate(env);
    let token = Address::generate(env);
    let escrow = Escrow {
        buyer: buyer.clone(),
        seller: seller.clone(),
        token: token.clone(),
        amount: 5_000_000,
        status: EscrowStatus::Pending,
    };
    (escrow, buyer, seller, token)
}

#[test]
fn test_store_and_retrieve_escrow() {
    let env = Env::default();
    let contract_id = env.register(Contract, ());
    let client = ContractClient::new(&env, &contract_id);

    let (escrow, buyer, seller, token) = make_escrow(&env);

    client.store_escrow(&1u64, &escrow);
    let retrieved = client.get_escrow(&1u64);

    assert_eq!(retrieved.buyer, buyer);
    assert_eq!(retrieved.seller, seller);
    assert_eq!(retrieved.token, token);
    assert_eq!(retrieved.amount, 5_000_000);
    assert_eq!(retrieved.status, EscrowStatus::Pending);
}

#[test]
fn test_multiple_escrows_stored_independently() {
    let env = Env::default();
    let contract_id = env.register(Contract, ());
    let client = ContractClient::new(&env, &contract_id);

    let (escrow_a, buyer_a, _, _) = make_escrow(&env);
    let (mut escrow_b, buyer_b, _, _) = make_escrow(&env);
    escrow_b.amount = 9_999_999;
    escrow_b.status = EscrowStatus::Released;

    client.store_escrow(&1u64, &escrow_a);
    client.store_escrow(&2u64, &escrow_b);

    let r_a = client.get_escrow(&1u64);
    let r_b = client.get_escrow(&2u64);

    assert_eq!(r_a.buyer, buyer_a);
    assert_eq!(r_a.status, EscrowStatus::Pending);

    assert_eq!(r_b.buyer, buyer_b);
    assert_eq!(r_b.amount, 9_999_999);
    assert_eq!(r_b.status, EscrowStatus::Released);
}

#[test]
fn test_escrow_status_variants_round_trip() {
    let env = Env::default();
    let contract_id = env.register(Contract, ());
    let client = ContractClient::new(&env, &contract_id);

    let statuses = [
        EscrowStatus::Pending,
        EscrowStatus::Released,
        EscrowStatus::Refunded,
        EscrowStatus::Disputed,
    ];

    for (id, status) in statuses.iter().enumerate() {
        let (mut escrow, _, _, _) = make_escrow(&env);
        escrow.status = status.clone();
        client.store_escrow(&(id as u64), &escrow);
        let retrieved = client.get_escrow(&(id as u64));
        assert_eq!(&retrieved.status, status);
    }
}
