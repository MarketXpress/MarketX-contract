use soroban_sdk::Env;
use crate::{storage::DataKey, errors::AccessError};

pub fn assert_not_paused(env: &Env) {
    let paused: bool = env.storage().instance().get(&DataKey::Paused).unwrap_or(false);
    if paused {
        panic_with_error!(env, AccessError::ContractPaused);
    }
}

pub fn set_pause(env: &Env, value: bool) {
    env.storage().instance().set(&DataKey::Paused, &value);
}
