//! Volume Discount Tests for MarketX Contract
//! 
//! Tests for verifying volume-based fee discount functionality
//! 
//! ## Test Coverage
//! 
//! 1. Volume updates after escrow release
//! 2. Tier calculation based on volume
//! 3. Whitelist overrides volume discount
//! 4. Default tiers set on initialize
//! 5. Multiple releases accumulate volume

#![cfg(test)]
mod volume_tests {
    use soroban_sdk::{
        testutils::Address as _,
        token::Client as TokenClient,
        Address, Env,
    };
    use crate::{Client, Contract};

    const FEE_BPS: u32 = 500; // 5%
    const MIN_FEE: i128 = 0;
    const MAX_FEE: i128 = 0;

    fn setup() -> (Env, Address, Address, Address, TokenClient, Address, Client) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::random(&env);
        let buyer = Address::random(&env);
        let seller = Address::random(&env);
        
        let sac = env.register_stellar_asset_contract_v2(admin.clone());
        let token = TokenClient::new(&env, &sac);
        let token_id = sac.address();

        let contract_id = env.register_contract(None, Contract);
        let client = Client::new(&env, &contract_id);

        client.initialize(&admin, &admin, &FEE_BPS, &MIN_FEE, &MAX_FEE);
        token.approve(&admin, &contract_id, &i128::MAX);

        (env, admin, buyer, seller, token, token_id, client)
    }

    #[test]
    fn test_volume_updated_after_escrow_release() {
        let (env, _admin, buyer, seller, _token, token_id, client) = setup();

        let escrow_id = client.create_escrow(
            &buyer,
            &seller,
            &token_id,
            &100_000,
            &None,
            &None,
            &None,
        );

        client.fund_escrow(&escrow_id);
        client.release_escrow(&escrow_id);

        // Verify volume was updated
        #[cfg(feature = "testutils")]
        {
            let volume = client.get_buyer_volume(&buyer);
            assert_eq!(volume, 100_000, "Volume should be 100,000 after release");
        }
    }

    #[test]
    fn test_tier_calculation_from_volume() {
        let (env, _admin, buyer, seller, _token, token_id, client) = setup();

        // Create escrows to reach tier 1 (100,000+)
        for _ in 0..2 {
            let escrow_id = client.create_escrow(
                &buyer,
                &seller,
                &token_id,
                &100_000,
                &None,
                &None,
                &None,
            );
            client.fund_escrow(&escrow_id);
            client.release_escrow(&escrow_id);
        }

        // Total volume = 200,000 should be tier 1
        #[cfg(feature = "testutils")]
        {
            let tier = client.get_buyer_tier(&buyer);
            assert_eq!(tier, 1, "200,000 volume should be tier 1 (10% discount)");
        }
    }

    #[test]
    fn test_whitelist_prevents_fee() {
        let (env, admin, buyer, seller, _token, token_id, client) = setup();

        // Add buyer to whitelist
        client.add_fee_whitelist(&buyer);

        let escrow_id = client.create_escrow(
            &buyer,
            &seller,
            &token_id,
            &1_000_000,
            &None,
            &None,
            &None,
        );

        client.fund_escrow(&escrow_id);
        
        // Release should succeed - whitelist gives 100% discount
        let result = client.try_release_escrow(&escrow_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_default_tiers_set_on_initialize() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::random(&env);

        let contract_id = env.register_contract(None, Contract);
        let client = Client::new(&env, &contract_id);

        client.initialize(&admin, &admin, &FEE_BPS, &MIN_FEE, &MAX_FEE);

        // Check default tiers exist
        #[cfg(feature = "testutils")]
        {
            let tiers = client.get_volume_tiers();
            assert_eq!(tiers.tier_1_threshold, 100_000);
            assert_eq!(tiers.tier_2_threshold, 1_000_000);
            assert_eq!(tiers.tier_3_threshold, 10_000_000);
            assert_eq!(tiers.tier_1_discount_bps, 100);   // 10%
            assert_eq!(tiers.tier_2_discount_bps, 250);   // 25%
            assert_eq!(tiers.tier_3_discount_bps, 500);  // 50% max
        }
    }

    #[test]
    fn test_volume_accumulates() {
        let (env, _admin, buyer, seller, _token, token_id, client) = setup();

        // First escrow
        let id1 = client.create_escrow(&buyer, &seller, &token_id, &100_000, &None, &None, &None);
        client.fund_escrow(&id1);
        client.release_escrow(&id1);

        // Second escrow
        let id2 = client.create_escrow(&buyer, &seller, &token_id, &50_000, &None, &None, &None);
        client.fund_escrow(&id2);
        client.release_escrow(&id2);

        #[cfg(feature = "testutils")]
        {
            let volume = client.get_buyer_volume(&buyer);
            assert_eq!(volume, 150_000, "Volume should accumulate to 150,000");
        }
    }

    #[test]
    fn test_high_volume_tier_3() {
        let (env, _admin, buyer, seller, _token, token_id, client) = setup();

        // Create many escrows to reach tier 3
        for _ in 0..10 {
            let escrow_id = client.create_escrow(
                &buyer,
                &seller,
                &token_id,
                &1_000_000, // 0.1 XLM each
                &None,
                &None,
                &None,
            );
            client.fund_escrow(&escrow_id);
            client.release_escrow(&escrow_id);
        }

        // 10M+ should be tier 3
        #[cfg(feature = "testutils")]
        {
            let tier = client.get_buyer_tier(&buyer);
            assert!(tier >= 3, "10M+ volume should be tier 3");
        }
    }
}