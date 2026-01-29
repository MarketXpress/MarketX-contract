use soroban_sdk::{Env, Address, Symbol, Vec};
use crate::{storage::DataKey, errors::AccessError};

pub const ROLE_ADMIN: Symbol = symbol_short!("ADMIN");
pub const ROLE_SELLER: Symbol = symbol_short!("SELLER");
pub const ROLE_BUYER: Symbol = symbol_short!("BUYER");

pub fn assign_role(env: &Env, user: Address, role: Symbol) {
    let mut roles: Vec<Symbol> =
        env.storage().instance().get(&DataKey::Roles(user.clone()))
        .unwrap_or(Vec::new(env));

    if roles.contains(&role) {
        panic_with_error!(env, AccessError::AlreadyHasRole);
    }

    roles.push_back(role);
    env.storage().instance().set(&DataKey::Roles(user), &roles);
}

pub fn revoke_role(env: &Env, user: Address, role: Symbol) {
    let mut roles: Vec<Symbol> =
        env.storage().instance().get(&DataKey::Roles(user.clone()))
        .unwrap_or(Vec::new(env));

    roles.retain(|r| r != role);
    env.storage().instance().set(&DataKey::Roles(user), &roles);
}

pub fn has_role(env: &Env, user: Address, role: Symbol) -> bool {
    env.storage().instance()
        .get::<_, Vec<Symbol>>(&DataKey::Roles(user))
        .map(|r| r.contains(&role))
        .unwrap_or(false)
}
