use soroban_sdk::{contracterror, contracttype};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FraudError {
    NotInitialized = 0,
    AlreadyInitialized = 1,
    Unauthorized = 2,
    InvalidInput = 3,
    UserBlacklisted = 4,
    LimitExceeded = 5,
    SuspiciousActivity = 6,
    UserNotFound = 7,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum UserStatus {
    Unverified = 0,
    Verified = 1,
    Suspicious = 2,
    Blacklisted = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskConfig {
    pub max_tx_amount: i128,
    pub daily_volume_limit: i128,
    pub max_daily_tx_count: u32,
    pub updated_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserStats {
    pub last_tx_timestamp: u64,
    pub daily_volume: i128,
    pub daily_tx_count: u32,
    pub total_tx_count: u64,
}
