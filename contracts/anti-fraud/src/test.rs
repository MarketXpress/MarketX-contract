#![cfg(test)]

use super::*;
use crate::types::{FraudError, UserStatus};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env,
};

#[test]
fn test_initialization() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AntiFraudContract);
    let client = AntiFraudContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);

    client.initialize(&admin);

    let config = client.get_risk_config();
    assert_eq!(config.max_tx_amount, 10_000_000_000);
}

#[test]
fn test_double_initialization_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AntiFraudContract);
    let client = AntiFraudContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);

    client.initialize(&admin);
    let res = client.try_initialize(&admin);
    assert_eq!(res, Err(Ok(FraudError::AlreadyInitialized)));
}

#[test]
fn test_blacklist_enforcement() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AntiFraudContract);
    let client = AntiFraudContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    client.initialize(&admin);

    // Should work initially
    assert!(client.try_check_transaction(&user, &100).is_ok());

    // Blacklist user
    client.set_user_status(&admin, &user, &UserStatus::Blacklisted);

    // Should fail now
    let res = client.try_check_transaction(&user, &100);
    assert_eq!(res, Err(Ok(FraudError::UserBlacklisted)));
}

#[test]
fn test_limit_enforcement() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AntiFraudContract);
    let client = AntiFraudContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    client.initialize(&admin);

    // Set low limit for testing
    client.set_risk_params(&admin, &1000, &5000, &10);

    // Under limit
    assert!(client.try_check_transaction(&user, &900).is_ok());

    // Over limit
    let res = client.try_check_transaction(&user, &1001);
    assert_eq!(res, Err(Ok(FraudError::LimitExceeded)));
}

#[test]
fn test_velocity_limits() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AntiFraudContract);
    let client = AntiFraudContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    client.initialize(&admin);
    // Max 100 per tx, Max 200 per day, Max 3 txs per day
    client.set_risk_params(&admin, &100, &200, &3);

    // 1st Tx: OK
    client.report_activity(&user, &50);

    // 2nd Tx: OK
    client.report_activity(&user, &50);

    // 3rd Tx: OK
    client.report_activity(&user, &50); // Total 150

    // Check tx that would exceed volume (Current 150 + 60 = 210 > 200)
    let res = client.try_check_transaction(&user, &60);
    assert_eq!(res, Err(Ok(FraudError::LimitExceeded))); // Volume limit

    // Check tx count limit (Already 3 txs)
    let res_count = client.try_check_transaction(&user, &10);
    assert_eq!(res_count, Err(Ok(FraudError::LimitExceeded))); // Count limit
}

#[test]
fn test_kyc_status() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AntiFraudContract);
    let client = AntiFraudContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let user = Address::generate(&e);

    client.initialize(&admin);

    assert_eq!(client.get_user_status(&user), UserStatus::Unverified);

    client.set_kyc_status(&admin, &user, &true);

    assert_eq!(client.get_user_status(&user), UserStatus::Verified);
}
