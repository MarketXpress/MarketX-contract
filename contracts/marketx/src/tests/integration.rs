use soroban_sdk::{Env, Address, testutils::Address as _};
use marketx_contract::ProjectContract;
use soroban_token_sdk::TokenClient;

fn setup() -> (Env, Address, TokenClient, Address) {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy mock token
    let token_admin = Address::random(&env);
    let token_client = TokenClient::new(&env, &token_admin);

    // Deploy contract
    let contract_id = env.register_contract(None, ProjectContract);
    let contract_client = ProjectContract::new(&env, &contract_id);

    (env, contract_id, token_client, token_admin)
}
