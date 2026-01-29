#![cfg(test)]
use super::*;
use soroban_sdk::{Env, Address};

#[test]
fn admin_can_assign_role() {
    let env = Env::default();
    let admin = Address::random(&env);
    let user = Address::random(&env);

    env.mock_all_auths();

    roles::assign_role(&env, admin.clone(), roles::ROLE_ADMIN);
    AccessControl::assign_role(env.clone(), admin, user.clone(), roles::ROLE_BUYER);

    assert!(roles::has_role(&env, user, roles::ROLE_BUYER));
}
