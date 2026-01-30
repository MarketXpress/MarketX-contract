use soroban_sdk::{contracttype, Address, BytesN, String, Vec};

/// Storage keys for registry contract
#[contracttype]
#[derive(Clone)]
pub enum StorageKey {
    Admin,
    Factory,
    Initialized,
    ContractCounter,
    Contract(Address),
    ContractById(u64),
    ContractsByDeployer(Address),
    ContractsByTemplate(u32),
    ContractsByVersion(u32),
    ContractPermissions(Address),
    Stats,
}

/// Contract status enum
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractStatus {
    Active = 0,
    Paused = 1,
    Archived = 2,
    Deleted = 3,
}

/// Permission types for contracts
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Permission {
    CanCallFactory = 0,
    CanCallRegistry = 1,
    CanCallMarketplace = 2,
    CanCollectFees = 3,
}

/// Deployed contract information
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DeployedContract {
    pub registry_id: u64,
    pub contract_address: Address,
    pub deployer: Address,
    pub template_id: u32,
    pub wasm_hash: BytesN<32>,
    pub version: u32,
    pub status: ContractStatus,
    pub deployed_at: u64,
    pub updated_at: u64,
    pub archive_reason: Option<String>,
}

/// Registry configuration
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RegistryConfig {
    pub admin: Address,
    pub factory: Address,
    pub updated_at: u64,
}

/// Registry statistics
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RegistryStats {
    pub total_contracts: u64,
    pub active_contracts: u64,
    pub paused_contracts: u64,
    pub archived_contracts: u64,
    pub deleted_contracts: u64,
}
