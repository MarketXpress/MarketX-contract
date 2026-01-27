use crate::storage;
use soroban_sdk::{Address, Env};

pub fn require_admin(env: &Env, admin: &Address) {
    let stored_admin = storage::get_admin(env);
    stored_admin.require_auth();
    if stored_admin != *admin {
        panic!("unauthorized");
    }
}
