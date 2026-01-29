use soroban_sdk::{Env, Address, Symbol};

pub fn role_assigned(env: &Env, user: Address, role: Symbol) {
    env.events().publish(
        (symbol_short!("ROLE_ASSIGNED"),),
        (user, role)
    );
}

pub fn role_revoked(env: &Env, user: Address, role: Symbol) {
    env.events().publish(
        (symbol_short!("ROLE_REVOKED"),),
        (user, role)
    );
}

pub fn paused(env: &Env, value: bool) {
    env.events().publish(
        (symbol_short!("PAUSED"),),
        value
    );
}
