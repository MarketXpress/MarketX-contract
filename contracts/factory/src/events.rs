use soroban_sdk::{contracttype, Address, BytesN, Env, String};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactoryInitializedEvent {
    pub admin: Address,
    pub registry: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateRegisteredEvent {
    pub template_id: u32,
    pub wasm_hash: BytesN<32>,
    pub name: String,
    pub version: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateDeactivatedEvent {
    pub template_id: u32,
    pub admin: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractDeployedEvent {
    pub contract_address: Address,
    pub deployer: Address,
    pub template_id: u32,
    pub deployment_id: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeCollectedEvent {
    pub deployer: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeConfigUpdatedEvent {
    pub deployment_fee: i128,
    pub fee_collector: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactoryPausedEvent {
    pub admin: Address,
    pub is_paused: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminChangedEvent {
    pub old_admin: Address,
    pub new_admin: Address,
}

pub fn emit_factory_initialized(env: &Env, admin: Address, registry: Address) {
    let event = FactoryInitializedEvent { admin: admin.clone(), registry };
    env.events().publish(("factory_initialized", admin), event);
}

pub fn emit_template_registered(env: &Env, template_id: u32, wasm_hash: BytesN<32>, name: String, version: u32) {
    let event = TemplateRegisteredEvent { template_id, wasm_hash, name, version };
    env.events().publish(("template_registered", template_id), event);
}

pub fn emit_template_deactivated(env: &Env, template_id: u32, admin: Address) {
    let event = TemplateDeactivatedEvent { template_id, admin: admin.clone() };
    env.events().publish(("template_deactivated", template_id), event);
}

pub fn emit_contract_deployed(env: &Env, contract_address: Address, deployer: Address, template_id: u32, deployment_id: u64) {
    let event = ContractDeployedEvent {
        contract_address: contract_address.clone(),
        deployer: deployer.clone(),
        template_id,
        deployment_id
    };
    env.events().publish(("contract_deployed", contract_address, deployer), event);
}

pub fn emit_fee_collected(env: &Env, deployer: Address, amount: i128) {
    let event = FeeCollectedEvent { deployer: deployer.clone(), amount };
    env.events().publish(("fee_collected", deployer), event);
}

pub fn emit_fee_config_updated(env: &Env, deployment_fee: i128, fee_collector: Address) {
    let event = FeeConfigUpdatedEvent { deployment_fee, fee_collector: fee_collector.clone() };
    env.events().publish(("fee_config_updated", fee_collector), event);
}

pub fn emit_factory_paused(env: &Env, admin: Address, is_paused: bool) {
    let event = FactoryPausedEvent { admin: admin.clone(), is_paused };
    env.events().publish(("factory_paused", admin), event);
}

pub fn emit_admin_changed(env: &Env, old_admin: Address, new_admin: Address) {
    let event = AdminChangedEvent { old_admin: old_admin.clone(), new_admin: new_admin.clone() };
    env.events().publish(("admin_changed", old_admin, new_admin), event);
}
