//! Comprehensive Event Logging System for MarketX Marketplace
//!
//! This module provides standardized event structures and emission functions
//! for all marketplace contract interactions including:
//! - Product listing/delisting events
//! - Payment processing events
//! - Escrow state change events
//! - User action logging events
//! - Error event emissions

use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol, contracterror};

// ============================================================================
// Event Topic Constants
// ============================================================================

/// Order-related event topics
pub const TOPIC_ORDER_CREATED: Symbol = symbol_short!("ORD_CRT");
pub const TOPIC_ORDER_CANCELLED: Symbol = symbol_short!("ORD_CAN");
pub const TOPIC_ORDER_SHIPPED: Symbol = symbol_short!("ORD_SHP");
pub const TOPIC_ORDER_DELIVERED: Symbol = symbol_short!("ORD_DLV");
pub const TOPIC_ORDER_DISPUTED: Symbol = symbol_short!("ORD_DSP");
pub const TOPIC_ORDER_RESOLVED: Symbol = symbol_short!("ORD_RSV");

/// Escrow-related event topics
pub const TOPIC_ESCROW_LOCKED: Symbol = symbol_short!("ESC_LCK");
pub const TOPIC_ESCROW_RELEASED: Symbol = symbol_short!("ESC_REL");
pub const TOPIC_ESCROW_REFUNDED: Symbol = symbol_short!("ESC_RFD");

/// Payment-related event topics
pub const TOPIC_PAYMENT_RECEIVED: Symbol = symbol_short!("PAY_RCV");
pub const TOPIC_PAYMENT_PROCESSED: Symbol = symbol_short!("PAY_PRC");
pub const TOPIC_PAYMENT_FAILED: Symbol = symbol_short!("PAY_FLD");

/// Product listing event topics
pub const TOPIC_PRODUCT_LISTED: Symbol = symbol_short!("PRD_LST");
pub const TOPIC_PRODUCT_DELISTED: Symbol = symbol_short!("PRD_DLS");
pub const TOPIC_PRODUCT_UPDATED: Symbol = symbol_short!("PRD_UPD");

/// User action event topics
pub const TOPIC_USER_ACTION: Symbol = symbol_short!("USR_ACT");

/// Error event topics
pub const TOPIC_ERROR: Symbol = symbol_short!("ERROR");

// ============================================================================
// Error Codes
// ============================================================================

/// Standard error codes for marketplace operations
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MarketplaceError {
    Unauthorized = 1,
    InvalidState = 2,
    OrderNotFound = 3,
    InsufficientFunds = 4,
    ProductNotFound = 5,
    InvalidAmount = 6,
    EscrowError = 7,
    PaymentFailed = 8,
    AlreadyExists = 9,
    Timeout = 10,
}

// ============================================================================
// Event Data Structures
// ============================================================================

/// Event data for order creation
#[contracttype]
#[derive(Clone, Debug)]
pub struct OrderCreatedEvent {
    pub order_id: u64,
    pub buyer: Address,
    pub seller: Address,
    pub asset: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Event data for order cancellation
#[contracttype]
#[derive(Clone, Debug)]
pub struct OrderCancelledEvent {
    pub order_id: u64,
    pub cancelled_by: Address,
    pub reason: OrderCancelReason,
    pub timestamp: u64,
}

/// Reasons for order cancellation
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum OrderCancelReason {
    BuyerRequested,
    SellerRequested,
    Timeout,
    AdminAction,
}

/// Event data for order shipment
#[contracttype]
#[derive(Clone, Debug)]
pub struct OrderShippedEvent {
    pub order_id: u64,
    pub seller: Address,
    pub shipping_ref: String,
    pub timestamp: u64,
}

/// Event data for order delivery confirmation
#[contracttype]
#[derive(Clone, Debug)]
pub struct OrderDeliveredEvent {
    pub order_id: u64,
    pub buyer: Address,
    pub seller: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Event data for order disputes
#[contracttype]
#[derive(Clone, Debug)]
pub struct OrderDisputedEvent {
    pub order_id: u64,
    pub disputed_by: Address,
    pub timestamp: u64,
}

/// Event data for dispute resolution
#[contracttype]
#[derive(Clone, Debug)]
pub struct DisputeResolvedEvent {
    pub order_id: u64,
    pub resolved_by: Address,
    pub resolution: DisputeResolution,
    pub refunded: bool,
    pub timestamp: u64,
}

/// Dispute resolution outcomes
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DisputeResolution {
    RefundBuyer,
    ReleaseSeller,
    PartialRefund,
}

/// Event data for escrow fund locking
#[contracttype]
#[derive(Clone, Debug)]
pub struct EscrowLockedEvent {
    pub order_id: u64,
    pub buyer: Address,
    pub seller: Address,
    pub asset: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Event data for escrow fund release
#[contracttype]
#[derive(Clone, Debug)]
pub struct EscrowReleasedEvent {
    pub order_id: u64,
    pub recipient: Address,
    pub asset: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Event data for escrow refund
#[contracttype]
#[derive(Clone, Debug)]
pub struct EscrowRefundedEvent {
    pub order_id: u64,
    pub recipient: Address,
    pub asset: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Event data for payment processing
#[contracttype]
#[derive(Clone, Debug)]
pub struct PaymentEvent {
    pub order_id: u64,
    pub payer: Address,
    pub payee: Address,
    pub asset: Address,
    pub amount: i128,
    pub status: PaymentStatus,
    pub timestamp: u64,
}

/// Payment status types
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum PaymentStatus {
    Pending,
    Received,
    Processed,
    Failed,
    Refunded,
}

/// Event data for product listing
#[contracttype]
#[derive(Clone, Debug)]
pub struct ProductListedEvent {
    pub product_id: u64,
    pub seller: Address,
    pub price: i128,
    pub asset: Address,
    pub timestamp: u64,
}

/// Event data for product delisting
#[contracttype]
#[derive(Clone, Debug)]
pub struct ProductDelistedEvent {
    pub product_id: u64,
    pub seller: Address,
    pub reason: DelistReason,
    pub timestamp: u64,
}

/// Reasons for product delisting
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DelistReason {
    SellerRequested,
    OutOfStock,
    PolicyViolation,
    AdminAction,
}

/// Event data for user actions
#[contracttype]
#[derive(Clone, Debug)]
pub struct UserActionEvent {
    pub user: Address,
    pub action: UserAction,
    pub target_id: u64,
    pub timestamp: u64,
}

/// Types of user actions
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum UserAction {
    CreateOrder,
    CancelOrder,
    ShipOrder,
    ConfirmDelivery,
    RaiseDispute,
    ListProduct,
    DelistProduct,
    UpdateProduct,
}

/// Event data for errors
#[contracttype]
#[derive(Clone, Debug)]
pub struct ErrorEvent {
    pub error_code: u32,
    pub user: Address,
    pub operation: String,
    pub timestamp: u64,
}

// ============================================================================
// Event Filtering Helpers
// ============================================================================

/// Event filter configuration for querying events
#[contracttype]
#[derive(Clone, Debug)]
pub struct EventFilter {
    pub topic: Option<Symbol>,
    pub order_id: Option<u64>,
    pub user: Option<Address>,
    pub start_timestamp: Option<u64>,
    pub end_timestamp: Option<u64>,
}

impl EventFilter {
    /// Create a new empty filter
    pub fn new() -> Self {
        EventFilter {
            topic: None,
            order_id: None,
            user: None,
            start_timestamp: None,
            end_timestamp: None,
        }
    }
    
    /// Filter by event topic
    pub fn with_topic(mut self, topic: Symbol) -> Self {
        self.topic = Some(topic);
        self
    }
    
    /// Filter by order ID
    pub fn with_order_id(mut self, order_id: u64) -> Self {
        self.order_id = Some(order_id);
        self
    }
    
    /// Filter by user address
    pub fn with_user(mut self, user: Address) -> Self {
        self.user = Some(user);
        self
    }
    
    /// Filter by timestamp range
    pub fn with_time_range(mut self, start: u64, end: u64) -> Self {
        self.start_timestamp = Some(start);
        self.end_timestamp = Some(end);
        self
    }
}

impl Default for EventFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Event Emission Functions - Orders
// ============================================================================

/// Emit event when a new order is created
#[allow(deprecated)]
pub fn emit_order_created(
    env: &Env,
    order_id: u64,
    buyer: Address,
    seller: Address,
    asset: Address,
    amount: i128,
) {
    let timestamp = env.ledger().timestamp();
    let event = OrderCreatedEvent {
        order_id,
        buyer: buyer.clone(),
        seller: seller.clone(),
        asset,
        amount,
        timestamp,
    };
    
    env.events().publish(
        (TOPIC_ORDER_CREATED, order_id, buyer, seller),
        event,
    );
}

/// Emit event when an order is cancelled
#[allow(deprecated)]
pub fn emit_order_cancelled(
    env: &Env,
    order_id: u64,
    cancelled_by: Address,
    reason: OrderCancelReason,
) {
    let timestamp = env.ledger().timestamp();
    let event = OrderCancelledEvent {
        order_id,
        cancelled_by: cancelled_by.clone(),
        reason,
        timestamp,
    };
    
    env.events().publish(
        (TOPIC_ORDER_CANCELLED, order_id, cancelled_by),
        event,
    );
}

/// Emit event when an order is shipped
#[allow(deprecated)]
pub fn emit_order_shipped(
    env: &Env,
    order_id: u64,
    seller: Address,
    shipping_ref: String,
) {
    let timestamp = env.ledger().timestamp();
    let event = OrderShippedEvent {
        order_id,
        seller: seller.clone(),
        shipping_ref,
        timestamp,
    };
    
    env.events().publish(
        (TOPIC_ORDER_SHIPPED, order_id, seller),
        event,
    );
}

/// Emit event when an order is delivered
#[allow(deprecated)]
pub fn emit_order_delivered(
    env: &Env,
    order_id: u64,
    buyer: Address,
    seller: Address,
    amount: i128,
) {
    let timestamp = env.ledger().timestamp();
    let event = OrderDeliveredEvent {
        order_id,
        buyer: buyer.clone(),
        seller: seller.clone(),
        amount,
        timestamp,
    };
    
    env.events().publish(
        (TOPIC_ORDER_DELIVERED, order_id, buyer, seller),
        event,
    );
}

/// Emit event when an order is disputed
#[allow(deprecated)]
pub fn emit_order_disputed(
    env: &Env,
    order_id: u64,
    disputed_by: Address,
) {
    let timestamp = env.ledger().timestamp();
    let event = OrderDisputedEvent {
        order_id,
        disputed_by: disputed_by.clone(),
        timestamp,
    };
    
    env.events().publish(
        (TOPIC_ORDER_DISPUTED, order_id, disputed_by),
        event,
    );
}

/// Emit event when a dispute is resolved
#[allow(deprecated)]
pub fn emit_dispute_resolved(
    env: &Env,
    order_id: u64,
    resolved_by: Address,
    refunded: bool,
) {
    let timestamp = env.ledger().timestamp();
    let resolution = if refunded {
        DisputeResolution::RefundBuyer
    } else {
        DisputeResolution::ReleaseSeller
    };
    
    let event = DisputeResolvedEvent {
        order_id,
        resolved_by: resolved_by.clone(),
        resolution,
        refunded,
        timestamp,
    };
    
    env.events().publish(
        (TOPIC_ORDER_RESOLVED, order_id, resolved_by),
        event,
    );
}

// ============================================================================
// Event Emission Functions - Escrow
// ============================================================================

/// Emit event when funds are locked in escrow
#[allow(deprecated)]
pub fn emit_escrow_locked(
    env: &Env,
    order_id: u64,
    buyer: Address,
    seller: Address,
    asset: Address,
    amount: i128,
) {
    let timestamp = env.ledger().timestamp();
    let event = EscrowLockedEvent {
        order_id,
        buyer: buyer.clone(),
        seller: seller.clone(),
        asset,
        amount,
        timestamp,
    };
    
    env.events().publish(
        (TOPIC_ESCROW_LOCKED, order_id, buyer, seller),
        event,
    );
}

/// Emit event when escrow funds are released to seller
#[allow(deprecated)]
pub fn emit_escrow_released(
    env: &Env,
    order_id: u64,
    recipient: Address,
    asset: Address,
    amount: i128,
) {
    let timestamp = env.ledger().timestamp();
    let event = EscrowReleasedEvent {
        order_id,
        recipient: recipient.clone(),
        asset,
        amount,
        timestamp,
    };
    
    env.events().publish(
        (TOPIC_ESCROW_RELEASED, order_id, recipient),
        event,
    );
}

/// Emit event when escrow funds are refunded to buyer
#[allow(deprecated)]
pub fn emit_escrow_refunded(
    env: &Env,
    order_id: u64,
    recipient: Address,
    asset: Address,
    amount: i128,
) {
    let timestamp = env.ledger().timestamp();
    let event = EscrowRefundedEvent {
        order_id,
        recipient: recipient.clone(),
        asset,
        amount,
        timestamp,
    };
    
    env.events().publish(
        (TOPIC_ESCROW_REFUNDED, order_id, recipient),
        event,
    );
}

// ============================================================================
// Event Emission Functions - Payments
// ============================================================================

/// Emit event for payment status changes
#[allow(deprecated)]
pub fn emit_payment_event(
    env: &Env,
    order_id: u64,
    payer: Address,
    payee: Address,
    asset: Address,
    amount: i128,
    status: PaymentStatus,
) {
    let timestamp = env.ledger().timestamp();
    let event = PaymentEvent {
        order_id,
        payer: payer.clone(),
        payee: payee.clone(),
        asset,
        amount,
        status,
        timestamp,
    };
    
    let topic = match &event.status {
        PaymentStatus::Received => TOPIC_PAYMENT_RECEIVED,
        PaymentStatus::Processed => TOPIC_PAYMENT_PROCESSED,
        PaymentStatus::Failed => TOPIC_PAYMENT_FAILED,
        _ => TOPIC_PAYMENT_RECEIVED,
    };
    
    env.events().publish(
        (topic, order_id, payer, payee),
        event,
    );
}

// ============================================================================
// Event Emission Functions - Products
// ============================================================================

/// Emit event when a product is listed
#[allow(deprecated)]
pub fn emit_product_listed(
    env: &Env,
    product_id: u64,
    seller: Address,
    price: i128,
    asset: Address,
) {
    let timestamp = env.ledger().timestamp();
    let event = ProductListedEvent {
        product_id,
        seller: seller.clone(),
        price,
        asset,
        timestamp,
    };
    
    env.events().publish(
        (TOPIC_PRODUCT_LISTED, product_id, seller),
        event,
    );
}

/// Emit event when a product is delisted
#[allow(deprecated)]
pub fn emit_product_delisted(
    env: &Env,
    product_id: u64,
    seller: Address,
    reason: DelistReason,
) {
    let timestamp = env.ledger().timestamp();
    let event = ProductDelistedEvent {
        product_id,
        seller: seller.clone(),
        reason,
        timestamp,
    };
    
    env.events().publish(
        (TOPIC_PRODUCT_DELISTED, product_id, seller),
        event,
    );
}

// ============================================================================
// Event Emission Functions - User Actions
// ============================================================================

/// Emit generic user action event
#[allow(deprecated)]
pub fn emit_user_action(
    env: &Env,
    user: Address,
    action: UserAction,
    target_id: u64,
) {
    let timestamp = env.ledger().timestamp();
    let event = UserActionEvent {
        user: user.clone(),
        action,
        target_id,
        timestamp,
    };
    
    env.events().publish(
        (TOPIC_USER_ACTION, user, target_id),
        event,
    );
}

// ============================================================================
// Event Emission Functions - Errors
// ============================================================================

/// Emit error event for failed operations
#[allow(deprecated)]
pub fn emit_error(
    env: &Env,
    error_code: u32,
    user: Address,
    operation: String,
) {
    let timestamp = env.ledger().timestamp();
    let event = ErrorEvent {
        error_code,
        user: user.clone(),
        operation,
        timestamp,
    };
    
    env.events().publish(
        (TOPIC_ERROR, error_code, user),
        event,
    );
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    // Note: Event emission tests require a contract context in Soroban.
    // These tests validate the event data structures and filter builders.
    // Full event emission testing is done in the contract integration tests.

    #[test]
    fn test_event_filter_builder() {
        let env = Env::default();
        let user = Address::generate(&env);
        
        let filter = EventFilter::new()
            .with_topic(TOPIC_ORDER_CREATED)
            .with_order_id(1)
            .with_user(user.clone())
            .with_time_range(1000, 2000);
        
        assert_eq!(filter.topic, Some(TOPIC_ORDER_CREATED));
        assert_eq!(filter.order_id, Some(1));
        assert_eq!(filter.user, Some(user));
        assert_eq!(filter.start_timestamp, Some(1000));
        assert_eq!(filter.end_timestamp, Some(2000));
    }

    #[test]
    fn test_event_filter_default() {
        let filter = EventFilter::default();
        
        assert_eq!(filter.topic, None);
        assert_eq!(filter.order_id, None);
        assert_eq!(filter.user, None);
        assert_eq!(filter.start_timestamp, None);
        assert_eq!(filter.end_timestamp, None);
    }

    #[test]
    fn test_order_cancel_reason_variants() {
        // Verify all variants can be created
        let _buyer = OrderCancelReason::BuyerRequested;
        let _seller = OrderCancelReason::SellerRequested;
        let _timeout = OrderCancelReason::Timeout;
        let _admin = OrderCancelReason::AdminAction;
        
        assert_eq!(OrderCancelReason::BuyerRequested, OrderCancelReason::BuyerRequested);
        assert_ne!(OrderCancelReason::BuyerRequested, OrderCancelReason::SellerRequested);
    }

    #[test]
    fn test_dispute_resolution_variants() {
        let _refund = DisputeResolution::RefundBuyer;
        let _release = DisputeResolution::ReleaseSeller;
        let _partial = DisputeResolution::PartialRefund;
        
        assert_eq!(DisputeResolution::RefundBuyer, DisputeResolution::RefundBuyer);
        assert_ne!(DisputeResolution::RefundBuyer, DisputeResolution::ReleaseSeller);
    }

    #[test]
    fn test_payment_status_variants() {
        let _pending = PaymentStatus::Pending;
        let _received = PaymentStatus::Received;
        let _processed = PaymentStatus::Processed;
        let _failed = PaymentStatus::Failed;
        let _refunded = PaymentStatus::Refunded;
        
        assert_eq!(PaymentStatus::Pending, PaymentStatus::Pending);
        assert_ne!(PaymentStatus::Pending, PaymentStatus::Processed);
    }

    #[test]
    fn test_delist_reason_variants() {
        let _seller = DelistReason::SellerRequested;
        let _stock = DelistReason::OutOfStock;
        let _policy = DelistReason::PolicyViolation;
        let _admin = DelistReason::AdminAction;
        
        assert_eq!(DelistReason::SellerRequested, DelistReason::SellerRequested);
        assert_ne!(DelistReason::SellerRequested, DelistReason::OutOfStock);
    }

    #[test]
    fn test_user_action_variants() {
        let _create = UserAction::CreateOrder;
        let _cancel = UserAction::CancelOrder;
        let _ship = UserAction::ShipOrder;
        let _deliver = UserAction::ConfirmDelivery;
        let _dispute = UserAction::RaiseDispute;
        let _list = UserAction::ListProduct;
        let _delist = UserAction::DelistProduct;
        let _update = UserAction::UpdateProduct;
        
        assert_eq!(UserAction::CreateOrder, UserAction::CreateOrder);
        assert_ne!(UserAction::CreateOrder, UserAction::CancelOrder);
    }

    #[test]
    fn test_marketplace_error_codes() {
        assert_eq!(MarketplaceError::Unauthorized as u32, 1);
        assert_eq!(MarketplaceError::InvalidState as u32, 2);
        assert_eq!(MarketplaceError::OrderNotFound as u32, 3);
        assert_eq!(MarketplaceError::InsufficientFunds as u32, 4);
        assert_eq!(MarketplaceError::ProductNotFound as u32, 5);
        assert_eq!(MarketplaceError::InvalidAmount as u32, 6);
        assert_eq!(MarketplaceError::EscrowError as u32, 7);
        assert_eq!(MarketplaceError::PaymentFailed as u32, 8);
        assert_eq!(MarketplaceError::AlreadyExists as u32, 9);
        assert_eq!(MarketplaceError::Timeout as u32, 10);
    }

    #[test]
    fn test_event_struct_creation() {
        let env = Env::default();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        
        // Test OrderCreatedEvent structure
        let order_created = OrderCreatedEvent {
            order_id: 1,
            buyer: buyer.clone(),
            seller: seller.clone(),
            asset: asset.clone(),
            amount: 1000,
            timestamp: 12345,
        };
        assert_eq!(order_created.order_id, 1);
        assert_eq!(order_created.amount, 1000);
        assert_eq!(order_created.timestamp, 12345);
        
        // Test OrderCancelledEvent structure
        let order_cancelled = OrderCancelledEvent {
            order_id: 1,
            cancelled_by: buyer.clone(),
            reason: OrderCancelReason::BuyerRequested,
            timestamp: 12345,
        };
        assert_eq!(order_cancelled.order_id, 1);
        assert_eq!(order_cancelled.reason, OrderCancelReason::BuyerRequested);
        
        // Test EscrowLockedEvent structure
        let escrow_locked = EscrowLockedEvent {
            order_id: 1,
            buyer: buyer.clone(),
            seller: seller.clone(),
            asset: asset.clone(),
            amount: 1000,
            timestamp: 12345,
        };
        assert_eq!(escrow_locked.order_id, 1);
        assert_eq!(escrow_locked.amount, 1000);
    }

    #[test]
    fn test_topic_constants() {
        // Verify topic constants are properly defined
        assert_eq!(TOPIC_ORDER_CREATED, symbol_short!("ORD_CRT"));
        assert_eq!(TOPIC_ORDER_CANCELLED, symbol_short!("ORD_CAN"));
        assert_eq!(TOPIC_ORDER_SHIPPED, symbol_short!("ORD_SHP"));
        assert_eq!(TOPIC_ORDER_DELIVERED, symbol_short!("ORD_DLV"));
        assert_eq!(TOPIC_ORDER_DISPUTED, symbol_short!("ORD_DSP"));
        assert_eq!(TOPIC_ORDER_RESOLVED, symbol_short!("ORD_RSV"));
        assert_eq!(TOPIC_ESCROW_LOCKED, symbol_short!("ESC_LCK"));
        assert_eq!(TOPIC_ESCROW_RELEASED, symbol_short!("ESC_REL"));
        assert_eq!(TOPIC_ESCROW_REFUNDED, symbol_short!("ESC_RFD"));
        assert_eq!(TOPIC_PAYMENT_RECEIVED, symbol_short!("PAY_RCV"));
        assert_eq!(TOPIC_PAYMENT_PROCESSED, symbol_short!("PAY_PRC"));
        assert_eq!(TOPIC_PAYMENT_FAILED, symbol_short!("PAY_FLD"));
        assert_eq!(TOPIC_PRODUCT_LISTED, symbol_short!("PRD_LST"));
        assert_eq!(TOPIC_PRODUCT_DELISTED, symbol_short!("PRD_DLS"));
        assert_eq!(TOPIC_PRODUCT_UPDATED, symbol_short!("PRD_UPD"));
        assert_eq!(TOPIC_USER_ACTION, symbol_short!("USR_ACT"));
        assert_eq!(TOPIC_ERROR, symbol_short!("ERROR"));
    }
}
