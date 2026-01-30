use soroban_sdk::{Address, Env, Vec};

use crate::types::{
    Category, MarketplaceConfig, Order, OrderStatus, Product, Seller, StorageKey,
    PERSISTENT_TTL_AMOUNT, PERSISTENT_TTL_THRESHOLD,
};

// ============================================================================
// INITIALIZATION STORAGE
// ============================================================================

/// Check if contract is initialized
pub fn is_initialized(e: &Env) -> bool {
    e.storage()
        .instance()
        .get::<_, bool>(&StorageKey::Initialized)
        .unwrap_or(false)
}

/// Mark contract as initialized
pub fn set_initialized(e: &Env) {
    e.storage()
        .instance()
        .set(&StorageKey::Initialized, &true);
}

// ============================================================================
// CONFIG STORAGE
// ============================================================================

/// Get marketplace configuration
pub fn get_config(e: &Env) -> Option<MarketplaceConfig> {
    let key = StorageKey::Config;
    let config = e.storage().persistent().get::<_, MarketplaceConfig>(&key);
    if config.is_some() {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    config
}

/// Set marketplace configuration
pub fn set_config(e: &Env, config: &MarketplaceConfig) {
    let key = StorageKey::Config;
    e.storage().persistent().set(&key, config);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

// ============================================================================
// SELLER STORAGE
// ============================================================================

/// Get seller information by address
pub fn get_seller(e: &Env, seller_address: &Address) -> Option<Seller> {
    let key = StorageKey::Seller(seller_address.clone());
    let seller = e.storage().persistent().get::<_, Seller>(&key);
    if seller.is_some() {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    seller
}

/// Set seller information
pub fn set_seller(e: &Env, seller: &Seller) {
    let key = StorageKey::Seller(seller.address.clone());
    e.storage().persistent().set(&key, seller);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

/// Check if seller exists
pub fn seller_exists(e: &Env, seller_address: &Address) -> bool {
    let key = StorageKey::Seller(seller_address.clone());
    e.storage().persistent().has(&key)
}

// ============================================================================
// PRODUCT STORAGE
// ============================================================================

/// Get product information by ID
pub fn get_product(e: &Env, product_id: u64) -> Option<Product> {
    let key = StorageKey::Product(product_id);
    let product = e.storage().persistent().get::<_, Product>(&key);
    if product.is_some() {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    product
}

/// Set product information
pub fn set_product(e: &Env, product: &Product) {
    let key = StorageKey::Product(product.id);
    e.storage().persistent().set(&key, product);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

// ============================================================================
// CATEGORY STORAGE
// ============================================================================

/// Get category information by ID
pub fn get_category(e: &Env, category_id: u32) -> Option<Category> {
    let key = StorageKey::Category(category_id);
    let category = e.storage().persistent().get::<_, Category>(&key);
    if category.is_some() {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    category
}

/// Set category information
pub fn set_category(e: &Env, category: &Category) {
    let key = StorageKey::Category(category.id);
    e.storage().persistent().set(&key, category);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

/// Check if category exists
pub fn category_exists(e: &Env, category_id: u32) -> bool {
    let key = StorageKey::Category(category_id);
    e.storage().persistent().has(&key)
}

// ============================================================================
// SELLER PRODUCTS STORAGE
// ============================================================================

/// Get all product IDs for a seller
pub fn get_seller_products(e: &Env, seller_address: &Address) -> Vec<u64> {
    let key = StorageKey::SellerProducts(seller_address.clone());
    let products = e
        .storage()
        .persistent()
        .get::<_, Vec<u64>>(&key)
        .unwrap_or(Vec::new(e));
    if !products.is_empty() {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    products
}

/// Add product to seller's product list
pub fn add_seller_product(e: &Env, seller_address: &Address, product_id: u64) {
    let key = StorageKey::SellerProducts(seller_address.clone());
    let mut products = get_seller_products(e, seller_address);
    products.push_back(product_id);
    e.storage().persistent().set(&key, &products);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

// ============================================================================
// CATEGORY PRODUCTS STORAGE
// ============================================================================

/// Get all product IDs for a category
pub fn get_category_products(e: &Env, category_id: u32) -> Vec<u64> {
    let key = StorageKey::CategoryProducts(category_id);
    let products = e
        .storage()
        .persistent()
        .get::<_, Vec<u64>>(&key)
        .unwrap_or(Vec::new(e));
    if !products.is_empty() {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    products
}

/// Add product to category's product list
pub fn add_category_product(e: &Env, category_id: u32, product_id: u64) {
    let key = StorageKey::CategoryProducts(category_id);
    let mut products = get_category_products(e, category_id);
    products.push_back(product_id);
    e.storage().persistent().set(&key, &products);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

// ============================================================================
// FEES STORAGE
// ============================================================================

/// Get total collected fees
pub fn get_total_fees(e: &Env) -> u128 {
    let key = StorageKey::FeesCollected;
    let fees = e.storage().persistent().get::<_, u128>(&key).unwrap_or(0);
    if fees > 0 {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    fees
}

/// Add to total collected fees
pub fn add_fees(e: &Env, amount: u128) {
    let key = StorageKey::FeesCollected;
    let mut fees = get_total_fees(e);
    fees = fees.checked_add(amount).unwrap_or(u128::MAX);
    e.storage().persistent().set(&key, &fees);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

// ============================================================================
// PRODUCT COUNTER STORAGE
// ============================================================================

/// Get next product ID
pub fn get_next_product_id(e: &Env) -> u64 {
    let key = StorageKey::ProductCounter;
    let counter = e.storage().persistent().get::<_, u64>(&key).unwrap_or(0);
    if counter > 0 {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    counter + 1
}

/// Increment product counter
pub fn increment_product_counter(e: &Env) {
    let key = StorageKey::ProductCounter;
    let counter = get_next_product_id(e);
    e.storage().persistent().set(&key, &counter);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

// ============================================================================
// CATEGORY FEE RATE STORAGE
// ============================================================================

/// Get fee rate for a specific category
pub fn get_category_fee_rate(e: &Env, category_id: u32) -> Option<u32> {
    let key = StorageKey::CategoryFeeRate(category_id);
    let rate = e.storage().persistent().get::<_, u32>(&key);
    if rate.is_some() {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    rate
}

/// Set fee rate for a specific category
pub fn set_category_fee_rate(e: &Env, category_id: u32, rate: u32) {
    let key = StorageKey::CategoryFeeRate(category_id);
    e.storage().persistent().set(&key, &rate);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

// ============================================================================
// ORDER STORAGE
// ============================================================================

/// Get order information by ID
pub fn get_order(e: &Env, order_id: u64) -> Option<Order> {
    let key = StorageKey::Order(order_id);
    let order = e.storage().persistent().get::<_, Order>(&key);
    if order.is_some() {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    order
}

/// Set order information
pub fn set_order(e: &Env, order: &Order) {
    let key = StorageKey::Order(order.id);
    e.storage().persistent().set(&key, order);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

/// Get next order ID
pub fn get_next_order_id(e: &Env) -> u64 {
    let key = StorageKey::OrderCounter;
    let counter = e.storage().persistent().get::<_, u64>(&key).unwrap_or(0);
    if counter > 0 {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    counter + 1
}

/// Increment order counter
pub fn increment_order_counter(e: &Env) {
    let key = StorageKey::OrderCounter;
    let counter = get_next_order_id(e);
    e.storage().persistent().set(&key, &counter);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

/// Get all order IDs for a buyer
pub fn get_buyer_orders(e: &Env, buyer: &Address) -> Vec<u64> {
    let key = StorageKey::BuyerOrders(buyer.clone());
    let orders = e
        .storage()
        .persistent()
        .get::<_, Vec<u64>>(&key)
        .unwrap_or(Vec::new(e));
    if !orders.is_empty() {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    orders
}

/// Add order to buyer's list
pub fn add_buyer_order(e: &Env, buyer: &Address, order_id: u64) {
    let key = StorageKey::BuyerOrders(buyer.clone());
    let mut orders = get_buyer_orders(e, buyer);
    orders.push_back(order_id);
    e.storage().persistent().set(&key, &orders);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

/// Get all order IDs for a seller
pub fn get_seller_orders(e: &Env, seller: &Address) -> Vec<u64> {
    let key = StorageKey::SellerOrders(seller.clone());
    let orders = e
        .storage()
        .persistent()
        .get::<_, Vec<u64>>(&key)
        .unwrap_or(Vec::new(e));
    if !orders.is_empty() {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    orders
}

/// Add order to seller's list
pub fn add_seller_order(e: &Env, seller: &Address, order_id: u64) {
    let key = StorageKey::SellerOrders(seller.clone());
    let mut orders = get_seller_orders(e, seller);
    orders.push_back(order_id);
    e.storage().persistent().set(&key, &orders);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}
