#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Address, Symbol};

mod roles;
mod storage;
mod errors;
mod pause;
mod multisig;
mod events;

use roles::*;
use pause::*;

#[contract]
pub struct AccessControl;

#[contractimpl]
impl AccessControl {

    pub fn assign_role(env: Env, admin: Address, user: Address, role: Symbol) {
        assert_not_paused(&env);
        admin.require_auth();

        if !has_role(&env, admin.clone(), ROLE_ADMIN) {
            panic_with_error!(&env, errors::AccessError::Unauthorized);
        }

        assign_role(&env, user.clone(), role.clone());
        events::role_assigned(&env, user, role);
    }

    pub fn revoke_role(env: Env, admin: Address, user: Address, role: Symbol) {
        assert_not_paused(&env);
        admin.require_auth();

        revoke_role(&env, user.clone(), role.clone());
        events::role_revoked(&env, user, role);
    }

    pub fn pause(env: Env, admin: Address, proposal_id: u64) {
        admin.require_auth();
        multisig::assert_approved(&env, proposal_id);
        set_pause(&env, true);
        events::paused(&env, true);
    }
}
