#![no_std]

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String, Vec};

mod errors;
mod events;
mod storage;
mod types;

use errors::Error;
use types::{ContractStatus, DeployedContract, Permission, RegistryStats};

#[contract]
pub struct ContractRegistry;

#[contractimpl]
impl ContractRegistry {
    // ========== INITIALIZATION ==========

    /// Initialize the registry with admin and factory address
    pub fn initialize(env: Env, admin: Address, factory: Address) -> Result<(), Error> {
        if storage::is_initialized(&env) {
            return Err(Error::AlreadyInitialized);
        }

        admin.require_auth();

        storage::set_initialized(&env);
        storage::set_admin(&env, &admin);
        storage::set_factory(&env, &factory);

        events::emit_registry_initialized(&env, admin, factory);

        Ok(())
    }

    // ========== CONTRACT REGISTRATION (Factory Only) ==========

    /// Register a deployed contract (called by factory)
    pub fn register_contract(
        env: Env,
        caller: Address,
        contract_address: Address,
        deployer: Address,
        template_id: u32,
        wasm_hash: BytesN<32>,
        version: u32,
    ) -> Result<u64, Error> {
        Self::require_initialized(&env)?;
        Self::require_factory(&env, &caller)?;

        // Check if contract is already registered
        if storage::get_contract(&env, &contract_address).is_some() {
            return Err(Error::ContractAlreadyRegistered);
        }

        let registry_id = storage::increment_contract_counter(&env);

        let deployed_contract = DeployedContract {
            registry_id,
            contract_address: contract_address.clone(),
            deployer: deployer.clone(),
            template_id,
            wasm_hash,
            version,
            status: ContractStatus::Active,
            deployed_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
            archive_reason: None,
        };

        // Store contract
        storage::set_contract(&env, &deployed_contract);

        // Add to indexes
        storage::add_contract_to_deployer(&env, &deployer, &contract_address);
        storage::add_contract_to_template(&env, template_id, &contract_address);
        storage::add_contract_to_version(&env, version, &contract_address);

        // Update stats
        storage::update_stats_on_register(&env);

        events::emit_contract_registered(&env, contract_address, deployer, registry_id, template_id, version);

        Ok(registry_id)
    }

    // ========== CONTRACT MANAGEMENT ==========

    /// Update contract status
    pub fn update_status(
        env: Env,
        admin: Address,
        contract_address: Address,
        status: ContractStatus,
    ) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        let mut contract = storage::get_contract(&env, &contract_address)
            .ok_or(Error::ContractNotFound)?;

        if contract.status == ContractStatus::Deleted {
            return Err(Error::ContractDeleted);
        }

        let old_status = contract.status;
        contract.status = status;
        contract.updated_at = env.ledger().timestamp();

        storage::set_contract(&env, &contract);
        storage::update_stats_on_status_change(&env, old_status, status);

        events::emit_contract_status_updated(&env, contract_address, old_status, status);

        Ok(())
    }

    /// Archive a contract (soft delete)
    pub fn archive_contract(
        env: Env,
        admin: Address,
        contract_address: Address,
        reason: String,
    ) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        let mut contract = storage::get_contract(&env, &contract_address)
            .ok_or(Error::ContractNotFound)?;

        if contract.status == ContractStatus::Deleted {
            return Err(Error::ContractDeleted);
        }

        let old_status = contract.status;
        contract.status = ContractStatus::Archived;
        contract.archive_reason = Some(reason.clone());
        contract.updated_at = env.ledger().timestamp();

        storage::set_contract(&env, &contract);
        storage::update_stats_on_status_change(&env, old_status, ContractStatus::Archived);

        events::emit_contract_archived(&env, contract_address, reason);

        Ok(())
    }

    /// Restore an archived contract
    pub fn restore_contract(env: Env, admin: Address, contract_address: Address) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        let mut contract = storage::get_contract(&env, &contract_address)
            .ok_or(Error::ContractNotFound)?;

        if contract.status != ContractStatus::Archived {
            return Err(Error::InvalidStatus);
        }

        let old_status = contract.status;
        contract.status = ContractStatus::Active;
        contract.archive_reason = None;
        contract.updated_at = env.ledger().timestamp();

        storage::set_contract(&env, &contract);
        storage::update_stats_on_status_change(&env, old_status, ContractStatus::Active);

        events::emit_contract_restored(&env, contract_address);

        Ok(())
    }

    /// Delete contract from registry (hard delete - admin only)
    pub fn delete_contract(env: Env, admin: Address, contract_address: Address) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        let contract = storage::get_contract(&env, &contract_address)
            .ok_or(Error::ContractNotFound)?;

        if contract.status == ContractStatus::Deleted {
            return Err(Error::ContractDeleted);
        }

        let old_status = contract.status;

        // Remove from all indexes
        storage::remove_contract_from_deployer(&env, &contract.deployer, &contract_address);
        storage::remove_contract_from_template(&env, contract.template_id, &contract_address);
        storage::remove_contract_from_version(&env, contract.version, &contract_address);

        // Mark as deleted (don't actually remove to keep history)
        let mut updated_contract = contract;
        updated_contract.status = ContractStatus::Deleted;
        updated_contract.updated_at = env.ledger().timestamp();
        storage::set_contract(&env, &updated_contract);

        storage::update_stats_on_status_change(&env, old_status, ContractStatus::Deleted);

        events::emit_contract_deleted(&env, contract_address, admin);

        Ok(())
    }

    // ========== QUERIES ==========

    /// Get contract info by address
    pub fn get_contract(env: Env, contract_address: Address) -> Result<DeployedContract, Error> {
        storage::get_contract(&env, &contract_address).ok_or(Error::ContractNotFound)
    }

    /// Get contract by registry ID
    pub fn get_contract_by_id(env: Env, registry_id: u64) -> Result<DeployedContract, Error> {
        storage::get_contract_by_id(&env, registry_id).ok_or(Error::ContractNotFound)
    }

    /// List all contracts by deployer
    pub fn get_contracts_by_deployer(env: Env, deployer: Address) -> Vec<Address> {
        storage::get_contracts_by_deployer(&env, &deployer)
    }

    /// List all contracts by template
    pub fn get_contracts_by_template(env: Env, template_id: u32) -> Vec<Address> {
        storage::get_contracts_by_template(&env, template_id)
    }

    /// List all contracts by version
    pub fn get_contracts_by_version(env: Env, version: u32) -> Vec<Address> {
        storage::get_contracts_by_version(&env, version)
    }

    /// Check if address is a registered contract
    pub fn is_registered(env: Env, contract_address: Address) -> bool {
        storage::get_contract(&env, &contract_address).is_some()
    }

    /// Validate contract address (check if registered and active)
    pub fn validate_contract(env: Env, contract_address: Address) -> bool {
        if let Some(contract) = storage::get_contract(&env, &contract_address) {
            return contract.status == ContractStatus::Active;
        }
        false
    }

    // ========== PERMISSION LINKING ==========

    /// Link permission from factory to deployed contract
    pub fn link_permission(
        env: Env,
        admin: Address,
        contract_address: Address,
        permission: Permission,
    ) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        // Verify contract exists
        let contract = storage::get_contract(&env, &contract_address)
            .ok_or(Error::ContractNotFound)?;

        if contract.status == ContractStatus::Deleted {
            return Err(Error::ContractDeleted);
        }

        storage::add_permission(&env, &contract_address, permission);

        events::emit_permission_linked(&env, contract_address, permission);

        Ok(())
    }

    /// Revoke permission link
    pub fn revoke_permission(
        env: Env,
        admin: Address,
        contract_address: Address,
        permission: Permission,
    ) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        storage::remove_permission(&env, &contract_address, permission);

        events::emit_permission_revoked(&env, contract_address, permission);

        Ok(())
    }

    /// Check if contract has permission
    pub fn has_permission(env: Env, contract_address: Address, permission: Permission) -> bool {
        storage::has_permission(&env, &contract_address, permission)
    }

    /// Get all permissions for a contract
    pub fn get_permissions(env: Env, contract_address: Address) -> Vec<Permission> {
        storage::get_permissions(&env, &contract_address)
    }

    // ========== ACCESS CONTROL ==========

    /// Set admin
    pub fn set_admin(env: Env, current_admin: Address, new_admin: Address) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &current_admin)?;

        storage::set_admin(&env, &new_admin);

        events::emit_admin_changed(&env, current_admin, new_admin);

        Ok(())
    }

    /// Set authorized factory
    pub fn set_factory(env: Env, admin: Address, factory: Address) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        let old_factory = storage::get_factory(&env);
        storage::set_factory(&env, &factory);

        events::emit_factory_changed(&env, old_factory, factory);

        Ok(())
    }

    /// Get admin address
    pub fn get_admin(env: Env) -> Result<Address, Error> {
        if !storage::is_initialized(&env) {
            return Err(Error::NotInitialized);
        }
        Ok(storage::get_admin(&env))
    }

    /// Get factory address
    pub fn get_factory(env: Env) -> Result<Address, Error> {
        if !storage::is_initialized(&env) {
            return Err(Error::NotInitialized);
        }
        Ok(storage::get_factory(&env))
    }

    // ========== STATISTICS ==========

    /// Get registry statistics
    pub fn get_stats(env: Env) -> RegistryStats {
        storage::get_stats(&env)
    }

    /// Get total contracts count
    pub fn get_total_contracts(env: Env) -> u64 {
        storage::get_contract_counter(&env)
    }

    // ========== INTERNAL HELPERS ==========

    fn require_initialized(env: &Env) -> Result<(), Error> {
        if !storage::is_initialized(env) {
            return Err(Error::NotInitialized);
        }
        Ok(())
    }

    fn require_admin(env: &Env, admin: &Address) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin = storage::get_admin(env);
        if *admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }

    fn require_factory(env: &Env, caller: &Address) -> Result<(), Error> {
        let factory = storage::get_factory(env);
        if *caller != factory {
            return Err(Error::CallerNotFactory);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test;
