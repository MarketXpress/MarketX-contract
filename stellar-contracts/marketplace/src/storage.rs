use soroban_sdk::{Address, Env, Symbol, Vec};

use crate::types::{
    Category, MarketplaceConfig, OracleConfig, PriceRecord, Product, Seller, StorageKey,
    MAX_PRICE_RECORDS, PERSISTENT_TTL_AMOUNT, PERSISTENT_TTL_THRESHOLD,
};

pub fn is_initialized(e: &Env) -> bool {
    e.storage()
        .instance()
        .get::<_, bool>(&StorageKey::Initialized)
        .unwrap_or(false)
}

pub fn set_initialized(e: &Env) {
    e.storage()
        .instance()
        .set(&StorageKey::Initialized, &true);
}

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

pub fn set_config(e: &Env, config: &MarketplaceConfig) {
    let key = StorageKey::Config;
    e.storage().persistent().set(&key, config);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

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

pub fn set_seller(e: &Env, seller: &Seller) {
    let key = StorageKey::Seller(seller.address.clone());
    e.storage().persistent().set(&key, seller);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

pub fn seller_exists(e: &Env, seller_address: &Address) -> bool {
    let key = StorageKey::Seller(seller_address.clone());
    e.storage().persistent().has(&key)
}

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

pub fn set_product(e: &Env, product: &Product) {
    let key = StorageKey::Product(product.id);
    e.storage().persistent().set(&key, product);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

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

pub fn set_category(e: &Env, category: &Category) {
    let key = StorageKey::Category(category.id);
    e.storage().persistent().set(&key, category);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

pub fn category_exists(e: &Env, category_id: u32) -> bool {
    let key = StorageKey::Category(category_id);
    e.storage().persistent().has(&key)
}

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

pub fn add_seller_product(e: &Env, seller_address: &Address, product_id: u64) {
    let key = StorageKey::SellerProducts(seller_address.clone());
    let mut products = get_seller_products(e, seller_address);
    products.push_back(product_id);
    e.storage().persistent().set(&key, &products);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

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

pub fn add_category_product(e: &Env, category_id: u32, product_id: u64) {
    let key = StorageKey::CategoryProducts(category_id);
    let mut products = get_category_products(e, category_id);
    products.push_back(product_id);
    e.storage().persistent().set(&key, &products);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

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

pub fn add_fees(e: &Env, amount: u128) {
    let key = StorageKey::FeesCollected;
    let mut fees = get_total_fees(e);
    fees = fees.saturating_add(amount);
    e.storage().persistent().set(&key, &fees);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

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

pub fn increment_product_counter(e: &Env) {
    let key = StorageKey::ProductCounter;
    let counter = get_next_product_id(e);
    e.storage().persistent().set(&key, &counter);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

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

pub fn set_category_fee_rate(e: &Env, category_id: u32, rate: u32) {
    let key = StorageKey::CategoryFeeRate(category_id);
    e.storage().persistent().set(&key, &rate);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

pub fn get_oracle_config(e: &Env) -> Option<OracleConfig> {
    let key = StorageKey::OracleConfig;
    let config = e.storage().persistent().get::<_, OracleConfig>(&key);
    if config.is_some() {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    config
}

pub fn set_oracle_config(e: &Env, config: &OracleConfig) {
    let key = StorageKey::OracleConfig;
    e.storage().persistent().set(&key, config);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

pub fn get_price_history(e: &Env, asset_address: &Address) -> Vec<PriceRecord> {
    let key = StorageKey::PriceHistory(asset_address.clone());
    let history = e
        .storage()
        .persistent()
        .get::<_, Vec<PriceRecord>>(&key)
        .unwrap_or(Vec::new(e));
    if !history.is_empty() {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    history
}

pub fn add_price_record(e: &Env, asset_address: &Address, record: &PriceRecord) {
    let key = StorageKey::PriceHistory(asset_address.clone());
    let mut history = get_price_history(e, asset_address);

    if history.len() >= MAX_PRICE_RECORDS {
        let mut new_history = Vec::new(e);
        for i in 1..history.len() {
            new_history.push_back(history.get(i).unwrap());
        }
        history = new_history;
    }

    history.push_back(record.clone());
    e.storage().persistent().set(&key, &history);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

pub fn get_external_price_history(e: &Env, symbol: &Symbol) -> Vec<PriceRecord> {
    let key = StorageKey::ExternalPriceHistory(symbol.clone());
    let history = e
        .storage()
        .persistent()
        .get::<_, Vec<PriceRecord>>(&key)
        .unwrap_or(Vec::new(e));
    if !history.is_empty() {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    history
}

pub fn add_external_price_record(e: &Env, symbol: &Symbol, record: &PriceRecord) {
    let key = StorageKey::ExternalPriceHistory(symbol.clone());
    let mut history = get_external_price_history(e, symbol);

    if history.len() >= MAX_PRICE_RECORDS {
        let mut new_history = Vec::new(e);
        for i in 1..history.len() {
            new_history.push_back(history.get(i).unwrap());
        }
        history = new_history;
    }

    history.push_back(record.clone());
    e.storage().persistent().set(&key, &history);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}

pub fn get_last_price_update(e: &Env) -> u64 {
    let key = StorageKey::LastPriceUpdate;
    let timestamp = e.storage().persistent().get::<_, u64>(&key).unwrap_or(0);
    if timestamp > 0 {
        e.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
    }
    timestamp
}

pub fn set_last_price_update(e: &Env, timestamp: u64) {
    let key = StorageKey::LastPriceUpdate;
    e.storage().persistent().set(&key, &timestamp);
    e.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_TTL_AMOUNT);
}
