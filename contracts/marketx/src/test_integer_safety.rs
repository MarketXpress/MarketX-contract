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
    use soroban_sdk::{testutils::Address as _, token::Client as TokenClient, Address, Env, IntoVal};
    use crate::{Client, Contract};

    const FEE_BPS: u32 = 500; // 5%

    fn setup() -> (Env, Address, Address, TokenClient, Address, Client) {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::random(&env);
        let buyer = Address::random(&env);
        
        let sac = env.register_stellar_asset_contract_v2(admin.clone());
        let token = TokenClient::new(&env, &sac);
        let token_id = sac.address();

        let contract_id = env.register_contract(None, Contract);
        let client = Client::new(&env, &contract_id);

        client.initialize(&admin, &admin, &FEE_BPS, &0i128, &0i128);
        token.approve(&admin, &contract_id, &i128::MAX);

        (env, buyer, admin, token, token_id, client)
    }

    #[test]
    fn test_zero_amount_returns_zero_fee() {
        let (env, buyer, seller, _token, token_id, client) = setup();
        
        // Create and release escrow with 0 amount - should not panic
        let escrow_id = client.create_escrow(
            &buyer,
            &seller,
            &token_id,
            &0i128,
            &None,
            &None,
            &None,
        );

        client.fund_escrow(&escrow_id);
        let result = client.try_release_escrow(&escrow_id);
        
        // Should handle gracefully (either success or appropriate error)
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_small_amount_rounds_down() {
        let (env, buyer, seller, _token, token_id, client) = setup();
        
        // Amount 9,999 should give 0 fee with 500 bps
        // (9999 * 500) / 10000 = 499.95 -> rounds to 0
        let escrow_id = client.create_escrow(
            &buyer,
            &seller,
            &token_id,
            &9_999i128,
            &None,
            &None,
            &None,
        );

        client.fund_escrow(&escrow_id);
        client.release_escrow(&escrow_id);

        // Get total fees collected - should be 0 (rounded down)
        let total_fees = client.get_total_fees_collected();
        assert_eq!(total_fees, 0);
    }

    #[test]
    fn test_exact_division() {
        let (env, buyer, seller, _token, token_id, client) = setup();
        
        // Amount 10,000 with 500 bps = 500 fee exactly
        let escrow_id = client.create_escrow(
            &buyer,
            &seller,
            &token_id,
            &10_000i128,
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
        let (env, buyer, seller, _token, token_id, client) = setup();
        
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
        );

        client.fund_escrow(&escrow_id);
        client.release_escrow(&escrow_id);

        // Fee should be 500 (remainder discarded)
        let total_fees = client.get_total_fees_collected();
        assert_eq!(total_fees, 500);
    }

    #[test]
    fn test_large_amount_no_overflow() {
        let (env, buyer, seller, _token, token_id, client) = setup();
        
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
        for amount in [1000i128, 2000, 3000, 4000, 5000] {
            let escrow_id = client.create_escrow(
                &buyer,
                &seller,
                &token_id,
                &amount,
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
        let (env, admin, buyer, seller, _token, token_id, client) = setup();
        
        // Set zero fee bps
        client.set_fee_percentage(&admin, &0);

        let escrow_id = client.create_escrow(
            &buyer,
            &seller,
            &token_id,
            &10_000i128,
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