use soroban_sdk::contracterror;

/// Error codes for the MarketX marketplace contract.
/// Uses error codes starting at 500 to avoid conflicts with other contracts.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Contract has already been initialized
    AlreadyInitialized = 500,
    /// Contract has not been initialized
    NotInitialized = 501,
    /// Caller does not have required role or permissions
    Unauthorized = 502,
    /// Seller account not found
    SellerNotFound = 503,
    /// Product not found in marketplace
    ProductNotFound = 504,
    /// Invalid input parameters provided
    InvalidInput = 505,
    /// Insufficient balance for operation
    InsufficientBalance = 506,
    /// Seller account is not verified
    SellerNotVerified = 507,
    /// Product already exists
    ProductAlreadyExists = 508,
    /// Category not found
    CategoryNotFound = 509,
    /// Marketplace is paused for maintenance
    MarketplacePaused = 510,
    /// Product is out of stock
    OutOfStock = 511,
    /// Invalid product status
    InvalidProductStatus = 512,
    /// Invalid seller status
    InvalidSellerStatus = 513,
    /// Fee calculation overflow
    FeeOverflow = 514,
    /// Points overflow
    PointsOverflow = 515,
    /// Category already exists
    CategoryAlreadyExists = 516,
    /// Operation failed
    OperationFailed = 517,
    /// Invalid metadata provided
    InvalidMetadata = 518,
    /// Seller is suspended
    SellerSuspended = 519,
    /// Order not found
    OrderNotFound = 520,
    /// Invalid order status for operation
    InvalidOrderStatus = 521,
    /// Insufficient stock for order
    InsufficientStock = 522,
    /// Invalid payment amount
    InvalidPaymentAmount = 523,
    /// Batch operation failed
    BatchOperationFailed = 524,
}
