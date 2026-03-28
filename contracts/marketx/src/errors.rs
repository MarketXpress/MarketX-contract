use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    // Auth
    NotAdmin = 1,
    Unauthorized = 2,
    NotProposedAdmin = 3,

    // Escrow
    EscrowNotFound = 10,
    InvalidEscrowState = 11,
    InsufficientBalance = 12,
    InvalidEscrowAmount = 13,
    InvalidTransition = 14,
    RefundAmountExceedsEscrow = 15,
    RefundWindowExpired = 16,

    // Refunds
    RefundAlreadyRequested = 20,
    RefundNotFound = 21,

    // Security
    ReentrancyDetected = 30,

    // 🔒 Circuit Breaker
    ContractPaused = 31,

    // 🔢 Counter
    EscrowIdOverflow = 40,

    // Fee
    InvalidFeeConfig = 50,

    // Metadata
    MetadataTooLarge = 60,

    // Duplicates
    DuplicateEscrow = 70,
}
