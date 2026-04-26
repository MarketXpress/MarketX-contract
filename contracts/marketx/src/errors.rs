use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    // AUTHENTICATION ERRORS (1-9)
    NotAdmin = 1,
    Unauthorized = 2,
    NotProposedAdmin = 3,

    // ESCROW ERRORS (10-19)
    EscrowNotFound = 10,
    InvalidEscrowState = 11,
    InvalidEscrowAmount = 13,

    // SECURITY ERRORS (30-39)
    ContractPaused = 31,

    // COUNTER ERRORS (40-49)
    EscrowIdOverflow = 40,

    // FEE ERRORS (50-59)
    InvalidFeeConfig = 50,

    // METADATA ERRORS (60-69)
    MetadataTooLarge = 60,

    // DUPLICATION ERRORS (70-79)
    DuplicateEscrow = 70,

    // ITEM ERRORS (80-89)
    ItemNotFound = 80,
    ItemAlreadyReleased = 81,
    TooManyItems = 82,
    ItemAmountInvalid = 83,

    // EXPIRY ERRORS (90-99)
    EscrowNotExpired = 90,
    EscrowAlreadyFunded = 91,

    // RENTAL ERRORS (100-109)
    /// No rental escrow found for the given ID.
    RentalNotFound = 100,
    /// Rental is not in Active state.
    RentalNotActive = 101,
    /// Rent payment is not yet due.
    PaymentNotDue = 102,
    /// Deposit has already been returned or claimed.
    DepositAlreadySettled = 103,
    /// Rental has not defaulted; deposit cannot be claimed.
    RentalNotDefaulted = 104,
    /// All scheduled payments have already been made.
    AllPaymentsMade = 105,
}
