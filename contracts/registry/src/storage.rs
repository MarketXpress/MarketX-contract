use soroban_sdk::{Address, Env, Vec};
use crate::types::{ContractStatus, DeployedContract, Permission, RegistryConfig, RegistryStats, StorageKey};

// TTL constants
const DAY_IN_LEDGERS: u32 = 17280;
const PERSISTENT_TTL_AMOUNT: u32 = 90 * DAY_IN_LEDGERS;
const PERSISTENT_TTL_THRESHOLD: u32 = PERSISTENT_TTL_AMOUNT - DAY_IN_LEDGERS;

// ========== Initialization ==========

pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&StorageKey::Initialized)
}

pub fn set_initialized(env: &Env) {
    env.storage().instance().set(&StorageKey::Initialized, &true);
}

// ========== Admin ==========

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&StorageKey::Admin).unwrap()
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&StorageKey::Admin, admin);
}

// ========== Factory ==========

pub fn get_factory(env: &Env) -> Address {
    env.storage().instance().get(&StorageKey::Factory).unwrap()
}

pub fn set_factory(env: &Env, factory: &Address) {
    env.storage().instance().set(&StorageKey::Factory, factory);
}

// ========== Contract Counter ==========

pub fn get_contract_counter(env: &Env) -> u64 {
    env.storage().instance().get(&StorageKey::ContractCounter).unwrap_or(0)
}

pub fn increment_contract_counter(env: &Env) -> u64 {
    let counter = get_contract_counter(env) + 1;
    env.storage().instance().set(&StorageKey::ContractCounter, &counter);
    counter
}

// ========== Registry Config ==========

pub fn get_config(env: &Env) -> Option<RegistryConfig> {
    env.storage().instance().get(&StorageKey::Admin).map(|admin| {
        RegistryConfig {
            admin,
            factory: get_factory(env),
            updated_at: env.ledger().timestamp(),
        }
    })
}

// ========== Deployed Contracts ==========

pub fn get_contract(env: &Env, contract_address: &Address) -> Option<DeployedContract> {
    let key = StorageKey::Contract(contract_address.clone());
    let contract = env.storage().persistent().get::<_, DeployedContract>(&key);
    if contract.is_some() {
        env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    contract
}

pub fn set_contract(env: &Env, contract: &DeployedContract) {
    let key = StorageKey::Contract(contract.contract_address.clone());
    env.storage().persistent().set(&key, contract);
    env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);

    // Also index by ID
    let id_key = StorageKey::ContractById(contract.registry_id);
    env.storage().persistent().set(&id_key, &contract.contract_address);
    env.storage().persistent().extend_ttl(&id_key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

pub fn get_contract_by_id(env: &Env, registry_id: u64) -> Option<DeployedContract> {
    let id_key = StorageKey::ContractById(registry_id);
    let address: Option<Address> = env.storage().persistent().get(&id_key);
    if let Some(addr) = address {
        env.storage().persistent().extend_ttl(&id_key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
        return get_contract(env, &addr);
    }
    None
}

pub fn remove_contract(env: &Env, contract_address: &Address) {
    let key = StorageKey::Contract(contract_address.clone());
    if let Some(contract) = get_contract(env, contract_address) {
        let id_key = StorageKey::ContractById(contract.registry_id);
        env.storage().persistent().remove(&id_key);
    }
    env.storage().persistent().remove(&key);
}

// ========== Contracts by Deployer Index ==========

pub fn get_contracts_by_deployer(env: &Env, deployer: &Address) -> Vec<Address> {
    let key = StorageKey::ContractsByDeployer(deployer.clone());
    let contracts = env.storage().persistent().get::<_, Vec<Address>>(&key).unwrap_or(Vec::new(env));
    if !contracts.is_empty() {
        env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    contracts
}

pub fn add_contract_to_deployer(env: &Env, deployer: &Address, contract_address: &Address) {
    let key = StorageKey::ContractsByDeployer(deployer.clone());
    let mut contracts = get_contracts_by_deployer(env, deployer);
    contracts.push_back(contract_address.clone());
    env.storage().persistent().set(&key, &contracts);
    env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

pub fn remove_contract_from_deployer(env: &Env, deployer: &Address, contract_address: &Address) {
    let key = StorageKey::ContractsByDeployer(deployer.clone());
    let contracts = get_contracts_by_deployer(env, deployer);
    let mut new_contracts = Vec::new(env);
    for addr in contracts.iter() {
        if addr != contract_address.clone() {
            new_contracts.push_back(addr);
        }
    }
    env.storage().persistent().set(&key, &new_contracts);
}

// ========== Contracts by Template Index ==========

pub fn get_contracts_by_template(env: &Env, template_id: u32) -> Vec<Address> {
    let key = StorageKey::ContractsByTemplate(template_id);
    let contracts = env.storage().persistent().get::<_, Vec<Address>>(&key).unwrap_or(Vec::new(env));
    if !contracts.is_empty() {
        env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    contracts
}

pub fn add_contract_to_template(env: &Env, template_id: u32, contract_address: &Address) {
    let key = StorageKey::ContractsByTemplate(template_id);
    let mut contracts = get_contracts_by_template(env, template_id);
    contracts.push_back(contract_address.clone());
    env.storage().persistent().set(&key, &contracts);
    env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

pub fn remove_contract_from_template(env: &Env, template_id: u32, contract_address: &Address) {
    let key = StorageKey::ContractsByTemplate(template_id);
    let contracts = get_contracts_by_template(env, template_id);
    let mut new_contracts = Vec::new(env);
    for addr in contracts.iter() {
        if addr != contract_address.clone() {
            new_contracts.push_back(addr);
        }
    }
    env.storage().persistent().set(&key, &new_contracts);
}

// ========== Contracts by Version Index ==========

pub fn get_contracts_by_version(env: &Env, version: u32) -> Vec<Address> {
    let key = StorageKey::ContractsByVersion(version);
    let contracts = env.storage().persistent().get::<_, Vec<Address>>(&key).unwrap_or(Vec::new(env));
    if !contracts.is_empty() {
        env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    contracts
}

pub fn add_contract_to_version(env: &Env, version: u32, contract_address: &Address) {
    let key = StorageKey::ContractsByVersion(version);
    let mut contracts = get_contracts_by_version(env, version);
    contracts.push_back(contract_address.clone());
    env.storage().persistent().set(&key, &contracts);
    env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

pub fn remove_contract_from_version(env: &Env, version: u32, contract_address: &Address) {
    let key = StorageKey::ContractsByVersion(version);
    let contracts = get_contracts_by_version(env, version);
    let mut new_contracts = Vec::new(env);
    for addr in contracts.iter() {
        if addr != contract_address.clone() {
            new_contracts.push_back(addr);
        }
    }
    env.storage().persistent().set(&key, &new_contracts);
}

// ========== Permissions ==========

pub fn get_permissions(env: &Env, contract_address: &Address) -> Vec<Permission> {
    let key = StorageKey::ContractPermissions(contract_address.clone());
    env.storage().persistent().get::<_, Vec<Permission>>(&key).unwrap_or(Vec::new(env))
}

pub fn add_permission(env: &Env, contract_address: &Address, permission: Permission) {
    let key = StorageKey::ContractPermissions(contract_address.clone());
    let mut permissions = get_permissions(env, contract_address);

    // Check if permission already exists
    let mut exists = false;
    for p in permissions.iter() {
        if p == permission {
            exists = true;
            break;
        }
    }

    if !exists {
        permissions.push_back(permission);
        env.storage().persistent().set(&key, &permissions);
        env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
}

pub fn remove_permission(env: &Env, contract_address: &Address, permission: Permission) {
    let key = StorageKey::ContractPermissions(contract_address.clone());
    let permissions = get_permissions(env, contract_address);
    let mut new_permissions = Vec::new(env);

    for p in permissions.iter() {
        if p != permission {
            new_permissions.push_back(p);
        }
    }

    env.storage().persistent().set(&key, &new_permissions);
}

pub fn has_permission(env: &Env, contract_address: &Address, permission: Permission) -> bool {
    let permissions = get_permissions(env, contract_address);
    for p in permissions.iter() {
        if p == permission {
            return true;
        }
    }
    false
}

// ========== Stats ==========

pub fn get_stats(env: &Env) -> RegistryStats {
    env.storage().instance().get(&StorageKey::Stats).unwrap_or(RegistryStats {
        total_contracts: 0,
        active_contracts: 0,
        paused_contracts: 0,
        archived_contracts: 0,
        deleted_contracts: 0,
    })
}

pub fn set_stats(env: &Env, stats: &RegistryStats) {
    env.storage().instance().set(&StorageKey::Stats, stats);
}

pub fn update_stats_on_register(env: &Env) {
    let mut stats = get_stats(env);
    stats.total_contracts += 1;
    stats.active_contracts += 1;
    set_stats(env, &stats);
}

pub fn update_stats_on_status_change(env: &Env, old_status: ContractStatus, new_status: ContractStatus) {
    let mut stats = get_stats(env);

    // Decrement old status count
    match old_status {
        ContractStatus::Active => stats.active_contracts = stats.active_contracts.saturating_sub(1),
        ContractStatus::Paused => stats.paused_contracts = stats.paused_contracts.saturating_sub(1),
        ContractStatus::Archived => stats.archived_contracts = stats.archived_contracts.saturating_sub(1),
        ContractStatus::Deleted => stats.deleted_contracts = stats.deleted_contracts.saturating_sub(1),
    }

    // Increment new status count
    match new_status {
        ContractStatus::Active => stats.active_contracts += 1,
        ContractStatus::Paused => stats.paused_contracts += 1,
        ContractStatus::Archived => stats.archived_contracts += 1,
        ContractStatus::Deleted => stats.deleted_contracts += 1,
    }

    set_stats(env, &stats);
}
