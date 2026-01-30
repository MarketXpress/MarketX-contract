use soroban_sdk::{Address, BytesN, Env, String, Vec};
use crate::types::{ContractTemplate, FeeConfig, StorageKey};

// TTL constants (similar to marketplace pattern)
const DAY_IN_LEDGERS: u32 = 17280; // ~5 second block time
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

// ========== Registry ==========

pub fn get_registry(env: &Env) -> Address {
    env.storage().instance().get(&StorageKey::Registry).unwrap()
}

pub fn set_registry(env: &Env, registry: &Address) {
    env.storage().instance().set(&StorageKey::Registry, registry);
}

// ========== Paused State ==========

pub fn is_paused(env: &Env) -> bool {
    env.storage().instance().get(&StorageKey::Paused).unwrap_or(false)
}

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&StorageKey::Paused, &paused);
}

// ========== Template Counter ==========

pub fn get_template_counter(env: &Env) -> u32 {
    env.storage().instance().get(&StorageKey::TemplateCounter).unwrap_or(0)
}

pub fn increment_template_counter(env: &Env) -> u32 {
    let counter = get_template_counter(env) + 1;
    env.storage().instance().set(&StorageKey::TemplateCounter, &counter);
    counter
}

// ========== Deployment Counter ==========

pub fn get_deployment_counter(env: &Env) -> u64 {
    env.storage().instance().get(&StorageKey::DeploymentCounter).unwrap_or(0)
}

pub fn increment_deployment_counter(env: &Env) -> u64 {
    let counter = get_deployment_counter(env) + 1;
    env.storage().instance().set(&StorageKey::DeploymentCounter, &counter);
    counter
}

// ========== Templates ==========

pub fn get_template(env: &Env, template_id: u32) -> Option<ContractTemplate> {
    let key = StorageKey::Template(template_id);
    let template = env.storage().persistent().get::<_, ContractTemplate>(&key);
    if template.is_some() {
        env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    template
}

pub fn set_template(env: &Env, template: &ContractTemplate) {
    let key = StorageKey::Template(template.id);
    env.storage().persistent().set(&key, template);
    env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);

    // Also index by hash
    let hash_key = StorageKey::TemplateByHash(template.wasm_hash.clone());
    env.storage().persistent().set(&hash_key, &template.id);
    env.storage().persistent().extend_ttl(&hash_key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

pub fn get_template_id_by_hash(env: &Env, wasm_hash: &BytesN<32>) -> Option<u32> {
    let key = StorageKey::TemplateByHash(wasm_hash.clone());
    let id = env.storage().persistent().get::<_, u32>(&key);
    if id.is_some() {
        env.storage().persistent().extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    id
}

pub fn list_templates(env: &Env) -> Vec<ContractTemplate> {
    let counter = get_template_counter(env);
    let mut templates = Vec::new(env);

    for i in 1..=counter {
        if let Some(template) = get_template(env, i) {
            templates.push_back(template);
        }
    }

    templates
}

// ========== Fee Config ==========

pub fn get_fee_config(env: &Env) -> Option<FeeConfig> {
    env.storage().instance().get(&StorageKey::FeeConfig)
}

pub fn set_fee_config(env: &Env, config: &FeeConfig) {
    env.storage().instance().set(&StorageKey::FeeConfig, config);
}
