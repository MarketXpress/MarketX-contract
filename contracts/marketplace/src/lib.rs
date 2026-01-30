#![no_std]

mod errors;
mod events;
mod storage;
mod types;
mod batch;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, Address, Env, String, Vec};

use crate::errors::Error;
use crate::events::*;
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
    ///
    /// # Returns
    /// * Product ID if successful
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
            // Check for category-specific fee rate
             if let Some(category) = get_category(e, cat_id) {
                 category.commission_rate
             } else {
                 config.base_fee_rate
             }
        } else {
            config.base_fee_rate
        };

        // Calculate fee: (amount * rate) / 10000
        // Use u128 to prevent overflow before division
        let fee = amount
            .checked_mul(rate as u128)
            .ok_or(Error::FeeOverflow)?
            .checked_div(10000)
            .ok_or(Error::FeeOverflow)?;

        Ok(fee)
    }

    // ========================================================================
    // BATCH OPERATIONS
    // ========================================================================

    /// Batch add products
    pub fn batch_add_product(
        e: &Env,
        seller: Address,
        products: Vec<BatchCreateProductInput>,
    ) -> Result<Vec<u64>, Error> {
        batch::batch_add_product(e, seller, products)
    }

    /// Batch create orders
    pub fn batch_create_order(
        e: &Env,
        buyer: Address,
        token: Address,
        orders: Vec<BatchCreateOrderInput>,
    ) -> Result<Vec<u64>, Error> {
        batch::batch_create_order(e, buyer, token, orders)
    }

    /// Batch update order status
    pub fn batch_update_order_status(
        e: &Env,
        caller: Address,
        updates: Vec<BatchUpdateStatusInput>,
    ) -> Result<(), Error> {
        batch::batch_update_order_status(e, caller, updates)
    }

    /// Batch release escrow
    pub fn batch_release_escrow(
        e: &Env,
        caller: Address,
        token: Address,
        order_ids: Vec<u64>,
    ) -> Result<(), Error> {
        batch::batch_release_escrow(e, caller, token, order_ids)
    }
    
    /// Batch submit rating
    pub fn batch_submit_rating(
        e: &Env,
        caller: Address,
        ratings: Vec<BatchSubmitRatingInput>,
    ) -> Result<(), Error> {
        batch::batch_submit_rating(e, caller, ratings)
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
