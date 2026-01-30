#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, BytesN, Env, String,
};

use crate::{EscrowFactory, EscrowFactoryClient};

fn setup_test() -> (Env, Address, EscrowFactoryClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    // Set ledger time with current protocol version
    env.ledger().set(LedgerInfo {
        timestamp: 1000,
        protocol_version: 23,
        sequence_number: 1,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 100,
        max_entry_ttl: 1000000,
    });

    let contract_id = env.register(EscrowFactory, ());
    let client = EscrowFactoryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    (env, admin, client)
}

fn generate_wasm_hash(env: &Env) -> BytesN<32> {
    let mut bytes = [0u8; 32];
    bytes[0] = 1;
    bytes[1] = 2;
    bytes[2] = 3;
    BytesN::from_array(env, &bytes)
}

#[test]
fn test_factory_initialization() {
    let (env, admin, client) = setup_test();

    let registry = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let fee_collector = Address::generate(&env);
    let deployment_fee: i128 = 100;

    // Initialize factory
    client.initialize(&admin, &registry, &deployment_fee, &fee_token, &fee_collector);

    // Verify initialization
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_registry(), registry);
    assert_eq!(client.is_paused(), false);

    let fee_config = client.get_fee_config();
    assert_eq!(fee_config.deployment_fee, deployment_fee);
    assert_eq!(fee_config.fee_token, fee_token);
    assert_eq!(fee_config.fee_collector, fee_collector);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")] // AlreadyInitialized
fn test_double_initialization() {
    let (env, admin, client) = setup_test();

    let registry = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    client.initialize(&admin, &registry, &100, &fee_token, &fee_collector);

    // Second initialization should fail
    client.initialize(&admin, &registry, &100, &fee_token, &fee_collector);
}

#[test]
fn test_register_template() {
    let (env, admin, client) = setup_test();

    let registry = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    client.initialize(&admin, &registry, &100, &fee_token, &fee_collector);

    let wasm_hash = generate_wasm_hash(&env);
    let name = String::from_str(&env, "escrow-v1");
    let version: u32 = 1;

    // Register template
    let template_id = client.register_template(&admin, &wasm_hash, &name, &version);

    assert_eq!(template_id, 1);
    assert_eq!(client.get_template_count(), 1);

    // Verify template
    let template = client.get_template(&template_id);
    assert_eq!(template.id, template_id);
    assert_eq!(template.wasm_hash, wasm_hash);
    assert_eq!(template.name, name);
    assert_eq!(template.version, version);
    assert_eq!(template.is_active, true);
    assert_eq!(template.deployment_count, 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")] // TemplateAlreadyExists
fn test_register_duplicate_template() {
    let (env, admin, client) = setup_test();

    let registry = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    client.initialize(&admin, &registry, &100, &fee_token, &fee_collector);

    let wasm_hash = generate_wasm_hash(&env);
    let name = String::from_str(&env, "escrow-v1");

    client.register_template(&admin, &wasm_hash, &name, &1);

    // Registering same hash should fail
    client.register_template(&admin, &wasm_hash, &name, &2);
}

#[test]
fn test_deactivate_template() {
    let (env, admin, client) = setup_test();

    let registry = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    client.initialize(&admin, &registry, &100, &fee_token, &fee_collector);

    let wasm_hash = generate_wasm_hash(&env);
    let name = String::from_str(&env, "escrow-v1");

    let template_id = client.register_template(&admin, &wasm_hash, &name, &1);

    // Deactivate
    client.deactivate_template(&admin, &template_id);

    let template = client.get_template(&template_id);
    assert_eq!(template.is_active, false);

    // Reactivate
    client.activate_template(&admin, &template_id);

    let template = client.get_template(&template_id);
    assert_eq!(template.is_active, true);
}

#[test]
fn test_list_templates() {
    let (env, admin, client) = setup_test();

    let registry = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    client.initialize(&admin, &registry, &100, &fee_token, &fee_collector);

    // Register multiple templates
    for i in 1..=3 {
        let mut bytes = [0u8; 32];
        bytes[0] = i;
        let wasm_hash = BytesN::from_array(&env, &bytes);
        let name = String::from_str(&env, "escrow");
        client.register_template(&admin, &wasm_hash, &name, &(i as u32));
    }

    let templates = client.list_templates();
    assert_eq!(templates.len(), 3);
}

#[test]
fn test_update_fee_config() {
    let (env, admin, client) = setup_test();

    let registry = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    client.initialize(&admin, &registry, &100, &fee_token, &fee_collector);

    // Update deployment fee
    let new_fee: i128 = 200;
    client.set_deployment_fee(&admin, &new_fee);

    let config = client.get_fee_config();
    assert_eq!(config.deployment_fee, new_fee);

    // Update fee collector
    let new_collector = Address::generate(&env);
    client.set_fee_collector(&admin, &new_collector);

    let config = client.get_fee_config();
    assert_eq!(config.fee_collector, new_collector);
}

#[test]
fn test_pause_factory() {
    let (env, admin, client) = setup_test();

    let registry = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    client.initialize(&admin, &registry, &100, &fee_token, &fee_collector);

    // Pause
    client.set_paused(&admin, &true);
    assert_eq!(client.is_paused(), true);

    // Unpause
    client.set_paused(&admin, &false);
    assert_eq!(client.is_paused(), false);
}

#[test]
fn test_admin_transfer() {
    let (env, admin, client) = setup_test();

    let registry = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    client.initialize(&admin, &registry, &100, &fee_token, &fee_collector);

    let new_admin = Address::generate(&env);
    client.set_admin(&admin, &new_admin);

    assert_eq!(client.get_admin(), new_admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // Unauthorized
fn test_unauthorized_template_registration() {
    let (env, admin, client) = setup_test();

    let registry = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    client.initialize(&admin, &registry, &100, &fee_token, &fee_collector);

    // Try to register with non-admin
    let non_admin = Address::generate(&env);
    let wasm_hash = generate_wasm_hash(&env);
    let name = String::from_str(&env, "escrow");

    client.register_template(&non_admin, &wasm_hash, &name, &1);
}

#[test]
#[should_panic(expected = "Error(Contract, #13)")] // InvalidVersion
fn test_invalid_version() {
    let (env, admin, client) = setup_test();

    let registry = Address::generate(&env);
    let fee_token = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    client.initialize(&admin, &registry, &100, &fee_token, &fee_collector);

    let wasm_hash = generate_wasm_hash(&env);
    let name = String::from_str(&env, "escrow");

    // Version 0 should fail
    client.register_template(&admin, &wasm_hash, &name, &0);
}
