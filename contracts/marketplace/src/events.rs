use soroban_sdk::{contractevent, Address, String};

/// Event emitted when marketplace is initialized
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializedEventData {
    #[topic]
    pub admin: Address,
    pub base_fee_rate: u32,
}

/// Event emitted when a seller registers
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SellerRegisteredEventData {
    #[topic]
    pub seller: Address,
}

/// Event emitted when a seller is verified
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SellerVerifiedEventData {
    #[topic]
    pub seller: Address,
}

/// Event emitted when a seller is suspended
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SellerSuspendedEventData {
    #[topic]
    pub seller: Address,
}

/// Event emitted when a seller is unsuspended
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SellerUnsuspendedEventData {
    #[topic]
    pub seller: Address,
}

/// Event emitted when a category is created
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryCreatedEventData {
    #[topic]
    pub category_id: u32,
    pub name: String,
}

/// Event emitted when a product is listed
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductListedEventData {
    #[topic]
    pub seller: Address,
}

/// Event emitted when a product is updated
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductUpdatedEventData {
    #[topic]
    pub seller: Address,
}

/// Event emitted when a product is delisted
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductDelistedEventData {
    #[topic]
    pub seller: Address,
}

/// Event emitted when marketplace is paused/unpaused
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketplacePausedEventData {
    #[topic]
    pub admin: Address,
    pub is_paused: bool,
}

/// Event emitted when fee rate is updated
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeRateUpdatedEventData {
    #[topic]
    pub admin: Address,
    pub new_rate: u32,
}

/// Event emitted when fees are collected
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeCollectedEventData {
    #[topic]
    pub admin: Address,
}

/// Event emitted when seller rating is updated
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SellerRatingUpdatedEventData {
    #[topic]
    pub seller: Address,
    pub new_rating: u32,
}

/// Product quality rating submitted
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualityRatedEventData {
    #[topic]
    pub seller: Address,
}

/// Event emitted when an order is created
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderCreatedEventData {
    #[topic]
    pub buyer: Address,
    #[topic]
    pub seller: Address,
    pub order_id: u64,
}

/// Event emitted when order status is updated
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderStatusUpdatedEventData {
    #[topic]
    pub order_id: u64,
    pub new_status: u32,
}

/// Event emitted when escrow is released
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowReleasedEventData {
    #[topic]
    pub order_id: u64,
    pub amount: u128,
}
