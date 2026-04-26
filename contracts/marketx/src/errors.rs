use soroban_sdk::contracterror;

/// Errors that can be returned by the MarketX contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    NotAdmin = 1,
    Unauthorized = 2,
    NotProposedAdmin = 3,
    EscrowNotFound = 10,
    InvalidEscrowState = 11,
    InvalidEscrowAmount = 13,
    ContractPaused = 31,
    EscrowIdOverflow = 40,
    InvalidFeeConfig = 50,
    MetadataTooLarge = 60,
    DuplicateEscrow = 70,
    ItemNotFound = 80,
    ItemAlreadyReleased = 81,
    TooManyItems = 82,
    ItemAmountInvalid = 83,
    EscrowNotExpired = 90,
    EscrowAlreadyFunded = 91,
    ReentrancyForbidden = 100,
}
