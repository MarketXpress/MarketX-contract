use marketx::{Contract, ContractClient, DataKey};
use soroban_sdk::{
    testutils::{storage::Persistent as _, Address as _},
    Address, Bytes, Env,
};

#[test]
fn bump_escrow_extends_ttl_via_public_api() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Contract, ());
    let client = ContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let token = Address::generate(&env);

    client.initialize(&admin, &admin, &250);
    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &token,
        &1000,
        &Some(Bytes::from_slice(&env, b"integration-ttl")),
        &None,
        &None,
    );

    let escrow_key = DataKey::Escrow(escrow_id);
    let before_ttl = env.as_contract(&contract_id, || {
        env.storage().persistent().get_ttl(&escrow_key)
    });

    client.bump_escrow(&escrow_id);

    let after_ttl = env.as_contract(&contract_id, || {
        env.storage().persistent().get_ttl(&escrow_key)
    });

    assert!(after_ttl > before_ttl);
}
