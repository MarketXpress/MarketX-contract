use soroban_sdk::{contractevent, Address, String};

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializedEventData {
    #[topic]
    pub admin: Address,
    pub base_fee_rate: u32,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SellerRegisteredEventData {
    #[topic]
    pub seller: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SellerVerifiedEventData {
    #[topic]
    pub seller: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SellerSuspendedEventData {
    #[topic]
    pub seller: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SellerUnsuspendedEventData {
    #[topic]
    pub seller: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CategoryCreatedEventData {
    #[topic]
    pub category_id: u32,
    pub name: String,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductListedEventData {
    #[topic]
    pub seller: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductUpdatedEventData {
    #[topic]
    pub seller: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductDelistedEventData {
    #[topic]
    pub seller: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketplacePausedEventData {
    #[topic]
    pub admin: Address,
    pub is_paused: bool,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeRateUpdatedEventData {
    #[topic]
    pub admin: Address,
    pub new_rate: u32,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeCollectedEventData {
    #[topic]
    pub admin: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SellerRatingUpdatedEventData {
    #[topic]
    pub seller: Address,
    pub new_rating: u32,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualityRatedEventData {
    #[topic]
    pub seller: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleConfiguredEventData {
    #[topic]
    pub admin: Address,
    pub stellar_oracle: Address,
    pub external_oracle: Address,
    pub staleness_threshold: u64,
    pub price_tolerance: u32,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleEnabledEventData {
    #[topic]
    pub admin: Address,
    pub is_enabled: bool,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleAddressUpdateEventData {
    #[topic]
    pub admin: Address,
    pub oracle_type: u32,
    pub new_address: Address,
}
