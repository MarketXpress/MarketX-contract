#![no_std]

use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env, IntoVal, String, Symbol, Vec, vec};

mod errors;
mod events;
mod storage;
mod types;

use errors::Error;
use types::{ContractTemplate, EscrowInitParams, FeeConfig};

#[contract]
pub struct EscrowFactory;

#[contractimpl]
impl EscrowFactory {
    // ========== INITIALIZATION ==========

    /// Initialize the factory with admin and registry address
    pub fn initialize(
        env: Env,
        admin: Address,
        registry: Address,
        deployment_fee: i128,
        fee_token: Address,
        fee_collector: Address,
    ) -> Result<(), Error> {
        if storage::is_initialized(&env) {
            return Err(Error::AlreadyInitialized);
        }

        admin.require_auth();

        storage::set_initialized(&env);
        storage::set_admin(&env, &admin);
        storage::set_registry(&env, &registry);
        storage::set_paused(&env, false);

        let fee_config = FeeConfig {
            deployment_fee,
            fee_token,
            fee_collector: fee_collector.clone(),
        };
        storage::set_fee_config(&env, &fee_config);

        events::emit_factory_initialized(&env, admin, registry);

        Ok(())
    }

    // ========== TEMPLATE MANAGEMENT ==========

    /// Register a new contract template (admin only)
    pub fn register_template(
        env: Env,
        admin: Address,
        wasm_hash: BytesN<32>,
        name: String,
        version: u32,
    ) -> Result<u32, Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        // Check if template with same hash already exists
        if storage::get_template_id_by_hash(&env, &wasm_hash).is_some() {
            return Err(Error::TemplateAlreadyExists);
        }

        if version == 0 {
            return Err(Error::InvalidVersion);
        }

        let template_id = storage::increment_template_counter(&env);

        let template = ContractTemplate {
            id: template_id,
            wasm_hash: wasm_hash.clone(),
            name: name.clone(),
            version,
            created_at: env.ledger().timestamp(),
            is_active: true,
            deployment_count: 0,
        };

        storage::set_template(&env, &template);

        events::emit_template_registered(&env, template_id, wasm_hash, name, version);

        Ok(template_id)
    }

    /// Deactivate a template (admin only)
    pub fn deactivate_template(env: Env, admin: Address, template_id: u32) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        let mut template = storage::get_template(&env, template_id)
            .ok_or(Error::TemplateNotFound)?;

        template.is_active = false;
        storage::set_template(&env, &template);

        events::emit_template_deactivated(&env, template_id, admin);

        Ok(())
    }

    /// Reactivate a template (admin only)
    pub fn activate_template(env: Env, admin: Address, template_id: u32) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        let mut template = storage::get_template(&env, template_id)
            .ok_or(Error::TemplateNotFound)?;

        template.is_active = true;
        storage::set_template(&env, &template);

        Ok(())
    }

    /// Get template by ID
    pub fn get_template(env: Env, template_id: u32) -> Result<ContractTemplate, Error> {
        storage::get_template(&env, template_id).ok_or(Error::TemplateNotFound)
    }

    /// Get all templates (active and inactive)
    pub fn list_templates(env: Env) -> Vec<ContractTemplate> {
        storage::list_templates(&env)
    }

    // ========== CONTRACT DEPLOYMENT ==========

    /// Deploy a new escrow instance
    pub fn deploy_escrow(
        env: Env,
        deployer: Address,
        template_id: u32,
        salt: BytesN<32>,
        init_params: EscrowInitParams,
    ) -> Result<Address, Error> {
        Self::require_initialized(&env)?;
        Self::require_not_paused(&env)?;

        deployer.require_auth();

        // Get and validate template
        let mut template = storage::get_template(&env, template_id)
            .ok_or(Error::TemplateNotFound)?;

        if !template.is_active {
            return Err(Error::TemplateInactive);
        }

        // Collect deployment fee
        let fee_config = storage::get_fee_config(&env).ok_or(Error::NotInitialized)?;
        if fee_config.deployment_fee > 0 {
            let token_client = token::Client::new(&env, &fee_config.fee_token);
            token_client.transfer(&deployer, &fee_config.fee_collector, &fee_config.deployment_fee);
            events::emit_fee_collected(&env, deployer.clone(), fee_config.deployment_fee);
        }

        // Deploy the contract using the template's WASM hash
        let deployed_address = env
            .deployer()
            .with_current_contract(salt)
            .deploy(template.wasm_hash.clone());

        // Initialize the deployed escrow contract by calling its init function
        Self::call_escrow_init(
            &env,
            &deployed_address,
            &init_params.admin,
            init_params.fee_bps,
            &init_params.fee_collector,
            &init_params.emergency_admins,
            init_params.emergency_threshold,
        );

        // Update template deployment count
        template.deployment_count += 1;
        storage::set_template(&env, &template);

        // Increment deployment counter
        let deployment_id = storage::increment_deployment_counter(&env);

        // Register in registry
        let registry = storage::get_registry(&env);
        Self::call_registry_register(
            &env,
            &registry,
            &deployed_address,
            &deployer,
            template_id,
            &template.wasm_hash,
            template.version,
        );

        events::emit_contract_deployed(&env, deployed_address.clone(), deployer, template_id, deployment_id);

        Ok(deployed_address)
    }

    // ========== FEE MANAGEMENT ==========

    /// Update deployment fee (admin only)
    pub fn set_deployment_fee(env: Env, admin: Address, fee: i128) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        let mut config = storage::get_fee_config(&env).ok_or(Error::NotInitialized)?;
        config.deployment_fee = fee;
        storage::set_fee_config(&env, &config);

        events::emit_fee_config_updated(&env, fee, config.fee_collector);

        Ok(())
    }

    /// Update fee collector (admin only)
    pub fn set_fee_collector(env: Env, admin: Address, collector: Address) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        let mut config = storage::get_fee_config(&env).ok_or(Error::NotInitialized)?;
        config.fee_collector = collector.clone();
        storage::set_fee_config(&env, &config);

        events::emit_fee_config_updated(&env, config.deployment_fee, collector);

        Ok(())
    }

    /// Update fee token (admin only)
    pub fn set_fee_token(env: Env, admin: Address, token_addr: Address) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        let mut config = storage::get_fee_config(&env).ok_or(Error::NotInitialized)?;
        config.fee_token = token_addr;
        storage::set_fee_config(&env, &config);

        Ok(())
    }

    /// Get fee configuration
    pub fn get_fee_config(env: Env) -> Result<FeeConfig, Error> {
        storage::get_fee_config(&env).ok_or(Error::NotInitialized)
    }

    // ========== ACCESS CONTROL ==========

    /// Transfer admin role
    pub fn set_admin(env: Env, current_admin: Address, new_admin: Address) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &current_admin)?;

        storage::set_admin(&env, &new_admin);

        events::emit_admin_changed(&env, current_admin, new_admin);

        Ok(())
    }

    /// Pause/unpause factory (admin only)
    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), Error> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &admin)?;

        storage::set_paused(&env, paused);

        events::emit_factory_paused(&env, admin, paused);

        Ok(())
    }

    /// Check if factory is paused
    pub fn is_paused(env: Env) -> bool {
        storage::is_paused(&env)
    }

    // ========== VIEWS ==========

    /// Get admin address
    pub fn get_admin(env: Env) -> Result<Address, Error> {
        if !storage::is_initialized(&env) {
            return Err(Error::NotInitialized);
        }
        Ok(storage::get_admin(&env))
    }

    /// Get registry address
    pub fn get_registry(env: Env) -> Result<Address, Error> {
        if !storage::is_initialized(&env) {
            return Err(Error::NotInitialized);
        }
        Ok(storage::get_registry(&env))
    }

    /// Get total deployments count
    pub fn get_deployment_count(env: Env) -> u64 {
        storage::get_deployment_counter(&env)
    }

    /// Get total templates count
    pub fn get_template_count(env: Env) -> u32 {
        storage::get_template_counter(&env)
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

    fn require_not_paused(env: &Env) -> Result<(), Error> {
        if storage::is_paused(env) {
            return Err(Error::FactoryPaused);
        }
        Ok(())
    }

    /// Call the init function on the deployed escrow contract
    fn call_escrow_init(
        env: &Env,
        escrow_address: &Address,
        admin: &Address,
        fee_bps: u32,
        fee_collector: &Address,
        emergency_admins: &Vec<Address>,
        emergency_threshold: u32,
    ) {
        let init_fn = Symbol::new(env, "init");
        let args: Vec<soroban_sdk::Val> = vec![
            env,
            admin.into_val(env),
            fee_bps.into_val(env),
            fee_collector.into_val(env),
            emergency_admins.into_val(env),
            emergency_threshold.into_val(env),
        ];
        env.invoke_contract::<()>(escrow_address, &init_fn, args);
    }

    /// Call the register_contract function on the registry
    fn call_registry_register(
        env: &Env,
        registry_address: &Address,
        contract_address: &Address,
        deployer: &Address,
        template_id: u32,
        wasm_hash: &BytesN<32>,
        version: u32,
    ) {
        let register_fn = Symbol::new(env, "register_contract");
        let caller = env.current_contract_address();
        let args: Vec<soroban_sdk::Val> = vec![
            env,
            caller.into_val(env),
            contract_address.into_val(env),
            deployer.into_val(env),
            template_id.into_val(env),
            wasm_hash.into_val(env),
            version.into_val(env),
        ];
        let _: u64 = env.invoke_contract(registry_address, &register_fn, args);
    }
}

#[cfg(test)]
mod test;
