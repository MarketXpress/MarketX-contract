use soroban_sdk::{contracttype, Address, String, Symbol};

#[contracttype]
#[derive(Clone)]
pub enum StorageKey {
    Admin,
    Initialized,
    Config,
    Seller(Address),
    Product(u64),
    Category(u32),
    SellerProducts(Address),
    CategoryProducts(u32),
    FeesCollected,
    CategoryFeeRate(u32),
    ProductCounter,
    VerificationQueue,
    OracleConfig,
    PriceHistory(Address),
    ExternalPriceHistory(Symbol),
    LastPriceUpdate,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SellerStatus {
    Unverified = 0,
    Verified = 1,
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

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ProductStatus {
    Active = 0,
    Delisted = 1,
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Seller {
    pub address: Address,
    pub status: SellerStatus,
    pub rating: u32,
    pub total_sales: u64,
    pub total_revenue: u128,
    pub created_at: u64,
    pub metadata: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Product {
    pub id: u64,
    pub seller: Address,
    pub name: String,
    pub description: String,
    pub category_id: u32,
    pub price: u128,
    pub status: ProductStatus,
    pub stock_quantity: u64,
    pub rating: u32,
    pub purchase_count: u64,
    pub created_at: u64,
    pub metadata: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Category {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub commission_rate: u32,
    pub is_active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketplaceConfig {
    pub admin: Address,
    pub base_fee_rate: u32,
    pub is_paused: bool,
    pub total_products: u64,
    pub total_sellers: u64,
    pub updated_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionRecord {
    pub transaction_type: u32,
    pub amount: u128,
    pub timestamp: u64,
    pub product_id: u64,
}

pub const DAY_IN_LEDGERS: u32 = 17280;
pub const PERSISTENT_TTL_AMOUNT: u32 = 90 * DAY_IN_LEDGERS;
pub const PERSISTENT_TTL_THRESHOLD: u32 = PERSISTENT_TTL_AMOUNT - DAY_IN_LEDGERS;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleConfig {
    pub stellar_oracle: Address,
    pub external_oracle: Address,
    pub staleness_threshold: u64,
    pub price_deviation_threshold: u32,
    pub price_tolerance: u32,
    pub update_frequency: u64,
    pub is_enabled: bool,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PriceSource {
    Oracle = 0,
    Cached = 1,
}

impl PriceSource {
    pub fn as_u32(&self) -> u32 {
        match self {
            PriceSource::Oracle => 0,
            PriceSource::Cached => 1,
        }
    }

    pub fn from_u32(value: u32) -> Option<PriceSource> {
        match value {
            0 => Some(PriceSource::Oracle),
            1 => Some(PriceSource::Cached),
            _ => None,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceRecord {
    pub price: i128,
    pub timestamp: u64,
    pub source: PriceSource,
}

pub const MAX_PRICE_RECORDS: u32 = 100;
