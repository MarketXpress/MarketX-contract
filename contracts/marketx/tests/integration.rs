use marketx::{Contract, ContractClient, DataKey};
use soroban_sdk::{
    testutils::{storage::Persistent as _, Address as _},
    token::{Client as TokenClient, StellarAssetClient},
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

    client.initialize(&admin, &admin, &250, &0, &0);
    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &token,
        &1000,
        &Some(Bytes::from_slice(&env, b"integration-ttl")),
        &None,
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

#[test]
fn local_integration_uses_mock_token_contract_for_fund_release() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Contract, ());
    let client = ContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = StellarAssetClient::new(&env, &token_id.address());
    let token = TokenClient::new(&env, &token_id.address());

    client.initialize(&admin, &admin, &0, &0, &0);
    token_admin.mint(&buyer, &1000);

    let escrow_id = client.create_escrow(
        &buyer,
        &seller,
        &token_id.address(),
        &1000,
        &Some(Bytes::from_slice(&env, b"integration-mock-token")),
        &None,
        &None,
        &None,
    );

    client.fund_escrow(&escrow_id);
    assert_eq!(token.balance(&contract_id), 1000);

    client.release_escrow(&escrow_id);
    assert_eq!(token.balance(&seller), 1000);
    assert_eq!(token.balance(&contract_id), 0);
}
