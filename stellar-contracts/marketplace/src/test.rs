#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::types::*;
use crate::{MarketX, MarketXClient};

// ============================================================================
// TEST SETUP HELPERS
// ============================================================================

fn setup_env() -> (Env, Address) {
    let e = Env::default();
    let admin = Address::random(&e);
    e.mock_all_auths();
    (e, admin)
}

fn initialize_marketplace(e: &Env, admin: &Address) -> MarketXClient {
    let client = MarketXClient::new(e, &Address::random(e));
    client.initialize(admin, &250);
    client
}

fn create_seller(e: &Env, client: &MarketXClient) -> Address {
    let seller = Address::random(e);
    e.mock_all_auths();
    let metadata = String::from_small_copy(e, "Seller metadata");
    client.register_seller(&metadata);
    seller
}

fn verify_seller(e: &Env, client: &MarketXClient, admin: &Address, seller: &Address) {
    e.mock_all_auths();
    client.verify_seller(seller);
}

// ============================================================================
// INITIALIZATION TESTS
// ============================================================================

#[test]
fn test_initialize() {
    let (e, admin) = setup_env();
    let client = MarketXClient::new(&e, &Address::random(&e));

    let result = client.initialize(&admin, &250);
    assert_eq!(result, ());

    let config = client.get_config();
    assert_eq!(config.admin, admin);
    assert_eq!(config.base_fee_rate, 250);
    assert_eq!(config.is_paused, false);
}

#[test]
fn test_initialize_already_initialized() {
    let (e, admin) = setup_env();
    let client = MarketXClient::new(&e, &Address::random(&e));

    client.initialize(&admin, &250);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.initialize(&admin, &250);
    }));

    assert!(result.is_err());
}

#[test]
fn test_initialize_invalid_fee_rate() {
    let (e, admin) = setup_env();
    let client = MarketXClient::new(&e, &Address::random(&e));

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.initialize(&admin, &10001); // Fee rate > 100%
    }));

    assert!(result.is_err());
}

// ============================================================================
// MARKETPLACE CONFIGURATION TESTS
// ============================================================================

#[test]
fn test_set_fee_rate() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let result = client.set_fee_rate(&500);
    assert_eq!(result, ());

    let config = client.get_config();
    assert_eq!(config.base_fee_rate, 500);
}

#[test]
fn test_set_fee_rate_unauthorized() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let other_user = Address::random(&e);
    e.mock_all_auths();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let other_client = MarketXClient::new(&e, &other_user);
        other_client.set_fee_rate(&500);
    }));

    assert!(result.is_err());
}

#[test]
fn test_pause_marketplace() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let result = client.set_paused(&true);
    assert_eq!(result, ());

    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);
}

#[test]
fn test_unpause_marketplace() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    client.set_paused(&true);
    assert_eq!(client.is_paused(), true);

    client.set_paused(&false);
    assert_eq!(client.is_paused(), false);
}

// ============================================================================
// SELLER TESTS
// ============================================================================

#[test]
fn test_register_seller() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::random(&e);
    e.mock_all_auths();

    let metadata = String::from_small_copy(&e, "Test seller");
    let result = client.register_seller(&metadata);
    assert_eq!(result, ());

    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.status.as_u32(), SellerStatus::Unverified.as_u32());
    assert_eq!(seller_info.total_sales, 0);
}

#[test]
fn test_register_seller_already_exists() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::random(&e);
    e.mock_all_auths();

    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.register_seller(&metadata);
    }));

    assert!(result.is_err());
}

#[test]
fn test_register_seller_marketplace_paused() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    client.set_paused(&true);

    let seller = Address::random(&e);
    e.mock_all_auths();

    let metadata = String::from_small_copy(&e, "Test seller");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.register_seller(&metadata);
    }));

    assert!(result.is_err());
}

#[test]
fn test_verify_seller() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::random(&e);
    e.mock_all_auths();

    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);

    let result = client.verify_seller(&seller);
    assert_eq!(result, ());

    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.status.as_u32(), SellerStatus::Verified.as_u32());
}

#[test]
fn test_suspend_seller() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::random(&e);
    e.mock_all_auths();

    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);
    client.verify_seller(&seller);

    let result = client.suspend_seller(&seller);
    assert_eq!(result, ());

    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.status.as_u32(), SellerStatus::Suspended.as_u32());
}

#[test]
fn test_unsuspend_seller() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::random(&e);
    e.mock_all_auths();

    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);
    client.verify_seller(&seller);
    client.suspend_seller(&seller);

    let result = client.unsuspend_seller(&seller);
    assert_eq!(result, ());

    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.status.as_u32(), SellerStatus::Verified.as_u32());
}

#[test]
fn test_update_seller_rating() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::random(&e);
    e.mock_all_auths();

    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);

    let result = client.update_seller_rating(&seller, &400); // 4 stars
    assert_eq!(result, ());

    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.rating, 400);
}

#[test]
fn test_update_seller_rating_invalid() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::random(&e);
    e.mock_all_auths();

    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.update_seller_rating(&seller, &600); // > 5 stars
    }));

    assert!(result.is_err());
}

// ============================================================================
// CATEGORY TESTS
// ============================================================================

#[test]
fn test_create_category() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let name = String::from_small_copy(&e, "Electronics");
    let description = String::from_small_copy(&e, "Electronic products");

    let result = client.create_category(&1, &name, &description, &300);
    assert_eq!(result, ());

    let category = client.get_category(&1);
    assert_eq!(category.id, 1);
    assert_eq!(category.commission_rate, 300);
}

#[test]
fn test_create_category_duplicate() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let name = String::from_small_copy(&e, "Electronics");
    let description = String::from_small_copy(&e, "Electronic products");

    client.create_category(&1, &name, &description, &300);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.create_category(&1, &name, &description, &300);
    }));

    assert!(result.is_err());
}

#[test]
fn test_create_category_invalid_fee_rate() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let name = String::from_small_copy(&e, "Electronics");
    let description = String::from_small_copy(&e, "Electronic products");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.create_category(&1, &name, &description, &10001);
    }));

    assert!(result.is_err());
}

// ============================================================================
// PRODUCT TESTS
// ============================================================================

#[test]
fn test_add_product() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    // Setup category and seller
    let name = String::from_small_copy(&e, "Electronics");
    let description = String::from_small_copy(&e, "Electronic products");
    client.create_category(&1, &name, &description, &300);

    let seller = Address::random(&e);
    e.mock_all_auths();
    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);
    client.verify_seller(&seller);

    // Add product
    let product_name = String::from_small_copy(&e, "Laptop");
    let product_desc = String::from_small_copy(&e, "High performance laptop");
    let product_meta = String::from_small_copy(&e, "{}");

    let result = client.add_product(&product_name, &product_desc, &1, &100_000_000, &10, &product_meta);

    assert_eq!(result, 1);
}

#[test]
fn test_add_product_seller_not_verified() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    // Setup category
    let name = String::from_small_copy(&e, "Electronics");
    let description = String::from_small_copy(&e, "Electronic products");
    client.create_category(&1, &name, &description, &300);

    // Register but don't verify
    let seller = Address::random(&e);
    e.mock_all_auths();
    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);

    // Try to add product
    let product_name = String::from_small_copy(&e, "Laptop");
    let product_desc = String::from_small_copy(&e, "High performance laptop");
    let product_meta = String::from_small_copy(&e, "{}");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.add_product(&product_name, &product_desc, &1, &100_000_000, &10, &product_meta);
    }));

    assert!(result.is_err());
}

#[test]
fn test_add_product_invalid_category() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    // Register and verify seller
    let seller = Address::random(&e);
    e.mock_all_auths();
    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);
    client.verify_seller(&seller);

    // Try to add product with non-existent category
    let product_name = String::from_small_copy(&e, "Laptop");
    let product_desc = String::from_small_copy(&e, "High performance laptop");
    let product_meta = String::from_small_copy(&e, "{}");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.add_product(&product_name, &product_desc, &999, &100_000_000, &10, &product_meta);
    }));

    assert!(result.is_err());
}

#[test]
fn test_get_product() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    // Setup
    let name = String::from_small_copy(&e, "Electronics");
    let description = String::from_small_copy(&e, "Electronic products");
    client.create_category(&1, &name, &description, &300);

    let seller = Address::random(&e);
    e.mock_all_auths();
    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);
    client.verify_seller(&seller);

    let product_name = String::from_small_copy(&e, "Laptop");
    let product_desc = String::from_small_copy(&e, "High performance laptop");
    let product_meta = String::from_small_copy(&e, "{}");
    let product_id = client.add_product(&product_name, &product_desc, &1, &100_000_000, &10, &product_meta);

    let product = client.get_product(&product_id);
    assert_eq!(product.id, product_id);
    assert_eq!(product.price, 100_000_000);
    assert_eq!(product.stock_quantity, 10);
}

#[test]
fn test_update_product() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    // Setup
    let name = String::from_small_copy(&e, "Electronics");
    let description = String::from_small_copy(&e, "Electronic products");
    client.create_category(&1, &name, &description, &300);

    let seller = Address::random(&e);
    e.mock_all_auths();
    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);
    client.verify_seller(&seller);

    let product_name = String::from_small_copy(&e, "Laptop");
    let product_desc = String::from_small_copy(&e, "High performance laptop");
    let product_meta = String::from_small_copy(&e, "{}");
    let product_id = client.add_product(&product_name, &product_desc, &1, &100_000_000, &10, &product_meta);

    // Update product
    let result = client.update_product(&product_id, &150_000_000, &5, &0);
    assert_eq!(result, ());

    let product = client.get_product(&product_id);
    assert_eq!(product.price, 150_000_000);
    assert_eq!(product.stock_quantity, 5);
}

#[test]
fn test_delist_product() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    // Setup
    let name = String::from_small_copy(&e, "Electronics");
    let description = String::from_small_copy(&e, "Electronic products");
    client.create_category(&1, &name, &description, &300);

    let seller = Address::random(&e);
    e.mock_all_auths();
    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);
    client.verify_seller(&seller);

    let product_name = String::from_small_copy(&e, "Laptop");
    let product_desc = String::from_small_copy(&e, "High performance laptop");
    let product_meta = String::from_small_copy(&e, "{}");
    let product_id = client.add_product(&product_name, &product_desc, &1, &100_000_000, &10, &product_meta);

    // Delist product
    let result = client.delist_product(&product_id);
    assert_eq!(result, ());

    let product = client.get_product(&product_id);
    assert_eq!(product.status.as_u32(), ProductStatus::Delisted.as_u32());
}

// ============================================================================
// FEE CALCULATION TESTS
// ============================================================================

#[test]
fn test_calculate_fee_base_rate() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let fee = client.calculate_fee(&1000_000, &None);
    // 1000000 * 250 / 10000 = 25000
    assert_eq!(fee, 25000);
}

#[test]
fn test_calculate_fee_category_rate() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let name = String::from_small_copy(&e, "Electronics");
    let description = String::from_small_copy(&e, "Electronic products");
    client.create_category(&1, &name, &description, &300);

    let fee = client.calculate_fee(&1000_000, &Some(1));
    // 1000000 * 300 / 10000 = 30000
    assert_eq!(fee, 30000);
}

#[test]
fn test_calculate_fee_zero_amount() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let fee = client.calculate_fee(&0, &None);
    assert_eq!(fee, 0);
}

#[test]
fn test_record_fee_collection() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let result = client.record_fee_collection(&1_000_000);
    assert_eq!(result, ());

    let total_fees = client.get_total_fees();
    assert_eq!(total_fees, 1_000_000);
}

#[test]
fn test_record_fee_collection_unauthorized() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let other_user = Address::random(&e);
    e.mock_all_auths();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let other_client = MarketXClient::new(&e, &other_user);
        other_client.record_fee_collection(&1_000_000);
    }));

    assert!(result.is_err());
}

// ============================================================================
// SEARCH & FILTERING TESTS
// ============================================================================

#[test]
fn test_get_products_by_seller() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    // Setup
    let cat_name = String::from_small_copy(&e, "Electronics");
    let cat_desc = String::from_small_copy(&e, "Electronic products");
    client.create_category(&1, &cat_name, &cat_desc, &300);

    let seller = Address::random(&e);
    e.mock_all_auths();
    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);
    client.verify_seller(&seller);

    // Add products
    let product_name = String::from_small_copy(&e, "Laptop");
    let product_desc = String::from_small_copy(&e, "High performance laptop");
    let product_meta = String::from_small_copy(&e, "{}");

    let id1 = client.add_product(&product_name, &product_desc, &1, &100_000_000, &10, &product_meta);
    let id2 = client.add_product(&product_name, &product_desc, &1, &150_000_000, &5, &product_meta);

    let products = client.get_products_by_seller(&seller);
    assert_eq!(products.len(), 2);
}

#[test]
fn test_get_products_by_category() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    // Setup categories
    let cat_name = String::from_small_copy(&e, "Electronics");
    let cat_desc = String::from_small_copy(&e, "Electronic products");
    client.create_category(&1, &cat_name, &cat_desc, &300);

    let cat_name2 = String::from_small_copy(&e, "Books");
    let cat_desc2 = String::from_small_copy(&e, "Books");
    client.create_category(&2, &cat_name2, &cat_desc2, &200);

    let seller = Address::random(&e);
    e.mock_all_auths();
    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);
    client.verify_seller(&seller);

    // Add products to different categories
    let product_name = String::from_small_copy(&e, "Product");
    let product_desc = String::from_small_copy(&e, "Description");
    let product_meta = String::from_small_copy(&e, "{}");

    client.add_product(&product_name, &product_desc, &1, &100_000_000, &10, &product_meta);
    client.add_product(&product_name, &product_desc, &2, &100_000_000, &10, &product_meta);

    let category_1_products = client.get_products_by_category(&1);
    let category_2_products = client.get_products_by_category(&2);

    assert_eq!(category_1_products.len(), 1);
    assert_eq!(category_2_products.len(), 1);
}

// ============================================================================
// STATISTICS TESTS
// ============================================================================

#[test]
fn test_get_stats() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let stats = client.get_stats();
    assert_eq!(stats.0, 0); // total_products
    assert_eq!(stats.1, 0); // total_sellers
    assert_eq!(stats.2, 0); // total_fees

    // Add seller
    let seller = Address::random(&e);
    e.mock_all_auths();
    let metadata = String::from_small_copy(&e, "Test seller");
    client.register_seller(&metadata);

    let stats = client.get_stats();
    assert_eq!(stats.1, 1); // total_sellers should be 1
}

// ============================================================================
// INTEGRATION TESTS - COMPLETE WORKFLOWS
// ============================================================================

/// Test scenario: Complete marketplace workflow
/// 1. Initialize marketplace
/// 2. Create categories
/// 3. Register and verify sellers
/// 4. List products
/// 5. Calculate fees
/// 6. Manage marketplace
#[test]
fn test_complete_marketplace_workflow() {
    let e = Env::default();
    let admin = Address::random(&e);
    let seller1 = Address::random(&e);
    let seller2 = Address::random(&e);

    e.mock_all_auths();

    let contract_id = Address::random(&e);
    let client = MarketXClient::new(&e, &contract_id);

    // 1. Initialize marketplace
    client.initialize(&admin, &250); // 2.5% base fee

    let config = client.get_config();
    assert_eq!(config.admin, admin);
    assert_eq!(config.base_fee_rate, 250);
    assert_eq!(config.total_products, 0);
    assert_eq!(config.total_sellers, 0);

    // 2. Create categories
    let electronics_name = String::from_small_copy(&e, "Electronics");
    let electronics_desc = String::from_small_copy(&e, "Electronic devices and accessories");
    client.create_category(&1, &electronics_name, &electronics_desc, &300); // 3% commission

    let books_name = String::from_small_copy(&e, "Books");
    let books_desc = String::from_small_copy(&e, "Physical and digital books");
    client.create_category(&2, &books_name, &books_desc, &200); // 2% commission

    // 3. Register sellers
    let seller1_metadata = String::from_small_copy(&e, r#"{"name":"TechStore","reputation":4.8}"#);
    let seller2_metadata = String::from_small_copy(&e, r#"{"name":"BookNook","reputation":4.5}"#);

    e.mock_all_auths();
    client.register_seller(&seller1_metadata);

    e.mock_all_auths();
    client.register_seller(&seller2_metadata);

    // Verify sellers
    client.verify_seller(&seller1);
    client.verify_seller(&seller2);

    // Check seller info
    let seller1_info = client.get_seller(&seller1);
    assert_eq!(seller1_info.status.as_u32(), SellerStatus::Verified.as_u32());

    // 4. List products
    let laptop_name = String::from_small_copy(&e, "Premium Laptop");
    let laptop_desc = String::from_small_copy(&e, "High performance laptop with 16GB RAM");
    let laptop_meta = String::from_small_copy(&e, r#"{"cpu":"i7","ram":"16GB","storage":"512SSD"}"#);

    e.mock_all_auths();
    let product1_id = client.add_product(&laptop_name, &laptop_desc, &1, &99_999_999, &5, &laptop_meta);
    assert_eq!(product1_id, 1);

    let book_name = String::from_small_copy(&e, "Rust Programming");
    let book_desc = String::from_small_copy(&e, "Learn Rust programming language");
    let book_meta = String::from_small_copy(&e, r#"{"pages":"500","author":"John Doe"}"#);

    e.mock_all_auths();
    let product2_id = client.add_product(&book_name, &book_desc, &2, &49_999_999, &20, &book_meta);
    assert_eq!(product2_id, 2);

    // 5. Calculate fees
    let laptop_fee = client.calculate_fee(&99_999_999, &Some(1)); // Electronics category
    assert_eq!(laptop_fee, 29_999_999); // 99999999 * 300 / 10000

    let book_fee = client.calculate_fee(&49_999_999, &Some(2)); // Books category
    assert_eq!(book_fee, 9_999_999); // 49999999 * 200 / 10000

    // 6. Get marketplace stats
    let stats = client.get_stats();
    assert_eq!(stats.0, 2); // 2 products
    assert_eq!(stats.1, 2); // 2 sellers
    assert_eq!(stats.2, 0); // No fees collected yet

    // Test search functionality
    let electronics_products = client.get_products_by_category(&1);
    assert_eq!(electronics_products.len(), 1);

    let seller1_products = client.get_products_by_seller(&seller1);
    assert_eq!(seller1_products.len(), 1);
}

/// Test scenario: Seller management and verification flow
#[test]
fn test_seller_lifecycle() {
    let e = Env::default();
    let admin = Address::random(&e);
    let seller = Address::random(&e);

    e.mock_all_auths();

    let client = MarketXClient::new(&e, &Address::random(&e));
    client.initialize(&admin, &250);

    // 1. Register seller (unverified)
    let metadata = String::from_small_copy(&e, r#"{"name":"NewSeller","location":"USA"}"#);
    client.register_seller(&metadata);

    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.status.as_u32(), SellerStatus::Unverified.as_u32());
    assert_eq!(seller_info.total_sales, 0);
    assert_eq!(seller_info.rating, 0);

    // 2. Verify seller
    client.verify_seller(&seller);
    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.status.as_u32(), SellerStatus::Verified.as_u32());

    // 3. Update seller rating
    client.update_seller_rating(&seller, &450); // 4.5 stars
    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.rating, 450);

    // 4. Suspend seller
    client.suspend_seller(&seller);
    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.status.as_u32(), SellerStatus::Suspended.as_u32());

    // 5. Unsuspend seller
    client.unsuspend_seller(&seller);
    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.status.as_u32(), SellerStatus::Verified.as_u32());
}

/// Test scenario: Product lifecycle management
#[test]
fn test_product_lifecycle() {
    let e = Env::default();
    let admin = Address::random(&e);
    let seller = Address::random(&e);

    e.mock_all_auths();

    let client = MarketXClient::new(&e, &Address::random(&e));
    client.initialize(&admin, &250);

    // Setup
    let cat_name = String::from_small_copy(&e, "Electronics");
    let cat_desc = String::from_small_copy(&e, "Electronic devices");
    client.create_category(&1, &cat_name, &cat_desc, &300);

    let seller_metadata = String::from_small_copy(&e, "Seller");
    client.register_seller(&seller_metadata);
    client.verify_seller(&seller);

    // 1. Create product
    let name = String::from_small_copy(&e, "Smartphone");
    let desc = String::from_small_copy(&e, "Latest smartphone model");
    let meta = String::from_small_copy(&e, r#"{"model":"X1","storage":"128GB"}"#);

    e.mock_all_auths();
    let product_id = client.add_product(&name, &desc, &1, &799_999_999, &50, &meta);
    assert_eq!(product_id, 1);

    let product = client.get_product(&product_id);
    assert_eq!(product.price, 799_999_999);
    assert_eq!(product.stock_quantity, 50);
    assert_eq!(product.status.as_u32(), ProductStatus::Active.as_u32());

    // 2. Update product (price and stock)
    e.mock_all_auths();
    client.update_product(&product_id, &749_999_999, &40, &0);

    let product = client.get_product(&product_id);
    assert_eq!(product.price, 749_999_999);
    assert_eq!(product.stock_quantity, 40);

    // 3. Rate product
    e.mock_all_auths();
    client.update_product_rating(&product_id, &480); // 4.8 stars

    let product = client.get_product(&product_id);
    assert_eq!(product.rating, 480);

    // 4. Delist product
    e.mock_all_auths();
    client.delist_product(&product_id);

    let product = client.get_product(&product_id);
    assert_eq!(product.status.as_u32(), ProductStatus::Delisted.as_u32());
}

/// Test scenario: Fee management and calculation
#[test]
fn test_fee_management() {
    let e = Env::default();
    let admin = Address::random(&e);

    e.mock_all_auths();

    let client = MarketXClient::new(&e, &Address::random(&e));
    client.initialize(&admin, &250); // 2.5% base fee

    // Create categories with different fees
    let cat1_name = String::from_small_copy(&e, "Premium");
    let cat1_desc = String::from_small_copy(&e, "Premium products");
    client.create_category(&1, &cat1_name, &cat1_desc, &500); // 5% commission

    let cat2_name = String::from_small_copy(&e, "Economy");
    let cat2_desc = String::from_small_copy(&e, "Economy products");
    client.create_category(&2, &cat2_name, &cat2_desc, &100); // 1% commission

    // Test fee calculations
    let amount = 1_000_000_000;

    // Base fee (no category)
    let base_fee = client.calculate_fee(&amount, &None);
    assert_eq!(base_fee, 25_000_000); // 1B * 250 / 10000

    // Premium category fee
    let premium_fee = client.calculate_fee(&amount, &Some(1));
    assert_eq!(premium_fee, 50_000_000); // 1B * 500 / 10000

    // Economy category fee
    let economy_fee = client.calculate_fee(&amount, &Some(2));
    assert_eq!(economy_fee, 10_000_000); // 1B * 100 / 10000

    // Record fee collection
    client.record_fee_collection(&base_fee);
    client.record_fee_collection(&premium_fee);

    let total_fees = client.get_total_fees();
    assert_eq!(total_fees, 75_000_000); // 25M + 50M

    // Update base fee rate
    client.set_fee_rate(&350); // 3.5% new base fee

    let new_base_fee = client.calculate_fee(&amount, &None);
    assert_eq!(new_base_fee, 35_000_000); // 1B * 350 / 10000
}

/// Test scenario: Marketplace emergency controls
#[test]
fn test_marketplace_emergency_controls() {
    let e = Env::default();
    let admin = Address::random(&e);
    let seller = Address::random(&e);

    e.mock_all_auths();

    let client = MarketXClient::new(&e, &Address::random(&e));
    client.initialize(&admin, &250);

    // Setup
    let cat_name = String::from_small_copy(&e, "General");
    let cat_desc = String::from_small_copy(&e, "General products");
    client.create_category(&1, &cat_name, &cat_desc, &250);

    let seller_meta = String::from_small_copy(&e, "Seller");
    client.register_seller(&seller_meta);
    client.verify_seller(&seller);

    let product_name = String::from_small_copy(&e, "Product");
    let product_desc = String::from_small_copy(&e, "A product");
    let product_meta = String::from_small_copy(&e, "{}");

    e.mock_all_auths();
    client.add_product(&product_name, &product_desc, &1, &100_000, &10, &product_meta);

    // Marketplace is active
    assert_eq!(client.is_paused(), false);

    // Pause marketplace
    client.set_paused(&true);
    assert_eq!(client.is_paused(), true);

    // Try to register seller while paused
    let new_seller_meta = String::from_small_copy(&e, "New seller");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        e.mock_all_auths();
        client.register_seller(&new_seller_meta);
    }));
    assert!(result.is_err()); // Should fail

    // Unpause marketplace
    client.set_paused(&false);
    assert_eq!(client.is_paused(), false);
}

/// Test scenario: Product search and filtering
#[test]
fn test_product_search_and_filtering() {
    let e = Env::default();
    let admin = Address::random(&e);
    let seller1 = Address::random(&e);
    let seller2 = Address::random(&e);

    e.mock_all_auths();

    let client = MarketXClient::new(&e, &Address::random(&e));
    client.initialize(&admin, &250);

    // Create categories
    let cat1_name = String::from_small_copy(&e, "Electronics");
    let cat1_desc = String::from_small_copy(&e, "Electronics");
    client.create_category(&1, &cat1_name, &cat1_desc, &300);

    let cat2_name = String::from_small_copy(&e, "Clothing");
    let cat2_desc = String::from_small_copy(&e, "Clothing");
    client.create_category(&2, &cat2_name, &cat2_desc, &250);

    // Register sellers
    let seller1_meta = String::from_small_copy(&e, "Seller 1");
    let seller2_meta = String::from_small_copy(&e, "Seller 2");

    e.mock_all_auths();
    client.register_seller(&seller1_meta);

    e.mock_all_auths();
    client.register_seller(&seller2_meta);

    client.verify_seller(&seller1);
    client.verify_seller(&seller2);

    // Add products
    let name1 = String::from_small_copy(&e, "Laptop");
    let desc1 = String::from_small_copy(&e, "Laptop");
    let meta1 = String::from_small_copy(&e, "{}");

    e.mock_all_auths();
    client.add_product(&name1, &desc1, &1, &999_999_999, &5, &meta1);

    let name2 = String::from_small_copy(&e, "T-Shirt");
    let desc2 = String::from_small_copy(&e, "T-Shirt");
    let meta2 = String::from_small_copy(&e, "{}");

    e.mock_all_auths();
    client.add_product(&name2, &desc2, &2, &29_999_999, &100, &meta2);

    let name3 = String::from_small_copy(&e, "Jeans");
    let desc3 = String::from_small_copy(&e, "Jeans");
    let meta3 = String::from_small_copy(&e, "{}");

    e.mock_all_auths();
    client.add_product(&name3, &desc3, &2, &59_999_999, &50, &meta3);

    // Search by category
    let electronics = client.get_products_by_category(&1);
    assert_eq!(electronics.len(), 1);

    let clothing = client.get_products_by_category(&2);
    assert_eq!(clothing.len(), 2);

    // Search by seller
    let seller1_products = client.get_products_by_seller(&seller1);
    assert_eq!(seller1_products.len(), 1);
}

/// Test scenario: Marketplace configuration updates
#[test]
fn test_marketplace_configuration() {
    let e = Env::default();
    let admin = Address::random(&e);

    e.mock_all_auths();

    let client = MarketXClient::new(&e, &Address::random(&e));
    client.initialize(&admin, &250);

    let config = client.get_config();
    assert_eq!(config.base_fee_rate, 250);

    // Update fee rate
    client.set_fee_rate(&500); // 5%

    let config = client.get_config();
    assert_eq!(config.base_fee_rate, 500);

    // Update category fee rate
    let cat_name = String::from_small_copy(&e, "Premium");
    let cat_desc = String::from_small_copy(&e, "Premium products");
    client.create_category(&1, &cat_name, &cat_desc, &300);

    client.set_category_fee_rate(&1, &600); // 6% for premium

    let fee_with_category = client.calculate_fee(&1_000_000_000, &Some(1));
    assert_eq!(fee_with_category, 60_000_000); // 1B * 600 / 10000
}
