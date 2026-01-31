use crate::types::{RiskConfig, UserStats, UserStatus};
use soroban_sdk::{contracttype, Address, Env};

#[contracttype]
pub enum StorageKey {
    Admin,
    RiskConfig,
    UserStatus(Address),
    UserStats(Address),
    KycStatus(Address),
}

const DAY_IN_SECONDS: u64 = 86400; // 24 hours

pub fn set_admin(e: &Env, admin: &Address) {
    e.storage().instance().set(&StorageKey::Admin, admin);
}

pub fn get_admin(e: &Env) -> Option<Address> {
    e.storage().instance().get(&StorageKey::Admin)
}

pub fn set_risk_config(e: &Env, config: &RiskConfig) {
    e.storage().instance().set(&StorageKey::RiskConfig, config);
}

pub fn get_risk_config(e: &Env) -> Option<RiskConfig> {
    e.storage().instance().get(&StorageKey::RiskConfig)
}

pub fn set_user_status(e: &Env, user: &Address, status: UserStatus) {
    e.storage()
        .persistent()
        .set(&StorageKey::UserStatus(user.clone()), &status);
}

pub fn get_user_status(e: &Env, user: &Address) -> UserStatus {
    e.storage()
        .persistent()
        .get(&StorageKey::UserStatus(user.clone()))
        .unwrap_or(UserStatus::Unverified)
}

pub fn set_user_stats(e: &Env, user: &Address, stats: &UserStats) {
    e.storage()
        .persistent()
        .set(&StorageKey::UserStats(user.clone()), stats);
}

pub fn get_user_stats(e: &Env, user: &Address) -> UserStats {
    e.storage()
        .persistent()
        .get(&StorageKey::UserStats(user.clone()))
        .unwrap_or(UserStats {
            last_tx_timestamp: 0,
            daily_volume: 0,
            daily_tx_count: 0,
            total_tx_count: 0,
        })
}

pub fn set_kyc_status(e: &Env, user: &Address, status: bool) {
    e.storage()
        .persistent()
        .set(&StorageKey::KycStatus(user.clone()), &status);
}

pub fn get_kyc_status(e: &Env, user: &Address) -> bool {
    e.storage()
        .persistent()
        .get(&StorageKey::KycStatus(user.clone()))
        .unwrap_or(false)
}

pub fn is_initialized(e: &Env) -> bool {
    e.storage().instance().has(&StorageKey::Admin)
}

pub fn extend_instance_ttl(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(30 * DAY_IN_SECONDS as u32, 60 * DAY_IN_SECONDS as u32);
}
