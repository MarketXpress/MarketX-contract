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
    use crate::{Contract, ContractClient};
    use soroban_sdk::{
        testutils::Address as _,
        token::{Client as TokenClient, StellarAssetClient},
        Address, Env,
    };

    const FEE_BPS: u32 = 500; // 5%
    const MIN_FEE: i128 = 0;
    const MAX_FEE: i128 = 0;

    fn setup<'a>() -> (
        Env,
        Address,
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
        let seller = Address::generate(&env);

        let sac = env.register_stellar_asset_contract_v2(admin.clone());
        let token = TokenClient::new(&env, &sac.address());
        let token_admin = StellarAssetClient::new(&env, &sac.address());
        let token_id = sac.address();

        let contract_id = env.register(Contract, ());
        let client = ContractClient::new(&env, &contract_id);

        client.initialize(&admin, &admin, &FEE_BPS, &MIN_FEE, &MAX_FEE);
        token_admin.mint(&buyer, &20_000_000);
        token.approve(&buyer, &contract_id, &i128::MAX, &1000);

        (env, admin, buyer, seller, token, token_id, client)
    }

    #[test]
    fn test_volume_updated_after_escrow_release() {
        let (_env, _admin, buyer, seller, _token, token_id, client) = setup();

        let escrow_id = client.create_escrow(
            &buyer, &seller, &token_id, &100_000, &None, &None, &None, &None,
        );

        client.fund_escrow(&escrow_id);
        client.release_escrow(&escrow_id);

        // Verify volume was updated
        let volume = client.get_buyer_volume(&buyer);
        assert_eq!(volume, 100_000);
    }

    #[test]
    fn test_tier_calculation_from_volume() {
        let (env, _admin, buyer, seller, _token, token_id, client) = setup();

        // Create escrows to reach tier 1 (100,000+)
        for index in 0..2 {
            let escrow_id = client.create_escrow(
                &buyer,
                &seller,
                &token_id,
                &100_000,
                &Some(soroban_sdk::Bytes::from_slice(&env, &[index as u8])),
                &None,
                &None,
                &None,
            );
            client.fund_escrow(&escrow_id);
            client.release_escrow(&escrow_id);
        }

        // Total volume = 200,000 should be tier 1
        let tier = client.get_buyer_tier(&buyer);
        assert_eq!(tier, 1);
    }

    #[test]
    fn test_whitelist_prevents_fee() {
        let (_env, _admin, buyer, seller, _token, token_id, client) = setup();

        // Add buyer to whitelist
        client.add_fee_whitelist(&buyer);

        let escrow_id = client.create_escrow(
            &buyer, &seller, &token_id, &1_000_000, &None, &None, &None, &None,
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

        let admin = Address::generate(&env);

        let contract_id = env.register(Contract, ());
        let client = ContractClient::new(&env, &contract_id);

        client.initialize(&admin, &admin, &FEE_BPS, &MIN_FEE, &MAX_FEE);

        // Check default tiers exist
        let tiers = client.get_volume_tiers();
        assert_eq!(tiers.tier_1_threshold, 100_000);
        assert_eq!(tiers.tier_2_threshold, 1_000_000);
        assert_eq!(tiers.tier_3_threshold, 10_000_000);
        assert_eq!(tiers.tier_1_discount_bps, 100);
        assert_eq!(tiers.tier_2_discount_bps, 250);
        assert_eq!(tiers.tier_3_discount_bps, 500);
    }

    #[test]
    fn test_volume_accumulates() {
        let (env, _admin, buyer, seller, _token, token_id, client) = setup();

        // First escrow
        let id1 = client.create_escrow(
            &buyer, &seller, &token_id, &100_000, &None, &None, &None, &None,
        );
        client.fund_escrow(&id1);
        client.release_escrow(&id1);

        // Second escrow
        let id2 = client.create_escrow(
            &buyer,
            &seller,
            &token_id,
            &50_000,
            &Some(soroban_sdk::Bytes::from_slice(&env, &[2u8])),
            &None,
            &None,
            &None,
        );
        client.fund_escrow(&id2);
        client.release_escrow(&id2);

        let volume = client.get_buyer_volume(&buyer);
        assert_eq!(volume, 150_000);
    }

    #[test]
    fn test_high_volume_tier_3() {
        let (env, _admin, buyer, seller, _token, token_id, client) = setup();
        // Create many escrows to reach tier 3
        for index in 0..10 {
            let escrow_id = client.create_escrow(
                &buyer,
                &seller,
                &token_id,
                &1_000_000, // 0.1 XLM each
                &Some(soroban_sdk::Bytes::from_slice(&env, &[index as u8])),
                &None,
                &None,
                &None,
            );
            client.fund_escrow(&escrow_id);
            client.release_escrow(&escrow_id);
        }

        // 10M+ should be tier 3
        let tier = client.get_buyer_tier(&buyer);
        assert!(tier >= 3);
    }
}
