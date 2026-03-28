use soroban_sdk::contracterror;

/// Errors that can be returned by the MarketX contract.
///
/// Each error variant represents a specific failure condition that can occur
/// during contract execution. The error codes are organized by category
/// for easier maintenance and debugging.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    // =========================
    // AUTHENTICATION ERRORS (1-9)
    // =========================
    
    /// Caller is not the contract admin.
    ///
    /// This error is returned when a function requires admin privileges
    /// but the caller is not the configured admin address.
    ///
    /// **Used in:** `assert_admin()`, `set_fee_percentage()`
    NotAdmin = 1,
    
    /// Caller is not authorized to perform the requested action.
    ///
    /// This error is returned when a user attempts to perform an action
    /// they are not authorized for (e.g., non-buyer trying to refund).
    ///
    /// **Used in:** `refund_escrow()`
    Unauthorized = 2,
    NotProposedAdmin = 3,

    // =========================
    // ESCROW ERRORS (10-19)
    // =========================
    
    /// The specified escrow does not exist.
    ///
    /// This error is returned when attempting to operate on an escrow
    /// ID that was never created or has been deleted.
    ///
    /// **Used in:** `fund_escrow()`, `release_escrow()`, `release_item()`,
    ///             `refund_escrow()`, `bump_escrow()`, `resolve_dispute()`
    EscrowNotFound = 10,
    
    /// Escrow is not in the required state for the operation.
    ///
    /// This error is returned when the current escrow state does not
    /// allow the requested operation (e.g., releasing an already released escrow).
    ///
    /// **Used in:** `fund_escrow()`, `release_escrow()`, `release_item()`,
    ///             `refund_escrow()`, `resolve_dispute()`
    InvalidEscrowState = 11,
    
    /// The escrow amount is invalid.
    ///
    /// This error is returned when the escrow amount is zero or negative,
    /// or when a refund amount exceeds the escrow amount.
    ///
    /// **Used in:** `create_escrow()`, `refund_escrow()`
    InvalidEscrowAmount = 13,

    // =========================
    // SECURITY ERRORS (30-39)
    // =========================
    
    /// Contract is currently paused.
    ///
    /// This error is returned when attempting to perform operations
    /// while the contract is in a paused state.
    ///
    /// **Used in:** `assert_not_paused()`
    ContractPaused = 31,

    // =========================
    // COUNTER ERRORS (40-49)
    // =========================
    
    /// Escrow ID would overflow u64.
    ///
    /// This error is returned when the contract has already created
    /// the maximum number of escrows (2^64 - 1).
    ///
    /// **Used in:** `next_escrow_id()`, `next_refund_id()`
    EscrowIdOverflow = 40,

    // =========================
    // FEE ERRORS (50-59)
    // =========================
    
    /// Fee configuration is invalid.
    ///
    /// This error is returned when the fee configuration is malformed
    /// or missing required components (e.g., no fee collector set).
    ///
    /// **Used in:** `release_escrow()`, `set_fee_percentage()`
    InvalidFeeConfig = 50,

    // =========================
    // METADATA ERRORS (60-69)
    // =========================
    
    /// Metadata exceeds maximum allowed size.
    ///
    /// This error is returned when the provided metadata is larger
    /// than `MAX_METADATA_SIZE` (1KB).
    ///
    /// **Used in:** `validate_metadata()`
    MetadataTooLarge = 60,

    // =========================
    // DUPLICATION ERRORS (70-79)
    // =========================
    
    /// Duplicate escrow detected.
    ///
    /// This error is returned when attempting to create an escrow with
    /// the same buyer, seller, and metadata as an existing one.
    ///
    /// **Used in:** `check_duplicate_escrow()`
    DuplicateEscrow = 70,

    // =========================
    // ITEM ERRORS (80-89)
    // =========================
    
    /// Item not found in escrow.
    ///
    /// This error is returned when attempting to access an item
    /// with an invalid index in the escrow's items array.
    ///
    /// **Used in:** `release_item()`
    ItemNotFound = 80,
    
    /// Item has already been released.
    ///
    /// This error is returned when attempting to release an item
    /// that has already been released to the seller.
    ///
    /// **Used in:** `release_item()`
    ItemAlreadyReleased = 81,
    
    /// Too many items in escrow.
    ///
    /// This error is returned when attempting to create an escrow
    /// with more items than `MAX_ITEMS_PER_ESCROW` (50).
    ///
    /// **Used in:** `create_escrow()`
    TooManyItems = 82,
    
    /// Item amounts don't sum to total escrow amount.
    ///
    /// This error is returned when the sum of all item amounts
    /// does not equal the total escrow amount.
    ///
    /// **Used in:** `create_escrow()`
    ItemAmountInvalid = 83,
}
