use soroban_sdk::{contracttype, Address, BytesN, Env, String};
use crate::types::{ContractStatus, Permission};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryInitializedEvent {
    pub admin: Address,
    pub factory: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractRegisteredEvent {
    pub contract_address: Address,
    pub deployer: Address,
    pub registry_id: u64,
    pub template_id: u32,
    pub version: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractStatusUpdatedEvent {
    pub contract_address: Address,
    pub old_status: ContractStatus,
    pub new_status: ContractStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractArchivedEvent {
    pub contract_address: Address,
    pub reason: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractRestoredEvent {
    pub contract_address: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractDeletedEvent {
    pub contract_address: Address,
    pub admin: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionLinkedEvent {
    pub contract_address: Address,
    pub permission: Permission,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionRevokedEvent {
    pub contract_address: Address,
    pub permission: Permission,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminChangedEvent {
    pub old_admin: Address,
    pub new_admin: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactoryChangedEvent {
    pub old_factory: Address,
    pub new_factory: Address,
}

pub fn emit_registry_initialized(env: &Env, admin: Address, factory: Address) {
    let event = RegistryInitializedEvent { admin: admin.clone(), factory };
    env.events().publish(("registry_initialized", admin), event);
}

pub fn emit_contract_registered(env: &Env, contract_address: Address, deployer: Address, registry_id: u64, template_id: u32, version: u32) {
    let event = ContractRegisteredEvent {
        contract_address: contract_address.clone(),
        deployer: deployer.clone(),
        registry_id,
        template_id,
        version
    };
    env.events().publish(("contract_registered", contract_address, deployer), event);
}

pub fn emit_contract_status_updated(env: &Env, contract_address: Address, old_status: ContractStatus, new_status: ContractStatus) {
    let event = ContractStatusUpdatedEvent {
        contract_address: contract_address.clone(),
        old_status,
        new_status
    };
    env.events().publish(("contract_status_updated", contract_address), event);
}

pub fn emit_contract_archived(env: &Env, contract_address: Address, reason: String) {
    let event = ContractArchivedEvent { contract_address: contract_address.clone(), reason };
    env.events().publish(("contract_archived", contract_address), event);
}

pub fn emit_contract_restored(env: &Env, contract_address: Address) {
    let event = ContractRestoredEvent { contract_address: contract_address.clone() };
    env.events().publish(("contract_restored", contract_address), event);
}

pub fn emit_contract_deleted(env: &Env, contract_address: Address, admin: Address) {
    let event = ContractDeletedEvent { contract_address: contract_address.clone(), admin: admin.clone() };
    env.events().publish(("contract_deleted", contract_address, admin), event);
}

pub fn emit_permission_linked(env: &Env, contract_address: Address, permission: Permission) {
    let event = PermissionLinkedEvent { contract_address: contract_address.clone(), permission };
    env.events().publish(("permission_linked", contract_address), event);
}

pub fn emit_permission_revoked(env: &Env, contract_address: Address, permission: Permission) {
    let event = PermissionRevokedEvent { contract_address: contract_address.clone(), permission };
    env.events().publish(("permission_revoked", contract_address), event);
}

pub fn emit_admin_changed(env: &Env, old_admin: Address, new_admin: Address) {
    let event = AdminChangedEvent { old_admin: old_admin.clone(), new_admin: new_admin.clone() };
    env.events().publish(("admin_changed", old_admin, new_admin), event);
}

pub fn emit_factory_changed(env: &Env, old_factory: Address, new_factory: Address) {
    let event = FactoryChangedEvent { old_factory: old_factory.clone(), new_factory: new_factory.clone() };
    env.events().publish(("factory_changed", old_factory, new_factory), event);
}
