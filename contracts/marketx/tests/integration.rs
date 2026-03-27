use marketx::{Contract, ContractClient};
use soroban_sdk::{Address, Env};

fn setup() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy contract
    let contract_id = env.register(Contract, ());

    (env, contract_id)
}
