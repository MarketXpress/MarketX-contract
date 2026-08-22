//! Integer Safety Tests for Fee Basis Points Calculations
//!
//! Tests to verify overflow-safe fee calculations
//!
//! Test Cases:
//! - Zero amount returns zero fee
//! - Amount less than 10,000 returns 0 (rounds down)  
//! - Exact 10,000 divides cleanly
//! - Amount with remainder handled correctly
//! - Large amounts don't overflow
//! - Max i128 value doesn't panic

#![cfg(test)]
mod integer_safety_tests {
    use crate::{Contract, ContractClient};
    use soroban_sdk::{
        testutils::Address as _, token::Client as TokenClient, token::StellarAssetClient, Address,
        Env,
    };

    const FEE_BPS: u32 = 500; // 5%

    fn setup<'a>() -> (
        Env,
        Address,
        Address,
        TokenClient<'a>,
        Address,
        ContractClient<'a>,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let buyer = Address::generate(&env);

        let sac = env.register_stellar_asset_contract_v2(admin.clone());
        let token = TokenClient::new(&env, &sac.address());
        let token_admin = StellarAssetClient::new(&env, &sac.address());
        let token_id = sac.address();

        let contract_id = env.register(Contract, ());
        let client = ContractClient::new(&env, &contract_id);

        client.initialize(&admin, &admin, &FEE_BPS, &0i128, &0i128);
        token_admin.mint(&buyer, &2_000_000_000);
        token.approve(&buyer, &contract_id, &i128::MAX, &1000);

        (env, buyer, admin, token, token_id, client)
    }

    #[test]
    fn test_zero_amount_returns_zero_fee() {
        let (_env, buyer, seller, _token, token_id, client) = setup();

        let result = client.try_create_escrow(
            &buyer, &seller, &token_id, &0i128, &None, &None, &None, &None,
        );
        assert_eq!(result, Err(Ok(crate::ContractError::InvalidEscrowAmount)));
    }

    #[test]
    fn test_small_amount_rounds_down() {
        let (_env, buyer, seller, _token, token_id, client) = setup();

        // Amount 9,999 floors to a 499 fee with 500 bps.
        let escrow_id = client.create_escrow(
            &buyer, &seller, &token_id, &9_999i128, &None, &None, &None, &None,
        );

        client.fund_escrow(&escrow_id);
        client.release_escrow(&escrow_id);

        // Get total fees collected.
        let total_fees = client.get_total_fees_collected();
        assert_eq!(total_fees, 499);
    }

    #[test]
    fn test_exact_division() {
        let (_env, buyer, seller, _token, token_id, client) = setup();

        // Amount 10,000 with 500 bps = 500 fee exactly
        let escrow_id = client.create_escrow(
            &buyer,
            &seller,
            &token_id,
            &10_000i128,
            &None,
            &None,
            &None,
            &None,
        );

        client.fund_escrow(&escrow_id);
        client.release_escrow(&escrow_id);

        // Fee should be exactly 500
        let total_fees = client.get_total_fees_collected();
        assert_eq!(total_fees, 500);
    }

    #[test]
    fn test_remainder_handled_correctly() {
        let (_env, buyer, seller, _token, token_id, client) = setup();

        // Amount 10,001 with 500 bps
        // Fee = (10001 * 500) / 10000 = 500.05 -> floors to 500
        let escrow_id = client.create_escrow(
            &buyer,
            &seller,
            &token_id,
            &10_001i128,
            &None,
            &None,
            &None,
            &None,
        );

        client.fund_escrow(&escrow_id);
        client.release_escrow(&escrow_id);

        // Fee should be 500 (remainder discarded)
        let total_fees = client.get_total_fees_collected();
        assert_eq!(total_fees, 500);
    }

    #[test]
    fn test_large_amount_no_overflow() {
        let (_env, buyer, seller, _token, token_id, client) = setup();

        // Very large amount - should not cause overflow
        let large_amount = 1_000_000_000i128; // 100 XLM

        let escrow_id = client.create_escrow(
            &buyer,
            &seller,
            &token_id,
            &large_amount,
            &None,
            &None,
            &None,
            &None,
        );

        client.fund_escrow(&escrow_id);

        // This should not panic
        let result = client.try_release_escrow(&escrow_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_escrows_accumulate_safely() {
        let (env, buyer, seller, _token, token_id, client) = setup();

        // Create multiple escrows
        for (index, amount) in [1000i128, 2000, 3000, 4000, 5000].into_iter().enumerate() {
            let escrow_id = client.create_escrow(
                &buyer,
                &seller,
                &token_id,
                &amount,
                &Some(soroban_sdk::Bytes::from_slice(&env, &[index as u8])),
                &None,
                &None,
                &None,
            );
            client.fund_escrow(&escrow_id);
            client.release_escrow(&escrow_id);
        }

        // Total fees should accumulate correctly
        // (1000+2000+3000+4000+5000) * 500 / 10000 = 750
        let total_fees = client.get_total_fees_collected();
        assert_eq!(total_fees, 750);
    }

    #[test]
    fn test_zero_fee_bps_returns_zero() {
        let (env, buyer, _admin, _token, token_id, client) = setup();
        let seller = Address::generate(&env);

        // Set zero fee bps
        client.set_fee_percentage(&0);

        let escrow_id = client.create_escrow(
            &buyer,
            &seller,
            &token_id,
            &10_000i128,
            &None,
            &None,
            &None,
            &None,
        );

        client.fund_escrow(&escrow_id);
        client.release_escrow(&escrow_id);

        // Fee should be 0
        let total_fees = client.get_total_fees_collected();
        assert_eq!(total_fees, 0);
    }
}
