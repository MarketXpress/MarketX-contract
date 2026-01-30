#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, BytesN, Env, String,
};

use crate::{ContractRegistry, ContractRegistryClient};
use crate::types::{ContractStatus, Permission};

fn setup_test() -> (Env, Address, Address, ContractRegistryClient<'static>) {
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

    let contract_id = env.register(ContractRegistry, ());
    let client = ContractRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let factory = Address::generate(&env);

    (env, admin, factory, client)
}

fn generate_wasm_hash(env: &Env, seed: u8) -> BytesN<32> {
    let mut bytes = [0u8; 32];
    bytes[0] = seed;
    bytes[1] = seed + 1;
    bytes[2] = seed + 2;
    BytesN::from_array(env, &bytes)
}

#[test]
fn test_registry_initialization() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_factory(), factory);

    let stats = client.get_stats();
    assert_eq!(stats.total_contracts, 0);
    assert_eq!(stats.active_contracts, 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #100)")] // AlreadyInitialized
fn test_double_initialization() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);
    client.initialize(&admin, &factory);
}

#[test]
fn test_register_contract() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    let contract_address = Address::generate(&env);
    let deployer = Address::generate(&env);
    let wasm_hash = generate_wasm_hash(&env, 1);
    let template_id: u32 = 1;
    let version: u32 = 1;

    // Register contract (as factory)
    let registry_id = client.register_contract(
        &factory,
        &contract_address,
        &deployer,
        &template_id,
        &wasm_hash,
        &version,
    );

    assert_eq!(registry_id, 1);
    assert_eq!(client.get_total_contracts(), 1);

    // Verify contract
    let contract = client.get_contract(&contract_address);
    assert_eq!(contract.registry_id, registry_id);
    assert_eq!(contract.contract_address, contract_address);
    assert_eq!(contract.deployer, deployer);
    assert_eq!(contract.template_id, template_id);
    assert_eq!(contract.version, version);
    assert_eq!(contract.status, ContractStatus::Active);

    // Verify stats
    let stats = client.get_stats();
    assert_eq!(stats.total_contracts, 1);
    assert_eq!(stats.active_contracts, 1);
}

#[test]
#[should_panic(expected = "Error(Contract, #109)")] // CallerNotFactory
fn test_register_unauthorized() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    let non_factory = Address::generate(&env);
    let contract_address = Address::generate(&env);
    let deployer = Address::generate(&env);
    let wasm_hash = generate_wasm_hash(&env, 1);

    // Non-factory trying to register should fail
    client.register_contract(
        &non_factory,
        &contract_address,
        &deployer,
        &1,
        &wasm_hash,
        &1,
    );
}

#[test]
fn test_query_by_deployer() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    let deployer = Address::generate(&env);

    // Register multiple contracts from same deployer
    for i in 1..=3u8 {
        let contract_address = Address::generate(&env);
        let wasm_hash = generate_wasm_hash(&env, i);

        client.register_contract(
            &factory,
            &contract_address,
            &deployer,
            &1,
            &wasm_hash,
            &1,
        );
    }

    let contracts = client.get_contracts_by_deployer(&deployer);
    assert_eq!(contracts.len(), 3);
}

#[test]
fn test_query_by_template() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    let template_id: u32 = 5;

    // Register contracts with same template
    for i in 1..=2u8 {
        let contract_address = Address::generate(&env);
        let deployer = Address::generate(&env);
        let wasm_hash = generate_wasm_hash(&env, i);

        client.register_contract(
            &factory,
            &contract_address,
            &deployer,
            &template_id,
            &wasm_hash,
            &1,
        );
    }

    let contracts = client.get_contracts_by_template(&template_id);
    assert_eq!(contracts.len(), 2);
}

#[test]
fn test_archive_and_restore() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    let contract_address = Address::generate(&env);
    let deployer = Address::generate(&env);
    let wasm_hash = generate_wasm_hash(&env, 1);

    client.register_contract(
        &factory,
        &contract_address,
        &deployer,
        &1,
        &wasm_hash,
        &1,
    );

    // Archive
    let reason = String::from_str(&env, "deprecated");
    client.archive_contract(&admin, &contract_address, &reason);

    let contract = client.get_contract(&contract_address);
    assert_eq!(contract.status, ContractStatus::Archived);
    assert_eq!(contract.archive_reason, Some(reason));

    // Verify stats
    let stats = client.get_stats();
    assert_eq!(stats.active_contracts, 0);
    assert_eq!(stats.archived_contracts, 1);

    // Validate should return false for archived
    assert_eq!(client.validate_contract(&contract_address), false);

    // Restore
    client.restore_contract(&admin, &contract_address);

    let contract = client.get_contract(&contract_address);
    assert_eq!(contract.status, ContractStatus::Active);
    assert_eq!(contract.archive_reason, None);

    // Validate should return true after restore
    assert_eq!(client.validate_contract(&contract_address), true);
}

#[test]
fn test_delete_contract() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    let contract_address = Address::generate(&env);
    let deployer = Address::generate(&env);
    let wasm_hash = generate_wasm_hash(&env, 1);

    client.register_contract(
        &factory,
        &contract_address,
        &deployer,
        &1,
        &wasm_hash,
        &1,
    );

    // Delete
    client.delete_contract(&admin, &contract_address);

    let contract = client.get_contract(&contract_address);
    assert_eq!(contract.status, ContractStatus::Deleted);

    // Contract should no longer appear in deployer list
    let contracts = client.get_contracts_by_deployer(&deployer);
    assert_eq!(contracts.len(), 0);

    // Verify stats
    let stats = client.get_stats();
    assert_eq!(stats.active_contracts, 0);
    assert_eq!(stats.deleted_contracts, 1);
}

#[test]
fn test_permissions() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    let contract_address = Address::generate(&env);
    let deployer = Address::generate(&env);
    let wasm_hash = generate_wasm_hash(&env, 1);

    client.register_contract(
        &factory,
        &contract_address,
        &deployer,
        &1,
        &wasm_hash,
        &1,
    );

    // Link permission
    client.link_permission(&admin, &contract_address, &Permission::CanCallFactory);
    client.link_permission(&admin, &contract_address, &Permission::CanCollectFees);

    // Check permissions
    assert_eq!(client.has_permission(&contract_address, &Permission::CanCallFactory), true);
    assert_eq!(client.has_permission(&contract_address, &Permission::CanCollectFees), true);
    assert_eq!(client.has_permission(&contract_address, &Permission::CanCallMarketplace), false);

    // Get all permissions
    let perms = client.get_permissions(&contract_address);
    assert_eq!(perms.len(), 2);

    // Revoke permission
    client.revoke_permission(&admin, &contract_address, &Permission::CanCallFactory);

    assert_eq!(client.has_permission(&contract_address, &Permission::CanCallFactory), false);
    assert_eq!(client.has_permission(&contract_address, &Permission::CanCollectFees), true);
}

#[test]
fn test_update_status() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    let contract_address = Address::generate(&env);
    let deployer = Address::generate(&env);
    let wasm_hash = generate_wasm_hash(&env, 1);

    client.register_contract(
        &factory,
        &contract_address,
        &deployer,
        &1,
        &wasm_hash,
        &1,
    );

    // Update to Paused
    client.update_status(&admin, &contract_address, &ContractStatus::Paused);

    let contract = client.get_contract(&contract_address);
    assert_eq!(contract.status, ContractStatus::Paused);

    let stats = client.get_stats();
    assert_eq!(stats.active_contracts, 0);
    assert_eq!(stats.paused_contracts, 1);

    // Update back to Active
    client.update_status(&admin, &contract_address, &ContractStatus::Active);

    let contract = client.get_contract(&contract_address);
    assert_eq!(contract.status, ContractStatus::Active);
}

#[test]
fn test_is_registered() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    let contract_address = Address::generate(&env);
    let deployer = Address::generate(&env);
    let wasm_hash = generate_wasm_hash(&env, 1);

    // Not registered yet
    assert_eq!(client.is_registered(&contract_address), false);

    client.register_contract(
        &factory,
        &contract_address,
        &deployer,
        &1,
        &wasm_hash,
        &1,
    );

    // Now registered
    assert_eq!(client.is_registered(&contract_address), true);
}

#[test]
fn test_get_contract_by_id() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    let contract_address = Address::generate(&env);
    let deployer = Address::generate(&env);
    let wasm_hash = generate_wasm_hash(&env, 1);

    let registry_id = client.register_contract(
        &factory,
        &contract_address,
        &deployer,
        &1,
        &wasm_hash,
        &1,
    );

    let contract = client.get_contract_by_id(&registry_id);
    assert_eq!(contract.contract_address, contract_address);
}

#[test]
fn test_admin_transfer() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    let new_admin = Address::generate(&env);
    client.set_admin(&admin, &new_admin);

    assert_eq!(client.get_admin(), new_admin);
}

#[test]
fn test_factory_update() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    let new_factory = Address::generate(&env);
    client.set_factory(&admin, &new_factory);

    assert_eq!(client.get_factory(), new_factory);
}

#[test]
#[should_panic(expected = "Error(Contract, #104)")] // ContractAlreadyRegistered
fn test_register_duplicate() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    let contract_address = Address::generate(&env);
    let deployer = Address::generate(&env);
    let wasm_hash = generate_wasm_hash(&env, 1);

    client.register_contract(
        &factory,
        &contract_address,
        &deployer,
        &1,
        &wasm_hash,
        &1,
    );

    // Duplicate should fail
    client.register_contract(
        &factory,
        &contract_address,
        &deployer,
        &1,
        &wasm_hash,
        &1,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #107)")] // InvalidStatus
fn test_restore_non_archived() {
    let (env, admin, factory, client) = setup_test();

    client.initialize(&admin, &factory);

    let contract_address = Address::generate(&env);
    let deployer = Address::generate(&env);
    let wasm_hash = generate_wasm_hash(&env, 1);

    client.register_contract(
        &factory,
        &contract_address,
        &deployer,
        &1,
        &wasm_hash,
        &1,
    );

    // Try to restore non-archived contract
    client.restore_contract(&admin, &contract_address);
}
