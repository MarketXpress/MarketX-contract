use soroban_sdk::{contracttype, Address, BytesN, String, Vec};

/// Storage keys for factory contract
#[contracttype]
#[derive(Clone)]
pub enum StorageKey {
    Admin,
    Registry,
    Initialized,
    Paused,
    TemplateCounter,
    Template(u32),
    TemplateByHash(BytesN<32>),
    DeploymentCounter,
    FeeConfig,
}

/// Contract template information
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ContractTemplate {
    pub id: u32,
    pub wasm_hash: BytesN<32>,
    pub name: String,
    pub version: u32,
    pub created_at: u64,
    pub is_active: bool,
    pub deployment_count: u64,
}

/// Factory configuration
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct FactoryConfig {
    pub admin: Address,
    pub registry: Address,
    pub total_templates: u32,
    pub total_deployments: u64,
    pub is_paused: bool,
    pub updated_at: u64,
}

/// Fee configuration
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct FeeConfig {
    pub deployment_fee: i128,
    pub fee_token: Address,
    pub fee_collector: Address,
}

/// Parameters for escrow initialization (passed to deployed contract)
#[contracttype]
#[derive(Clone, Debug)]
pub struct EscrowInitParams {
    pub admin: Address,
    pub fee_bps: u32,
    pub fee_collector: Address,
    pub emergency_admins: Vec<Address>,
    pub emergency_threshold: u32,
}
