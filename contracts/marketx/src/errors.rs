use soroban_sdk::contracterror;

/// Errors that can be returned by the MarketX contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    NotAdmin = 1,
    Unauthorized = 2,
    NotProposedAdmin = 3,
    NotOracle = 4,
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
    MilestoneNotFound = 100,
    MilestoneAlreadyCompleted = 101,
    TimeLockNotReached = 110,
    TimeLockNotEnabled = 111,
    GroupBuyNotFunded = 120,
    GroupBuyAlreadyFunded = 121,
    GroupBuyDeadlinePassed = 122,
    InvalidGroupBuyAmount = 123,
}
