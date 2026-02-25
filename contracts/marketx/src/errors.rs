use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ContractError {
    EscrowNotFound = 1,
    InvalidTransition = 2,
    Unauthorized = 3,
    EscrowNotFunded = 4,
    AlreadyFunded = 5,
    InvalidFeeConfig = 6,
    InvalidEscrowAmount = 7,
    RefundAmountExceedsEscrow = 8,
    RefundAlreadyProcessed = 9,
    RefundRequestNotFound = 10,
    RefundWindowExpired = 11,
    NotAdmin = 12,
    FeeBelowMinimum = 13,
    LengthMismatch = 14,
    InvalidReleaseAmount = 15,
    ReentrancyDetected = 16,
}