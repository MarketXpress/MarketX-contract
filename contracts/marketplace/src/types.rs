use soroban_sdk::{contracttype, Address, String};

/// Storage keys for the MarketX contract.
#[contracttype]
#[derive(Clone)]
pub enum StorageKey {
    /// Admin address for marketplace configuration
    Admin,
    /// Initialization flag
    Initialized,
    /// Marketplace configuration
    Config,
    /// Seller data by address
    Seller(Address),
    /// Product data by ID
    Product(u64),
    /// Category data by ID
    Category(u32),
    /// Product IDs by seller address
    SellerProducts(Address),
    /// Product IDs by category
    CategoryProducts(u32),
    /// Total fees collected
    FeesCollected,
    /// Fee percentage by category
    CategoryFeeRate(u32),
    /// Last product ID counter
    ProductCounter,
    /// Seller verification queue
    VerificationQueue,
}

/// Seller verification status
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SellerStatus {
    /// Pending verification
    Unverified = 0,
    /// Seller is verified and can list products
    Verified = 1,
    /// Seller account is suspended
    Suspended = 2,
}

impl SellerStatus {
    pub fn as_u32(&self) -> u32 {
        match self {
            SellerStatus::Unverified => 0,
            SellerStatus::Verified => 1,
            SellerStatus::Suspended => 2,
        }
    }

    pub fn from_u32(value: u32) -> Option<SellerStatus> {
        match value {
            0 => Some(SellerStatus::Unverified),
            1 => Some(SellerStatus::Verified),
            2 => Some(SellerStatus::Suspended),
            _ => None,
        }
    }
}

/// Product listing status
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ProductStatus {
    /// Product is active and can be purchased
    Active = 0,
    /// Product has been delisted by seller
    Delisted = 1,
    /// Product is out of stock
    OutOfStock = 2,
}

impl ProductStatus {
    pub fn as_u32(&self) -> u32 {
        match self {
            ProductStatus::Active => 0,
            ProductStatus::Delisted => 1,
            ProductStatus::OutOfStock => 2,
        }
    }

    pub fn from_u32(value: u32) -> Option<ProductStatus> {
        match value {
            0 => Some(ProductStatus::Active),
            1 => Some(ProductStatus::Delisted),
            2 => Some(ProductStatus::OutOfStock),
            _ => None,
        }
    }
}

/// Order status
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OrderStatus {
    /// Order placed but not paid
    Pending = 0,
    /// Payment received
    Paid = 1,
    /// Seller has shipped
    Shipped = 2,
    /// Buyer received (or auto-confirmed)
    Delivered = 3,
    /// Order completed and escrow released
    Completed = 4,
    /// Order cancelled/refunded
    Cancelled = 5,
    /// Order disputed
    Disputed = 6,
}

impl OrderStatus {
    pub fn as_u32(&self) -> u32 {
        match self {
            OrderStatus::Pending => 0,
            OrderStatus::Paid => 1,
            OrderStatus::Shipped => 2,
            OrderStatus::Delivered => 3,
            OrderStatus::Completed => 4,
            OrderStatus::Cancelled => 5,
            OrderStatus::Disputed => 6,
        }
    }

    pub fn from_u32(value: u32) -> Option<OrderStatus> {
        match value {
            0 => Some(OrderStatus::Pending),
            1 => Some(OrderStatus::Paid),
            2 => Some(OrderStatus::Shipped),
            3 => Some(OrderStatus::Delivered),
            4 => Some(OrderStatus::Completed),
            5 => Some(OrderStatus::Cancelled),
            6 => Some(OrderStatus::Disputed),
            _ => None,
        }
    }
}

/// Seller information and reputation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Seller {
    /// Seller's blockchain address
    pub address: Address,
    /// Current verification status
    pub status: SellerStatus,
    /// Seller's reputation rating (1-5 stars * 100)
    pub rating: u32,
    /// Total sales count
    pub total_sales: u64,
    /// Total revenue from sales
    pub total_revenue: u128,
    /// Timestamp when seller registered
    pub created_at: u64,
    /// Optional metadata (JSON encoded)
    pub metadata: String,
}

/// Product listing information
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Product {
    /// Unique product identifier
    pub id: u64,
    /// Seller's address
    pub seller: Address,
    /// Product name
    pub name: String,
    /// Product description
    pub description: String,
    /// Category ID
    pub category_id: u32,
    /// Price in stroops
    pub price: u128,
    /// Current status
    pub status: ProductStatus,
    /// Available quantity
    pub stock_quantity: u64,
    /// Product rating (1-5 stars * 100)
    pub rating: u32,
    /// Number of purchases
    pub purchase_count: u64,
    /// Creation timestamp
    pub created_at: u64,
    /// Optional metadata (JSON encoded)
    pub metadata: String,
}

/// Order information
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Order {
    /// Unique order identifier
    pub id: u64,
    /// Buyer's address
    pub buyer: Address,
    /// Seller's address
    pub seller: Address,
    /// Product ID
    pub product_id: u64,
    /// Quantity ordered
    pub quantity: u64,
    /// Total price (quantity * unit price)
    pub total_price: u128,
    /// Current status
    pub status: OrderStatus,
    /// Timestamp created
    pub created_at: u64,
    /// Timestamp last updated
    pub updated_at: u64,
    /// Escrow balance (funds held)
    pub escrow_balance: u128,
}

/// Batch input for creating products
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchCreateProductInput {
    pub name: String,
    pub description: String,
    pub category_id: u32,
    pub price: u128,
    pub stock_quantity: u64,
    pub metadata: String,
}

/// Batch input for creating orders
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchCreateOrderInput {
    pub product_id: u64,
    pub quantity: u64,
}

/// Batch input for updating order status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchUpdateStatusInput {
    pub order_id: u64,
    pub new_status: u32,
}

/// Batch input for submitting ratings
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchSubmitRatingInput {
    pub order_id: u64,
    pub rating: u32,
    pub comment: String,
}

/// Product category
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Category {
    /// Unique category identifier
    pub id: u32,
    /// Category name
    pub name: String,
    /// Category description
    pub description: String,
    /// Commission rate in basis points (100 = 1%)
    pub commission_rate: u32,
    /// Whether category is active
    pub is_active: bool,
}

/// Marketplace configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketplaceConfig {
    /// Admin address with special privileges
    pub admin: Address,
    /// Base fee rate in basis points
    pub base_fee_rate: u32,
    /// Whether marketplace is paused
    pub is_paused: bool,
    /// Total number of products listed
    pub total_products: u64,
    /// Total number of registered sellers
    pub total_sellers: u64,
    /// Timestamp of last configuration update
    pub updated_at: u64,
}

/// Event record for tracking transactions
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionRecord {
    /// Transaction type (purchase, fee, etc.)
    pub transaction_type: u32,
    /// Amount involved
    pub amount: u128,
    /// Timestamp
    pub timestamp: u64,
    /// Associated product ID
    pub product_id: u64,
}

/// Number of ledgers in a day (assuming ~5 second block time)
pub const DAY_IN_LEDGERS: u32 = 17280;

/// TTL extension amount for persistent storage (90 days)
pub const PERSISTENT_TTL_AMOUNT: u32 = 90 * DAY_IN_LEDGERS;

/// TTL threshold for persistent storage
pub const PERSISTENT_TTL_THRESHOLD: u32 = PERSISTENT_TTL_AMOUNT - DAY_IN_LEDGERS;
