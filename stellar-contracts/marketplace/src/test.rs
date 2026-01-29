#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::oracle::OracleService;
use crate::types::*;
use crate::{MarketX, MarketXClient};

fn setup_env() -> (Env, Address) {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    (e, admin)
}

fn initialize_marketplace<'a>(e: &'a Env, admin: &Address) -> MarketXClient<'a> {
    let contract_id = e.register(MarketX, ());
    let client = MarketXClient::new(e, &contract_id);
    client.initialize(admin, &250);
    client
}

#[test]
fn test_initialize() {
    let (e, admin) = setup_env();
    let contract_id = e.register(MarketX, ());
    let client = MarketXClient::new(&e, &contract_id);

    client.initialize(&admin, &250);

    let config = client.get_config();
    assert_eq!(config.admin, admin);
    assert_eq!(config.base_fee_rate, 250);
    assert_eq!(config.is_paused, false);
}

#[test]
#[should_panic]
fn test_initialize_already_initialized() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);
    client.initialize(&admin, &250);
}

#[test]
#[should_panic]
fn test_initialize_invalid_fee_rate() {
    let (e, admin) = setup_env();
    let contract_id = e.register(MarketX, ());
    let client = MarketXClient::new(&e, &contract_id);
    client.initialize(&admin, &10001);
}

#[test]
fn test_set_fee_rate() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    client.set_fee_rate(&admin, &500);

    let config = client.get_config();
    assert_eq!(config.base_fee_rate, 500);
}

#[test]
fn test_pause_marketplace() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    client.set_paused(&admin, &true);
    assert_eq!(client.is_paused(), true);
}

#[test]
fn test_unpause_marketplace() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    client.set_paused(&admin, &true);
    assert_eq!(client.is_paused(), true);

    client.set_paused(&admin, &false);
    assert_eq!(client.is_paused(), false);
}

#[test]
fn test_register_seller() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);

    let seller_info = client.get_seller(&seller);
    assert_eq!(
        seller_info.status.as_u32(),
        SellerStatus::Unverified.as_u32()
    );
    assert_eq!(seller_info.total_sales, 0);
}

#[test]
#[should_panic]
fn test_register_seller_already_exists() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);
    client.register_seller(&seller, &metadata);
}

#[test]
#[should_panic]
fn test_register_seller_marketplace_paused() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    client.set_paused(&admin, &true);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);
}

#[test]
fn test_verify_seller() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);

    client.verify_seller(&admin, &seller);

    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.status.as_u32(), SellerStatus::Verified.as_u32());
}

#[test]
fn test_suspend_seller() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);
    client.verify_seller(&admin, &seller);

    client.suspend_seller(&admin, &seller);

    let seller_info = client.get_seller(&seller);
    assert_eq!(
        seller_info.status.as_u32(),
        SellerStatus::Suspended.as_u32()
    );
}

#[test]
fn test_unsuspend_seller() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);
    client.verify_seller(&admin, &seller);
    client.suspend_seller(&admin, &seller);

    client.unsuspend_seller(&admin, &seller);

    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.status.as_u32(), SellerStatus::Verified.as_u32());
}

#[test]
fn test_update_seller_rating() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);

    client.update_seller_rating(&admin, &seller, &400);

    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.rating, 400);
}

#[test]
#[should_panic]
fn test_update_seller_rating_invalid() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);

    client.update_seller_rating(&admin, &seller, &600);
}

#[test]
fn test_create_category() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let name = String::from_str(&e, "Electronics");
    let description = String::from_str(&e, "Electronic products");

    client.create_category(&admin, &1, &name, &description, &300);

    let category = client.get_category(&1);
    assert_eq!(category.id, 1);
    assert_eq!(category.commission_rate, 300);
}

#[test]
#[should_panic]
fn test_create_category_duplicate() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let name = String::from_str(&e, "Electronics");
    let description = String::from_str(&e, "Electronic products");

    client.create_category(&admin, &1, &name, &description, &300);
    client.create_category(&admin, &1, &name, &description, &300);
}

#[test]
#[should_panic]
fn test_create_category_invalid_fee_rate() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let name = String::from_str(&e, "Electronics");
    let description = String::from_str(&e, "Electronic products");

    client.create_category(&admin, &1, &name, &description, &10001);
}

#[test]
fn test_add_product() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let name = String::from_str(&e, "Electronics");
    let description = String::from_str(&e, "Electronic products");
    client.create_category(&admin, &1, &name, &description, &300);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);
    client.verify_seller(&admin, &seller);

    let product_name = String::from_str(&e, "Laptop");
    let product_desc = String::from_str(&e, "High performance laptop");
    let product_meta = String::from_str(&e, "{}");

    let result = client.add_product(
        &seller,
        &product_name,
        &product_desc,
        &1,
        &100_000_000,
        &10,
        &product_meta,
    );
    assert_eq!(result, 1);
}

#[test]
#[should_panic]
fn test_add_product_seller_not_verified() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let name = String::from_str(&e, "Electronics");
    let description = String::from_str(&e, "Electronic products");
    client.create_category(&admin, &1, &name, &description, &300);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);

    let product_name = String::from_str(&e, "Laptop");
    let product_desc = String::from_str(&e, "High performance laptop");
    let product_meta = String::from_str(&e, "{}");

    client.add_product(
        &seller,
        &product_name,
        &product_desc,
        &1,
        &100_000_000,
        &10,
        &product_meta,
    );
}

#[test]
#[should_panic]
fn test_add_product_invalid_category() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);
    client.verify_seller(&admin, &seller);

    let product_name = String::from_str(&e, "Laptop");
    let product_desc = String::from_str(&e, "High performance laptop");
    let product_meta = String::from_str(&e, "{}");

    client.add_product(
        &seller,
        &product_name,
        &product_desc,
        &999,
        &100_000_000,
        &10,
        &product_meta,
    );
}

#[test]
fn test_get_product() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let name = String::from_str(&e, "Electronics");
    let description = String::from_str(&e, "Electronic products");
    client.create_category(&admin, &1, &name, &description, &300);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);
    client.verify_seller(&admin, &seller);

    let product_name = String::from_str(&e, "Laptop");
    let product_desc = String::from_str(&e, "High performance laptop");
    let product_meta = String::from_str(&e, "{}");
    let product_id = client.add_product(
        &seller,
        &product_name,
        &product_desc,
        &1,
        &100_000_000,
        &10,
        &product_meta,
    );

    let product = client.get_product(&product_id);
    assert_eq!(product.id, product_id);
    assert_eq!(product.price, 100_000_000);
    assert_eq!(product.stock_quantity, 10);
}

#[test]
fn test_update_product() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let name = String::from_str(&e, "Electronics");
    let description = String::from_str(&e, "Electronic products");
    client.create_category(&admin, &1, &name, &description, &300);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);
    client.verify_seller(&admin, &seller);

    let product_name = String::from_str(&e, "Laptop");
    let product_desc = String::from_str(&e, "High performance laptop");
    let product_meta = String::from_str(&e, "{}");
    let product_id = client.add_product(
        &seller,
        &product_name,
        &product_desc,
        &1,
        &100_000_000,
        &10,
        &product_meta,
    );

    client.update_product(&seller, &product_id, &150_000_000, &5, &0);

    let product = client.get_product(&product_id);
    assert_eq!(product.price, 150_000_000);
    assert_eq!(product.stock_quantity, 5);
}

#[test]
fn test_delist_product() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let name = String::from_str(&e, "Electronics");
    let description = String::from_str(&e, "Electronic products");
    client.create_category(&admin, &1, &name, &description, &300);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);
    client.verify_seller(&admin, &seller);

    let product_name = String::from_str(&e, "Laptop");
    let product_desc = String::from_str(&e, "High performance laptop");
    let product_meta = String::from_str(&e, "{}");
    let product_id = client.add_product(
        &seller,
        &product_name,
        &product_desc,
        &1,
        &100_000_000,
        &10,
        &product_meta,
    );

    client.delist_product(&seller, &product_id);

    let product = client.get_product(&product_id);
    assert_eq!(product.status.as_u32(), ProductStatus::Delisted.as_u32());
}

#[test]
fn test_calculate_fee_base_rate() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let fee = client.calculate_fee(&1000_000, &None);
    assert_eq!(fee, 25000);
}

#[test]
fn test_calculate_fee_category_rate() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let name = String::from_str(&e, "Electronics");
    let description = String::from_str(&e, "Electronic products");
    client.create_category(&admin, &1, &name, &description, &300);

    let fee = client.calculate_fee(&1000_000, &Some(1));
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

    client.record_fee_collection(&admin, &1_000_000);

    let total_fees = client.get_total_fees();
    assert_eq!(total_fees, 1_000_000);
}

#[test]
fn test_get_products_by_seller() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let cat_name = String::from_str(&e, "Electronics");
    let cat_desc = String::from_str(&e, "Electronic products");
    client.create_category(&admin, &1, &cat_name, &cat_desc, &300);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);
    client.verify_seller(&admin, &seller);

    let product_name = String::from_str(&e, "Laptop");
    let product_desc = String::from_str(&e, "High performance laptop");
    let product_meta = String::from_str(&e, "{}");

    client.add_product(
        &seller,
        &product_name,
        &product_desc,
        &1,
        &100_000_000,
        &10,
        &product_meta,
    );
    client.add_product(
        &seller,
        &product_name,
        &product_desc,
        &1,
        &150_000_000,
        &5,
        &product_meta,
    );

    let products = client.get_products_by_seller(&seller);
    assert_eq!(products.len(), 2);
}

#[test]
fn test_get_products_by_category() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let cat_name = String::from_str(&e, "Electronics");
    let cat_desc = String::from_str(&e, "Electronic products");
    client.create_category(&admin, &1, &cat_name, &cat_desc, &300);

    let cat_name2 = String::from_str(&e, "Books");
    let cat_desc2 = String::from_str(&e, "Books");
    client.create_category(&admin, &2, &cat_name2, &cat_desc2, &200);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);
    client.verify_seller(&admin, &seller);

    let product_name = String::from_str(&e, "Product");
    let product_desc = String::from_str(&e, "Description");
    let product_meta = String::from_str(&e, "{}");

    client.add_product(
        &seller,
        &product_name,
        &product_desc,
        &1,
        &100_000_000,
        &10,
        &product_meta,
    );
    client.add_product(
        &seller,
        &product_name,
        &product_desc,
        &2,
        &100_000_000,
        &10,
        &product_meta,
    );

    let category_1_products = client.get_products_by_category(&1);
    let category_2_products = client.get_products_by_category(&2);

    assert_eq!(category_1_products.len(), 1);
    assert_eq!(category_2_products.len(), 1);
}

#[test]
fn test_get_stats() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);

    let stats = client.get_stats();
    assert_eq!(stats.0, 0);
    assert_eq!(stats.1, 0);
    assert_eq!(stats.2, 0);

    let seller = Address::generate(&e);
    let metadata = String::from_str(&e, "Test seller");
    client.register_seller(&seller, &metadata);

    let stats = client.get_stats();
    assert_eq!(stats.1, 1);
}

#[test]
fn test_complete_marketplace_workflow() {
    let (e, admin) = setup_env();
    let seller1 = Address::generate(&e);
    let seller2 = Address::generate(&e);

    let client = initialize_marketplace(&e, &admin);

    let config = client.get_config();
    assert_eq!(config.admin, admin);
    assert_eq!(config.base_fee_rate, 250);
    assert_eq!(config.total_products, 0);
    assert_eq!(config.total_sellers, 0);

    let electronics_name = String::from_str(&e, "Electronics");
    let electronics_desc = String::from_str(&e, "Electronic devices");
    client.create_category(&admin, &1, &electronics_name, &electronics_desc, &300);

    let books_name = String::from_str(&e, "Books");
    let books_desc = String::from_str(&e, "Physical and digital books");
    client.create_category(&admin, &2, &books_name, &books_desc, &200);

    let seller1_metadata = String::from_str(&e, "TechStore");
    let seller2_metadata = String::from_str(&e, "BookNook");

    client.register_seller(&seller1, &seller1_metadata);
    client.register_seller(&seller2, &seller2_metadata);

    client.verify_seller(&admin, &seller1);
    client.verify_seller(&admin, &seller2);

    let seller1_info = client.get_seller(&seller1);
    assert_eq!(
        seller1_info.status.as_u32(),
        SellerStatus::Verified.as_u32()
    );

    let laptop_name = String::from_str(&e, "Premium Laptop");
    let laptop_desc = String::from_str(&e, "High performance laptop");
    let laptop_meta = String::from_str(&e, "{}");

    let product1_id = client.add_product(
        &seller1,
        &laptop_name,
        &laptop_desc,
        &1,
        &99_999_999,
        &5,
        &laptop_meta,
    );
    assert_eq!(product1_id, 1);

    let book_name = String::from_str(&e, "Rust Programming");
    let book_desc = String::from_str(&e, "Learn Rust");
    let book_meta = String::from_str(&e, "{}");

    let product2_id = client.add_product(
        &seller2,
        &book_name,
        &book_desc,
        &2,
        &49_999_999,
        &20,
        &book_meta,
    );
    assert_eq!(product2_id, 2);

    let laptop_fee = client.calculate_fee(&99_999_999, &Some(1));
    assert_eq!(laptop_fee, 2_999_999);

    let book_fee = client.calculate_fee(&49_999_999, &Some(2));
    assert_eq!(book_fee, 999_999);

    let stats = client.get_stats();
    assert_eq!(stats.0, 2);
    assert_eq!(stats.1, 2);
    assert_eq!(stats.2, 0);

    let electronics_products = client.get_products_by_category(&1);
    assert_eq!(electronics_products.len(), 1);

    let seller1_products = client.get_products_by_seller(&seller1);
    assert_eq!(seller1_products.len(), 1);
}

#[test]
fn test_seller_lifecycle() {
    let (e, admin) = setup_env();
    let seller = Address::generate(&e);

    let client = initialize_marketplace(&e, &admin);

    let metadata = String::from_str(&e, "NewSeller");
    client.register_seller(&seller, &metadata);

    let seller_info = client.get_seller(&seller);
    assert_eq!(
        seller_info.status.as_u32(),
        SellerStatus::Unverified.as_u32()
    );
    assert_eq!(seller_info.total_sales, 0);
    assert_eq!(seller_info.rating, 0);

    client.verify_seller(&admin, &seller);
    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.status.as_u32(), SellerStatus::Verified.as_u32());

    client.update_seller_rating(&admin, &seller, &450);
    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.rating, 450);

    client.suspend_seller(&admin, &seller);
    let seller_info = client.get_seller(&seller);
    assert_eq!(
        seller_info.status.as_u32(),
        SellerStatus::Suspended.as_u32()
    );

    client.unsuspend_seller(&admin, &seller);
    let seller_info = client.get_seller(&seller);
    assert_eq!(seller_info.status.as_u32(), SellerStatus::Verified.as_u32());
}

#[test]
fn test_product_lifecycle() {
    let (e, admin) = setup_env();
    let seller = Address::generate(&e);

    let client = initialize_marketplace(&e, &admin);

    let cat_name = String::from_str(&e, "Electronics");
    let cat_desc = String::from_str(&e, "Electronic devices");
    client.create_category(&admin, &1, &cat_name, &cat_desc, &300);

    let seller_metadata = String::from_str(&e, "Seller");
    client.register_seller(&seller, &seller_metadata);
    client.verify_seller(&admin, &seller);

    let name = String::from_str(&e, "Smartphone");
    let desc = String::from_str(&e, "Latest smartphone model");
    let meta = String::from_str(&e, "{}");

    let product_id = client.add_product(&seller, &name, &desc, &1, &799_999_999, &50, &meta);
    assert_eq!(product_id, 1);

    let product = client.get_product(&product_id);
    assert_eq!(product.price, 799_999_999);
    assert_eq!(product.stock_quantity, 50);
    assert_eq!(product.status.as_u32(), ProductStatus::Active.as_u32());

    client.update_product(&seller, &product_id, &749_999_999, &40, &0);

    let product = client.get_product(&product_id);
    assert_eq!(product.price, 749_999_999);
    assert_eq!(product.stock_quantity, 40);

    client.update_product_rating(&seller, &product_id, &480);

    let product = client.get_product(&product_id);
    assert_eq!(product.rating, 480);

    client.delist_product(&seller, &product_id);

    let product = client.get_product(&product_id);
    assert_eq!(product.status.as_u32(), ProductStatus::Delisted.as_u32());
}

#[test]
fn test_fee_management() {
    let (e, admin) = setup_env();

    let client = initialize_marketplace(&e, &admin);

    let cat1_name = String::from_str(&e, "Premium");
    let cat1_desc = String::from_str(&e, "Premium products");
    client.create_category(&admin, &1, &cat1_name, &cat1_desc, &500);

    let cat2_name = String::from_str(&e, "Economy");
    let cat2_desc = String::from_str(&e, "Economy products");
    client.create_category(&admin, &2, &cat2_name, &cat2_desc, &100);

    let amount = 1_000_000_000;

    let base_fee = client.calculate_fee(&amount, &None);
    assert_eq!(base_fee, 25_000_000);

    let premium_fee = client.calculate_fee(&amount, &Some(1));
    assert_eq!(premium_fee, 50_000_000);

    let economy_fee = client.calculate_fee(&amount, &Some(2));
    assert_eq!(economy_fee, 10_000_000);

    client.record_fee_collection(&admin, &base_fee);
    client.record_fee_collection(&admin, &premium_fee);

    let total_fees = client.get_total_fees();
    assert_eq!(total_fees, 75_000_000);

    client.set_fee_rate(&admin, &350);

    let new_base_fee = client.calculate_fee(&amount, &None);
    assert_eq!(new_base_fee, 35_000_000);
}

#[test]
fn test_marketplace_configuration() {
    let (e, admin) = setup_env();

    let client = initialize_marketplace(&e, &admin);

    let config = client.get_config();
    assert_eq!(config.base_fee_rate, 250);

    client.set_fee_rate(&admin, &500);

    let config = client.get_config();
    assert_eq!(config.base_fee_rate, 500);

    let cat_name = String::from_str(&e, "Premium");
    let cat_desc = String::from_str(&e, "Premium products");
    client.create_category(&admin, &1, &cat_name, &cat_desc, &300);

    client.set_category_fee_rate(&admin, &1, &600);

    let fee_with_category = client.calculate_fee(&1_000_000_000, &Some(1));
    assert_eq!(fee_with_category, 60_000_000);
}

#[test]
fn test_configure_oracle() {
    let (e, admin) = setup_env();
    let stellar_oracle = Address::generate(&e);
    let external_oracle = Address::generate(&e);

    let client = initialize_marketplace(&e, &admin);

    client.configure_oracle(
        &admin,
        &stellar_oracle,
        &external_oracle,
        &300,
        &1000,
        &2000,
        &60,
    );

    let oracle_config = client.get_oracle_config();
    assert_eq!(oracle_config.stellar_oracle, stellar_oracle);
    assert_eq!(oracle_config.external_oracle, external_oracle);
    assert_eq!(oracle_config.staleness_threshold, 300);
    assert_eq!(oracle_config.price_deviation_threshold, 1000);
    assert_eq!(oracle_config.price_tolerance, 2000);
    assert_eq!(oracle_config.update_frequency, 60);
    assert_eq!(oracle_config.is_enabled, true);
}

#[test]
fn test_oracle_enable_disable() {
    let (e, admin) = setup_env();
    let stellar_oracle = Address::generate(&e);
    let external_oracle = Address::generate(&e);

    let client = initialize_marketplace(&e, &admin);
    client.configure_oracle(
        &admin,
        &stellar_oracle,
        &external_oracle,
        &300,
        &1000,
        &2000,
        &60,
    );

    client.set_oracle_enabled(&admin, &false);
    let oracle_config = client.get_oracle_config();
    assert_eq!(oracle_config.is_enabled, false);

    client.set_oracle_enabled(&admin, &true);
    let oracle_config = client.get_oracle_config();
    assert_eq!(oracle_config.is_enabled, true);
}

#[test]
fn test_update_oracle_address() {
    let (e, admin) = setup_env();
    let stellar_oracle = Address::generate(&e);
    let external_oracle = Address::generate(&e);
    let new_stellar_oracle = Address::generate(&e);
    let new_external_oracle = Address::generate(&e);

    let client = initialize_marketplace(&e, &admin);
    client.configure_oracle(
        &admin,
        &stellar_oracle,
        &external_oracle,
        &300,
        &1000,
        &2000,
        &60,
    );

    client.update_oracle_address(&admin, &0, &new_stellar_oracle);
    let oracle_config = client.get_oracle_config();
    assert_eq!(oracle_config.stellar_oracle, new_stellar_oracle);
    assert_eq!(oracle_config.external_oracle, external_oracle);

    client.update_oracle_address(&admin, &1, &new_external_oracle);
    let oracle_config = client.get_oracle_config();
    assert_eq!(oracle_config.external_oracle, new_external_oracle);
}

#[test]
#[should_panic]
fn test_oracle_not_configured() {
    let (e, admin) = setup_env();
    let client = initialize_marketplace(&e, &admin);
    client.get_oracle_config();
}

#[test]
fn test_price_staleness() {
    assert!(!OracleService::is_price_stale(100, 200, 300));
    assert!(OracleService::is_price_stale(100, 500, 300));
    assert!(!OracleService::is_price_stale(100, 400, 300));
    assert!(OracleService::is_price_stale(100, 401, 300));
}

#[test]
fn test_manipulation_detection() {
    assert!(!OracleService::detect_manipulation(105, 100, 1000));
    assert!(OracleService::detect_manipulation(115, 100, 1000));
    assert!(OracleService::detect_manipulation(85, 100, 1000));
    assert!(!OracleService::detect_manipulation(100, 0, 1000));
    assert!(!OracleService::detect_manipulation(100, 100, 1000));
}

#[test]
fn test_product_price_validation() {
    assert!(OracleService::validate_product_price(1000, 1000, 2000).is_ok());
    assert!(OracleService::validate_product_price(1000, 1200, 2000).is_ok());
    assert!(OracleService::validate_product_price(1000, 800, 2000).is_ok());
    assert!(OracleService::validate_product_price(1000, 1300, 2000).is_err());
    assert!(OracleService::validate_product_price(1000, 700, 2000).is_err());
    assert!(OracleService::validate_product_price(0, 1000, 2000).is_ok());
    assert!(OracleService::validate_product_price(-100, 1000, 2000).is_ok());
}

#[test]
fn test_price_validation_tolerances() {
    let oracle_price = 10000i128;

    assert!(OracleService::validate_product_price(oracle_price, 10000, 1000).is_ok());
    assert!(OracleService::validate_product_price(oracle_price, 11000, 1000).is_ok());
    assert!(OracleService::validate_product_price(oracle_price, 9000, 1000).is_ok());
    assert!(OracleService::validate_product_price(oracle_price, 11100, 1000).is_err());
    assert!(OracleService::validate_product_price(oracle_price, 8900, 1000).is_err());

    assert!(OracleService::validate_product_price(oracle_price, 15000, 5000).is_ok());
    assert!(OracleService::validate_product_price(oracle_price, 5000, 5000).is_ok());
    assert!(OracleService::validate_product_price(oracle_price, 15100, 5000).is_err());
    assert!(OracleService::validate_product_price(oracle_price, 4900, 5000).is_err());

    assert!(OracleService::validate_product_price(oracle_price, 10000, 0).is_ok());
    assert!(OracleService::validate_product_price(oracle_price, 10001, 0).is_err());
    assert!(OracleService::validate_product_price(oracle_price, 9999, 0).is_err());
}

#[test]
fn test_get_oracle_info() {
    let (e, admin) = setup_env();
    let stellar_oracle = Address::generate(&e);
    let external_oracle = Address::generate(&e);

    let client = initialize_marketplace(&e, &admin);
    client.configure_oracle(
        &admin,
        &stellar_oracle,
        &external_oracle,
        &300,
        &1000,
        &2000,
        &60,
    );

    let (is_enabled, last_update) = client.get_oracle_info();
    assert_eq!(is_enabled, true);
    assert_eq!(last_update, 0);
}

#[test]
fn test_oracle_complete_workflow() {
    let (e, admin) = setup_env();
    let stellar_oracle = Address::generate(&e);
    let external_oracle = Address::generate(&e);

    let client = initialize_marketplace(&e, &admin);

    client.configure_oracle(
        &admin,
        &stellar_oracle,
        &external_oracle,
        &300,
        &1000,
        &2000,
        &60,
    );

    let oracle_config = client.get_oracle_config();
    assert_eq!(oracle_config.is_enabled, true);

    let (is_enabled, _) = client.get_oracle_info();
    assert_eq!(is_enabled, true);

    client.set_oracle_enabled(&admin, &false);
    let oracle_config = client.get_oracle_config();
    assert_eq!(oracle_config.is_enabled, false);

    let new_stellar_oracle = Address::generate(&e);
    client.update_oracle_address(&admin, &0, &new_stellar_oracle);
    let oracle_config = client.get_oracle_config();
    assert_eq!(oracle_config.stellar_oracle, new_stellar_oracle);

    client.set_oracle_enabled(&admin, &true);
    let oracle_config = client.get_oracle_config();
    assert_eq!(oracle_config.is_enabled, true);
}

#[test]
fn test_price_history() {
    let (e, admin) = setup_env();
    let stellar_oracle = Address::generate(&e);
    let external_oracle = Address::generate(&e);
    let asset = Address::generate(&e);

    let client = initialize_marketplace(&e, &admin);

    client.configure_oracle(
        &admin,
        &stellar_oracle,
        &external_oracle,
        &300,
        &1000,
        &2000,
        &60,
    );

    let history = client.get_price_history(&asset, &10);
    assert_eq!(history.len(), 0);
}
