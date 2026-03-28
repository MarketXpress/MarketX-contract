use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    // Auth
    NotAdmin = 1,
    Unauthorized = 2,

    // Escrow
    EscrowNotFound = 10,
    InvalidEscrowState = 11,
    InsufficientBalance = 12,
    InvalidEscrowAmount = 13,
    InvalidTransition = 14,
    RefundAmountExceedsEscrow = 15,
    RefundWindowExpired = 16,
    InvalidDisputeResolution = 17,

    // Refunds
    RefundAlreadyRequested = 20,
    RefundRequestNotFound = 21,

    // Security
    ReentrancyDetected = 30,

    // Circuit Breaker
    ContractPaused = 31,

    // Counter
    EscrowIdOverflow = 40,

    // Fee
    InvalidFeeConfig = 50,
    FeeBelowMinimum = 51,

    // Metadata
    MetadataTooLarge = 60,

    // Duplicates
    DuplicateEscrow = 70,

    // Items
    ItemNotFound = 80,
    ItemAlreadyReleased = 81,
    TooManyItems = 82,
    ItemAmountInvalid = 83,
}
