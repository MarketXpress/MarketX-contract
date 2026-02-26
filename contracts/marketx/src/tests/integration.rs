use soroban_sdk::{Env, Address, testutils::Address as _};
use crate::Contract;

fn setup() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy contract
    let contract_id = env.register_contract(None, Contract);

    (env, contract_id)
}
