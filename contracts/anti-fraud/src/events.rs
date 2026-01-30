use crate::types::RiskConfig;
use soroban_sdk::{contracttype, Address, Env};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionBlockedEvent {
    pub user: Address,
    pub amount: i128,
    pub reason: soroban_sdk::Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserFlaggedEvent {
    pub user: Address,
    pub reason: soroban_sdk::Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskConfigUpdatedEvent {
    pub admin: Address,
    pub old_config: RiskConfig,
    pub new_config: RiskConfig,
}

pub fn publish_transaction_blocked(
    e: &Env,
    user: Address,
    amount: i128,
    reason: soroban_sdk::Symbol,
) {
    let event = TransactionBlockedEvent {
        user,
        amount,
        reason,
    };
    e.events().publish(("fraud", "blocked"), event);
}

pub fn publish_user_flagged(e: &Env, user: Address, reason: soroban_sdk::Symbol) {
    let event = UserFlaggedEvent { user, reason };
    e.events().publish(("fraud", "flagged"), event);
}

pub fn publish_risk_config_updated(
    e: &Env,
    admin: Address,
    old_config: RiskConfig,
    new_config: RiskConfig,
) {
    let event = RiskConfigUpdatedEvent {
        admin,
        old_config,
        new_config,
    };
    e.events().publish(("admin", "config_update"), event);
}
