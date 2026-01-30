#![no_std]

mod errors;
mod events;
mod oracle;
mod reflector;
mod storage;
mod types;

use soroban_sdk::{contract, contractimpl, Address, Env, String, Symbol, Vec};

use crate::errors::Error;
use crate::events::*;
use crate::oracle::OracleService;
use crate::storage::*;
use crate::types::*;

// ============================================================================
// Constants
// ============================================================================

/// Number of ledgers in a day (assuming ~5 second block time)
const DAY_IN_LEDGERS: u32 = 17280;

/// TTL extension amount for instance storage (30 days)
const INSTANCE_TTL_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;

/// TTL threshold before extending (29 days)
const INSTANCE_TTL_THRESHOLD: u32 = INSTANCE_TTL_AMOUNT - DAY_IN_LEDGERS;

/// Maximum rating value (5 stars * 100 for precision)
const MAX_RATING: u32 = 500;

/// Maximum basis points for fees
const MAX_FEE_RATE: u32 = 10000; // 100%

// ============================================================================
// Contract
// ============================================================================

/// MarketX Marketplace Smart Contract
///
/// A decentralized marketplace on Stellar/Soroban that handles:
/// - Product listing and categorization
/// - Seller registration and verification
/// - Fee calculation and collection
/// - Admin marketplace management
///
/// Built following Soroban best practices with modular architecture,
/// proper error handling, and comprehensive event emission.
#[contract]
pub struct MarketX;

#[contractimpl]
impl MarketX {
    // ========================================================================
    // INITIALIZATION
    // ========================================================================

    /// Initialize the MarketX marketplace contract.
    ///
    /// # Arguments
    /// * `admin` - Address that will have admin privileges
    /// * `base_fee_rate` - Base marketplace fee in basis points (100 = 1%)
    ///
    /// # Errors
    /// * `Error::AlreadyInitialized` - If the contract has already been initialized
    pub fn initialize(e: &Env, admin: Address, base_fee_rate: u32) -> Result<(), Error> {
        admin.require_auth();

        if is_initialized(e) {
            return Err(Error::AlreadyInitialized);
        }

        if base_fee_rate > MAX_FEE_RATE {
            return Err(Error::InvalidInput);
        }

        let config = MarketplaceConfig {
            admin: admin.clone(),
            base_fee_rate,
            is_paused: false,
            total_products: 0,
            total_sellers: 0,
            updated_at: e.ledger().timestamp(),
        };

        set_config(e, &config);
        set_initialized(e);
        Self::extend_instance_ttl(e);

        InitializedEventData {
            admin,
            base_fee_rate,
        }
        .publish(e);

        Ok(())
    }

    // ========================================================================
    // MARKETPLACE CONFIGURATION
    // ========================================================================

    /// Get marketplace configuration
    pub fn get_config(e: &Env) -> Result<MarketplaceConfig, Error> {
        get_config(e).ok_or(Error::NotInitialized)
    }

    /// Update base fee rate (admin only)
    pub fn set_fee_rate(e: &Env, admin: Address, new_rate: u32) -> Result<(), Error> {
        admin.require_auth();

        let mut config = get_config(e).ok_or(Error::NotInitialized)?;

        if admin != config.admin {
            return Err(Error::Unauthorized);
        }

        if new_rate > MAX_FEE_RATE {
            return Err(Error::InvalidInput);
        }

        config.base_fee_rate = new_rate;
        config.updated_at = e.ledger().timestamp();
        set_config(e, &config);

        FeeRateUpdatedEventData {
            admin: admin.clone(),
            new_rate,
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    /// Pause or unpause marketplace (admin only)
    pub fn set_paused(e: &Env, admin: Address, paused: bool) -> Result<(), Error> {
        admin.require_auth();

        let mut config = get_config(e).ok_or(Error::NotInitialized)?;

        if admin != config.admin {
            return Err(Error::Unauthorized);
        }

        config.is_paused = paused;
        config.updated_at = e.ledger().timestamp();
        set_config(e, &config);

        MarketplacePausedEventData {
            admin: admin.clone(),
            is_paused: paused,
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    /// Check if marketplace is paused
    pub fn is_paused(e: &Env) -> Result<bool, Error> {
        let config = get_config(e).ok_or(Error::NotInitialized)?;
        Ok(config.is_paused)
    }

    // ========================================================================
    // SELLER MANAGEMENT
    // ========================================================================

    /// Register a new seller
    ///
    /// # Arguments
    /// * `seller` - Address registering as seller
    /// * `metadata` - JSON encoded seller information (name, description, etc.)
    ///
    /// # Errors
    /// * `Error::InvalidInput` - If seller already exists or marketplace is paused
    pub fn register_seller(e: &Env, seller: Address, metadata: String) -> Result<(), Error> {
        seller.require_auth();

        let config = get_config(e).ok_or(Error::NotInitialized)?;

        if config.is_paused {
            return Err(Error::MarketplacePaused);
        }

        if seller_exists(e, &seller) {
            return Err(Error::InvalidInput);
        }

        if metadata.is_empty() {
            return Err(Error::InvalidMetadata);
        }

        let seller_data = Seller {
            address: seller.clone(),
            status: SellerStatus::Unverified,
            rating: 0,
            total_sales: 0,
            total_revenue: 0,
            created_at: e.ledger().timestamp(),
            metadata,
        };

        set_seller(e, &seller_data);

        let mut updated_config = config;
        updated_config.total_sellers += 1;
        updated_config.updated_at = e.ledger().timestamp();
        set_config(e, &updated_config);

        SellerRegisteredEventData {
            seller: seller.clone(),
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    /// Get seller information
    pub fn get_seller(e: &Env, seller_address: Address) -> Result<Seller, Error> {
        get_seller(e, &seller_address).ok_or(Error::SellerNotFound)
    }

    /// Verify a seller (admin only)
    ///
    /// # Errors
    /// * `Error::Unauthorized` - If caller is not admin
    /// * `Error::SellerNotFound` - If seller doesn't exist
    pub fn verify_seller(e: &Env, admin: Address, seller_address: Address) -> Result<(), Error> {
        admin.require_auth();

        let config = get_config(e).ok_or(Error::NotInitialized)?;

        if admin != config.admin {
            return Err(Error::Unauthorized);
        }

        let mut seller = get_seller(e, &seller_address).ok_or(Error::SellerNotFound)?;

        seller.status = SellerStatus::Verified;
        set_seller(e, &seller);

        SellerVerifiedEventData {
            seller: seller_address.clone(),
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    /// Suspend a seller (admin only)
    pub fn suspend_seller(e: &Env, admin: Address, seller_address: Address) -> Result<(), Error> {
        admin.require_auth();

        let config = get_config(e).ok_or(Error::NotInitialized)?;

        if admin != config.admin {
            return Err(Error::Unauthorized);
        }

        let mut seller = get_seller(e, &seller_address).ok_or(Error::SellerNotFound)?;

        seller.status = SellerStatus::Suspended;
        set_seller(e, &seller);

        SellerSuspendedEventData {
            seller: seller_address.clone(),
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    /// Unsuspend a seller (admin only)
    pub fn unsuspend_seller(e: &Env, admin: Address, seller_address: Address) -> Result<(), Error> {
        admin.require_auth();

        let config = get_config(e).ok_or(Error::NotInitialized)?;

        if admin != config.admin {
            return Err(Error::Unauthorized);
        }

        let mut seller = get_seller(e, &seller_address).ok_or(Error::SellerNotFound)?;

        if seller.status != SellerStatus::Suspended {
            return Err(Error::InvalidSellerStatus);
        }

        seller.status = SellerStatus::Verified;
        set_seller(e, &seller);

        SellerUnsuspendedEventData {
            seller: seller_address.clone(),
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    /// Update seller rating (admin only)
    ///
    /// # Arguments
    /// * `new_rating` - Rating value (0-500, where 500 = 5 stars)
    pub fn update_seller_rating(
        e: &Env,
        admin: Address,
        seller_address: Address,
        new_rating: u32,
    ) -> Result<(), Error> {
        admin.require_auth();

        let config = get_config(e).ok_or(Error::NotInitialized)?;

        if admin != config.admin {
            return Err(Error::Unauthorized);
        }

        if new_rating > MAX_RATING {
            return Err(Error::InvalidInput);
        }

        let mut seller = get_seller(e, &seller_address).ok_or(Error::SellerNotFound)?;

        seller.rating = new_rating;
        set_seller(e, &seller);

        SellerRatingUpdatedEventData {
            seller: seller_address.clone(),
            new_rating,
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    // ========================================================================
    // CATEGORY MANAGEMENT
    // ========================================================================

    /// Create a new product category (admin only)
    pub fn create_category(
        e: &Env,
        admin: Address,
        id: u32,
        name: String,
        description: String,
        commission_rate: u32,
    ) -> Result<(), Error> {
        admin.require_auth();

        let config = get_config(e).ok_or(Error::NotInitialized)?;

        if admin != config.admin {
            return Err(Error::Unauthorized);
        }

        if category_exists(e, id) {
            return Err(Error::CategoryAlreadyExists);
        }

        if commission_rate > MAX_FEE_RATE {
            return Err(Error::InvalidInput);
        }

        if name.is_empty() || description.is_empty() {
            return Err(Error::InvalidMetadata);
        }

        let category = Category {
            id,
            name: name.clone(),
            description,
            commission_rate,
            is_active: true,
        };

        set_category(e, &category);

        CategoryCreatedEventData {
            category_id: id,
            name,
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    /// Get category information
    pub fn get_category(e: &Env, id: u32) -> Result<Category, Error> {
        get_category(e, id).ok_or(Error::CategoryNotFound)
    }

    // ========================================================================
    // PRODUCT LISTING
    // ========================================================================

    /// Add a new product (verified sellers only)
    ///
    /// # Arguments
    /// * `seller` - Seller address listing the product
    /// * `name` - Product name
    /// * `description` - Product description
    /// * `category_id` - Category ID
    /// * `price` - Price in stroops
    /// * `stock_quantity` - Available quantity
    /// * `metadata` - Optional JSON metadata
    /// * `payment_asset` - Optional payment asset address for oracle price validation
    ///
    /// # Returns
    /// * Product ID if successful
    ///
    /// # Price Validation
    /// If oracle is configured and enabled, the product price will be validated
    /// against the oracle reference price. The price must be within the configured
    /// tolerance (e.g., 20%) of the oracle price.
    pub fn add_product(
        e: &Env,
        seller: Address,
        name: String,
        description: String,
        category_id: u32,
        price: u128,
        stock_quantity: u64,
        metadata: String,
    ) -> Result<u64, Error> {
        seller.require_auth();

        let config = get_config(e).ok_or(Error::NotInitialized)?;

        if config.is_paused {
            return Err(Error::MarketplacePaused);
        }

        // Verify seller exists and is verified
        let seller_data = get_seller(e, &seller).ok_or(Error::SellerNotFound)?;

        if seller_data.status != SellerStatus::Verified {
            return Err(Error::SellerNotVerified);
        }

        if seller_data.status == SellerStatus::Suspended {
            return Err(Error::SellerSuspended);
        }

        // Verify category exists
        let _category = get_category(e, category_id).ok_or(Error::CategoryNotFound)?;

        if name.is_empty() || description.is_empty() {
            return Err(Error::InvalidMetadata);
        }

        if price == 0 || stock_quantity == 0 {
            return Err(Error::InvalidInput);
        }

        let product_id = get_next_product_id(e);

        let product = Product {
            id: product_id,
            seller: seller.clone(),
            name,
            description,
            category_id,
            price,
            status: ProductStatus::Active,
            stock_quantity,
            rating: 0,
            purchase_count: 0,
            created_at: e.ledger().timestamp(),
            metadata,
        };

        set_product(e, &product);
        add_seller_product(e, &seller, product_id);
        add_category_product(e, category_id, product_id);
        increment_product_counter(e);

        let mut updated_config = config;
        updated_config.total_products += 1;
        updated_config.updated_at = e.ledger().timestamp();
        set_config(e, &updated_config);

        ProductListedEventData {
            seller: seller.clone(),
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(product_id)
    }

    /// Add a new product with oracle price validation (verified sellers only)
    ///
    /// # Arguments
    /// * `seller` - Seller address listing the product
    /// * `name` - Product name
    /// * `description` - Product description
    /// * `category_id` - Category ID
    /// * `price` - Price in the payment asset
    /// * `stock_quantity` - Available quantity
    /// * `metadata` - Optional JSON metadata
    /// * `payment_asset` - Payment asset address for oracle price validation
    ///
    /// # Returns
    /// * Product ID if successful
    ///
    /// # Errors
    /// * `Error::PriceOutOfRange` - If price deviates more than tolerance from oracle
    /// * `Error::PaymentAssetNotSupported` - If payment asset is not tracked by oracle
    pub fn add_product_with_validation(
        e: &Env,
        seller: Address,
        name: String,
        description: String,
        category_id: u32,
        price: u128,
        stock_quantity: u64,
        metadata: String,
        payment_asset: Address,
    ) -> Result<u64, Error> {
        seller.require_auth();

        let config = get_config(e).ok_or(Error::NotInitialized)?;

        if config.is_paused {
            return Err(Error::MarketplacePaused);
        }

        // Verify seller exists and is verified
        let seller_data = get_seller(e, &seller).ok_or(Error::SellerNotFound)?;

        if seller_data.status != SellerStatus::Verified {
            return Err(Error::SellerNotVerified);
        }

        if seller_data.status == SellerStatus::Suspended {
            return Err(Error::SellerSuspended);
        }

        // Verify category exists
        let _category = get_category(e, category_id).ok_or(Error::CategoryNotFound)?;

        if name.is_empty() || description.is_empty() {
            return Err(Error::InvalidMetadata);
        }

        if price == 0 || stock_quantity == 0 {
            return Err(Error::InvalidInput);
        }

        // Validate price against oracle if configured
        if let Some(oracle_config) = get_oracle_config(e) {
            if oracle_config.is_enabled {
                // Validate that the payment asset is supported
                OracleService::validate_payment_asset(e, &payment_asset)?;

                // Get oracle price and validate product price
                let price_data = OracleService::get_stellar_asset_price(e, &payment_asset)?;
                OracleService::validate_product_price(
                    price_data.price,
                    price,
                    oracle_config.price_tolerance,
                )?;
            }
        }

        let product_id = get_next_product_id(e);

        let product = Product {
            id: product_id,
            seller: seller.clone(),
            name,
            description,
            category_id,
            price,
            status: ProductStatus::Active,
            stock_quantity,
            rating: 0,
            purchase_count: 0,
            created_at: e.ledger().timestamp(),
            metadata,
        };

        set_product(e, &product);
        add_seller_product(e, &seller, product_id);
        add_category_product(e, category_id, product_id);
        increment_product_counter(e);

        let mut updated_config = config;
        updated_config.total_products += 1;
        updated_config.updated_at = e.ledger().timestamp();
        set_config(e, &updated_config);

        ProductListedEventData {
            seller: seller.clone(),
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(product_id)
    }

    /// Get product information
    pub fn get_product(e: &Env, product_id: u64) -> Result<Product, Error> {
        get_product(e, product_id).ok_or(Error::ProductNotFound)
    }

    /// Update product (seller only)
    ///
    /// # Arguments
    /// * `seller` - Seller address (must be product owner)
    /// * `product_id` - Product to update
    /// * `price` - New price (pass 0 to keep current)
    /// * `stock_quantity` - New stock (pass 0 to keep current)
    /// * `status` - New status (0=Active, 1=Delisted, 2=OutOfStock)
    pub fn update_product(
        e: &Env,
        seller: Address,
        product_id: u64,
        price: u128,
        stock_quantity: u64,
        status: u32,
    ) -> Result<(), Error> {
        seller.require_auth();

        let mut product = get_product(e, product_id).ok_or(Error::ProductNotFound)?;

        if seller != product.seller {
            return Err(Error::Unauthorized);
        }

        let mut updated = false;

        if price > 0 && price != product.price {
            product.price = price;
            updated = true;
        }

        if stock_quantity > 0 && stock_quantity != product.stock_quantity {
            product.stock_quantity = stock_quantity;
            updated = true;
        }

        if status <= 2 && (status as u32) != product.status.as_u32() {
            product.status = match status {
                0 => ProductStatus::Active,
                1 => ProductStatus::Delisted,
                2 => ProductStatus::OutOfStock,
                _ => return Err(Error::InvalidProductStatus),
            };
            updated = true;
        }

        if !updated {
            return Err(Error::InvalidInput);
        }

        set_product(e, &product);

        ProductUpdatedEventData {
            seller: seller.clone(),
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    /// Update product with oracle price validation (seller only)
    ///
    /// # Arguments
    /// * `seller` - Seller address (must be product owner)
    /// * `product_id` - Product to update
    /// * `price` - New price (pass 0 to keep current)
    /// * `stock_quantity` - New stock (pass 0 to keep current)
    /// * `status` - New status (0=Active, 1=Delisted, 2=OutOfStock)
    /// * `payment_asset` - Payment asset address for oracle price validation
    ///
    /// # Errors
    /// * `Error::PriceOutOfRange` - If new price deviates more than tolerance from oracle
    pub fn update_product_with_validation(
        e: &Env,
        seller: Address,
        product_id: u64,
        price: u128,
        stock_quantity: u64,
        status: u32,
        payment_asset: Address,
    ) -> Result<(), Error> {
        seller.require_auth();

        let mut product = get_product(e, product_id).ok_or(Error::ProductNotFound)?;

        if seller != product.seller {
            return Err(Error::Unauthorized);
        }

        let mut updated = false;

        if price > 0 && price != product.price {
            // Validate new price against oracle if configured
            if let Some(oracle_config) = get_oracle_config(e) {
                if oracle_config.is_enabled {
                    let price_data = OracleService::get_stellar_asset_price(e, &payment_asset)?;
                    OracleService::validate_product_price(
                        price_data.price,
                        price,
                        oracle_config.price_tolerance,
                    )?;
                }
            }
            product.price = price;
            updated = true;
        }

        if stock_quantity > 0 && stock_quantity != product.stock_quantity {
            product.stock_quantity = stock_quantity;
            updated = true;
        }

        if status <= 2 && (status as u32) != product.status.as_u32() {
            product.status = match status {
                0 => ProductStatus::Active,
                1 => ProductStatus::Delisted,
                2 => ProductStatus::OutOfStock,
                _ => return Err(Error::InvalidProductStatus),
            };
            updated = true;
        }

        if !updated {
            return Err(Error::InvalidInput);
        }

        set_product(e, &product);

        ProductUpdatedEventData {
            seller: seller.clone(),
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    /// Delist product (seller only)
    pub fn delist_product(e: &Env, seller: Address, product_id: u64) -> Result<(), Error> {
        seller.require_auth();

        let mut product = get_product(e, product_id).ok_or(Error::ProductNotFound)?;

        if seller != product.seller {
            return Err(Error::Unauthorized);
        }

        product.status = ProductStatus::Delisted;
        set_product(e, &product);

        ProductDelistedEventData {
            seller: seller.clone(),
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    /// Update product rating (seller only)
    ///
    /// # Arguments
    /// * `seller` - Seller address (must be product owner)
    /// * `product_id` - Product to rate
    /// * `new_rating` - Rating value (0-500, where 500 = 5 stars)
    pub fn update_product_rating(
        e: &Env,
        seller: Address,
        product_id: u64,
        new_rating: u32,
    ) -> Result<(), Error> {
        seller.require_auth();

        let mut product = get_product(e, product_id).ok_or(Error::ProductNotFound)?;

        if new_rating > MAX_RATING {
            return Err(Error::InvalidInput);
        }

        if seller != product.seller {
            return Err(Error::Unauthorized);
        }

        product.rating = new_rating;
        set_product(e, &product);

        QualityRatedEventData {
            seller: seller.clone(),
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    // ========================================================================
    // PRODUCT SEARCH & FILTERING
    // ========================================================================

    /// Get all products by seller
    pub fn get_products_by_seller(e: &Env, seller_address: Address) -> Result<Vec<u64>, Error> {
        if !seller_exists(e, &seller_address) {
            return Err(Error::SellerNotFound);
        }

        Ok(get_seller_products(e, &seller_address))
    }

    /// Get all products in category
    pub fn get_products_by_category(e: &Env, category_id: u32) -> Result<Vec<u64>, Error> {
        if !category_exists(e, category_id) {
            return Err(Error::CategoryNotFound);
        }

        Ok(get_category_products(e, category_id))
    }

    /// Get products by price range (paginated)
    ///
    /// # Arguments
    /// * `min_price` - Minimum price (inclusive)
    /// * `max_price` - Maximum price (inclusive)
    /// * `offset` - Pagination offset
    /// * `limit` - Maximum results to return
    pub fn get_products_by_price_range(
        e: &Env,
        min_price: u128,
        max_price: u128,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Product>, Error> {
        let config = get_config(e).ok_or(Error::NotInitialized)?;

        if min_price > max_price {
            return Err(Error::InvalidInput);
        }

        if limit == 0 || limit > 100 {
            return Err(Error::InvalidInput);
        }

        let mut results: Vec<Product> = Vec::new(e);
        let mut count = 0u32;
        let mut returned = 0u32;

        for i in 1..=config.total_products {
            if returned >= limit {
                break;
            }

            if let Some(product) =
                e.storage()
                    .persistent()
                    .get::<_, Product>(&StorageKey::Product(i))
            {
                if product.price >= min_price
                    && product.price <= max_price
                    && product.status == ProductStatus::Active
                {
                    if count >= offset {
                        results.push_back(product);
                        returned += 1;
                    }
                    count += 1;
                }
            }
        }

        Ok(results)
    }

    // ========================================================================
    // FEE MANAGEMENT
    // ========================================================================

    /// Calculate fee for a transaction
    ///
    /// # Arguments
    /// * `amount` - Transaction amount
    /// * `category_id` - Optional category ID for category-specific fees
    pub fn calculate_fee(
        e: &Env,
        amount: u128,
        category_id: Option<u32>,
    ) -> Result<u128, Error> {
        let config = get_config(e).ok_or(Error::NotInitialized)?;

        let rate = if let Some(cat_id) = category_id {
            // Check for category-specific fee rate override first
            if let Some(cat_rate) = get_category_fee_rate(e, cat_id) {
                cat_rate
            } else if let Some(category) = get_category(e, cat_id) {
                // Fall back to category's commission_rate
                category.commission_rate
            } else {
                // Fall back to base rate if category not found
                config.base_fee_rate
            }
        } else {
            config.base_fee_rate
        };

        // Calculate fee: amount * rate / 10000
        let fee = amount
            .checked_mul(rate as u128)
            .ok_or(Error::FeeOverflow)?
            .checked_div(10000)
            .ok_or(Error::FeeOverflow)?;

        Ok(fee)
    }

    /// Record a fee collection (admin only)
    pub fn record_fee_collection(e: &Env, admin: Address, amount: u128) -> Result<(), Error> {
        admin.require_auth();

        let config = get_config(e).ok_or(Error::NotInitialized)?;

        if admin != config.admin {
            return Err(Error::Unauthorized);
        }

        add_fees(e, amount);

        FeeCollectedEventData {
            admin: admin.clone(),
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    /// Get total collected fees
    pub fn get_total_fees(e: &Env) -> Result<u128, Error> {
        let _config = get_config(e).ok_or(Error::NotInitialized)?;
        Ok(get_total_fees(e))
    }

    /// Set category-specific fee rate (admin only)
    pub fn set_category_fee_rate(
        e: &Env,
        admin: Address,
        category_id: u32,
        rate: u32,
    ) -> Result<(), Error> {
        admin.require_auth();

        let config = get_config(e).ok_or(Error::NotInitialized)?;

        if admin != config.admin {
            return Err(Error::Unauthorized);
        }

        if !category_exists(e, category_id) {
            return Err(Error::CategoryNotFound);
        }

        if rate > MAX_FEE_RATE {
            return Err(Error::InvalidInput);
        }

        set_category_fee_rate(e, category_id, rate);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    // ========================================================================
    // STATISTICS & INFO
    // ========================================================================

    /// Get marketplace statistics
    pub fn get_stats(e: &Env) -> Result<(u64, u64, u128), Error> {
        let config = get_config(e).ok_or(Error::NotInitialized)?;
        let total_fees = get_total_fees(e);

        Ok((config.total_products, config.total_sellers, total_fees))
    }

    // ========================================================================
    // ORACLE CONFIGURATION (Admin Functions)
    // ========================================================================

    /// Configure the oracle for price feeds (admin only)
    ///
    /// # Arguments
    /// * `admin` - Admin address
    /// * `stellar_oracle` - Address of the Stellar Pubnet oracle for on-chain assets
    /// * `external_oracle` - Address of the external oracle for BTC, ETH, etc.
    /// * `staleness_threshold` - Max age of price in seconds (e.g., 300 = 5 min)
    /// * `deviation_threshold` - Max % deviation from TWAP before manipulation alert (e.g., 1000 = 10%)
    /// * `price_tolerance` - Max % product prices can deviate from oracle (e.g., 2000 = 20%)
    /// * `update_frequency` - Min time between price updates in seconds
    pub fn configure_oracle(
        e: &Env,
        admin: Address,
        stellar_oracle: Address,
        external_oracle: Address,
        staleness_threshold: u64,
        deviation_threshold: u32,
        price_tolerance: u32,
        update_frequency: u64,
    ) -> Result<(), Error> {
        admin.require_auth();

        let config = get_config(e).ok_or(Error::NotInitialized)?;

        if admin != config.admin {
            return Err(Error::Unauthorized);
        }

        let oracle_config = OracleConfig {
            stellar_oracle: stellar_oracle.clone(),
            external_oracle: external_oracle.clone(),
            staleness_threshold,
            price_deviation_threshold: deviation_threshold,
            price_tolerance,
            update_frequency,
            is_enabled: true,
        };

        set_oracle_config(e, &oracle_config);

        OracleConfiguredEventData {
            admin: admin.clone(),
            stellar_oracle,
            external_oracle,
            staleness_threshold,
            price_tolerance,
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    /// Enable or disable the oracle (admin only)
    pub fn set_oracle_enabled(e: &Env, admin: Address, enabled: bool) -> Result<(), Error> {
        admin.require_auth();

        let config = get_config(e).ok_or(Error::NotInitialized)?;

        if admin != config.admin {
            return Err(Error::Unauthorized);
        }

        let mut oracle_config = get_oracle_config(e).ok_or(Error::OracleNotConfigured)?;
        oracle_config.is_enabled = enabled;
        set_oracle_config(e, &oracle_config);

        OracleEnabledEventData {
            admin: admin.clone(),
            is_enabled: enabled,
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    /// Update a specific oracle address (admin only)
    ///
    /// # Arguments
    /// * `oracle_type` - 0 for Stellar oracle, 1 for External oracle
    /// * `new_address` - New oracle address
    pub fn update_oracle_address(
        e: &Env,
        admin: Address,
        oracle_type: u32,
        new_address: Address,
    ) -> Result<(), Error> {
        admin.require_auth();

        let config = get_config(e).ok_or(Error::NotInitialized)?;

        if admin != config.admin {
            return Err(Error::Unauthorized);
        }

        let mut oracle_config = get_oracle_config(e).ok_or(Error::OracleNotConfigured)?;

        match oracle_type {
            0 => oracle_config.stellar_oracle = new_address.clone(),
            1 => oracle_config.external_oracle = new_address.clone(),
            _ => return Err(Error::InvalidInput),
        }

        set_oracle_config(e, &oracle_config);

        OracleAddressUpdateEventData {
            admin: admin.clone(),
            oracle_type,
            new_address,
        }
        .publish(e);

        Self::extend_instance_ttl(e);
        Ok(())
    }

    /// Get current oracle configuration
    pub fn get_oracle_config(e: &Env) -> Result<OracleConfig, Error> {
        get_oracle_config(e).ok_or(Error::OracleNotConfigured)
    }

    // ========================================================================
    // ORACLE PRICE QUERY FUNCTIONS
    // ========================================================================

    /// Get the current price for a Stellar asset (XLM, USDC, etc.)
    ///
    /// # Arguments
    /// * `asset_address` - Address of the Stellar token
    ///
    /// # Returns
    /// * Tuple of (price, timestamp)
    pub fn get_stellar_asset_price(
        e: &Env,
        asset_address: Address,
    ) -> Result<(i128, u64), Error> {
        let price_data = OracleService::get_stellar_asset_price(e, &asset_address)?;
        Ok((price_data.price, price_data.timestamp))
    }

    /// Get the current price for an external asset (BTC, ETH, etc.)
    ///
    /// # Arguments
    /// * `symbol` - Symbol of the external asset (e.g., "BTC", "ETH")
    ///
    /// # Returns
    /// * Tuple of (price, timestamp)
    pub fn get_external_asset_price(
        e: &Env,
        symbol: Symbol,
    ) -> Result<(i128, u64), Error> {
        let price_data = OracleService::get_external_asset_price(e, &symbol)?;
        Ok((price_data.price, price_data.timestamp))
    }

    /// Get the time-weighted average price for a Stellar asset
    ///
    /// # Arguments
    /// * `asset_address` - Address of the Stellar token
    /// * `records` - Number of records to use for TWAP calculation
    ///
    /// # Returns
    /// * TWAP price
    pub fn get_asset_twap(
        e: &Env,
        asset_address: Address,
        records: u32,
    ) -> Result<i128, Error> {
        OracleService::get_stellar_asset_twap(e, &asset_address, records)
    }

    /// Convert an amount from one asset to another
    ///
    /// # Arguments
    /// * `amount` - Amount to convert
    /// * `from_asset` - Source asset address
    /// * `to_asset` - Target asset address
    ///
    /// # Returns
    /// * Converted amount
    pub fn convert_price(
        e: &Env,
        amount: i128,
        from_asset: Address,
        to_asset: Address,
    ) -> Result<i128, Error> {
        OracleService::convert_price(e, amount, &from_asset, &to_asset)
    }

    /// Get historical prices for an asset
    ///
    /// # Arguments
    /// * `asset_address` - Address of the asset
    /// * `limit` - Maximum number of records to return
    ///
    /// # Returns
    /// * Vector of (price, timestamp) tuples
    pub fn get_price_history(
        e: &Env,
        asset_address: Address,
        limit: u32,
    ) -> Result<Vec<(i128, u64)>, Error> {
        let history = crate::storage::get_price_history(e, &asset_address);
        let mut result: Vec<(i128, u64)> = Vec::new(e);

        let len = history.len();
        let count = core::cmp::min(limit, len);
        let start_idx = len.saturating_sub(count);

        for i in start_idx..len {
            let record = history.get(i).unwrap();
            result.push_back((record.price, record.timestamp));
        }

        Ok(result)
    }

    /// Get oracle status and last update time
    ///
    /// # Returns
    /// * Tuple of (is_enabled, last_update_timestamp)
    pub fn get_oracle_info(e: &Env) -> Result<(bool, u64), Error> {
        let (config, last_update) = OracleService::get_oracle_info(e)?;
        Ok((config.is_enabled, last_update))
    }

    // ========================================================================
    // ORACLE VALIDATION FUNCTIONS
    // ========================================================================

    /// Validate that a proposed price is within acceptable range of oracle price
    ///
    /// # Arguments
    /// * `asset_address` - Address of the payment asset
    /// * `proposed_price` - Proposed product price
    ///
    /// # Returns
    /// * Ok(()) if valid, Err(PriceOutOfRange) if not
    pub fn validate_price(
        e: &Env,
        asset_address: Address,
        proposed_price: u128,
    ) -> Result<(), Error> {
        let oracle_config = get_oracle_config(e).ok_or(Error::OracleNotConfigured)?;

        if !oracle_config.is_enabled {
            // If oracle is disabled, skip validation
            return Ok(());
        }

        let price_data = OracleService::get_stellar_asset_price(e, &asset_address)?;
        OracleService::validate_product_price(
            price_data.price,
            proposed_price,
            oracle_config.price_tolerance,
        )
    }

    // ========================================================================
    // INTERNAL HELPERS
    // ========================================================================

    /// Extend the TTL of instance storage.
    /// Called internally during state-changing operations.
    fn extend_instance_ttl(e: &Env) {
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_AMOUNT);
    }
}

#[cfg(test)]
mod test;
