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

    // Refunds
    RefundAlreadyRequested = 13,
    RefundNotFound = 14,

    // Security
    ReentrancyDetected = 15,

    // 🔒 Circuit Breaker
    ContractPaused = 16,
}


pub enum ContractError {
    // Auth
    NotAdmin = 1,

    // Escrow
    EscrowNotFound = 10,
    InvalidEscrowState = 11,

    // Refunds
    RefundAlreadyRequested = 13,

    // Security
    ReentrancyDetected = 15,
    ContractPaused = 16,

    // 🔢 Counter
    EscrowIdOverflow = 17,
}
