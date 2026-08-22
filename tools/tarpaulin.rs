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

    // =========================
    // EXPIRY ERRORS (90-99)
    // =========================
    /// Escrow has not yet passed the unfunded expiry window.
    ///
    /// This error is returned when `cancel_unfunded` is called before
    /// the escrow has been pending long enough to be considered expired.
    ///
    /// **Used in:** `cancel_unfunded()`
    EscrowNotExpired = 90,

    /// Escrow has already been funded and cannot be cancelled as unfunded.
    ///
    /// **Used in:** `cancel_unfunded()`
    EscrowAlreadyFunded = 91,
}
use soroban_sdk::{contractevent, contracttype, Address, Bytes, BytesN, Vec};

#[cfg(test)]
use soroban_sdk::Env;

/// Returns the contract address for the native XLM token (Stellar Asset Contract).
///
/// # Example
/// ```ignore
/// use soroban_sdk::Env;
/// use crate::types::native_xlm_address;
///
/// fn example(env: &Env) {
///     let xlm_address = native_xlm_address(env);
///     // Use xlm_address for creating escrows with native XLM
/// }
/// ```
///
/// # Note
/// The native XLM token uses the Stellar Asset Contract (SAC) which implements
/// the SEP-41 Token Interface. This means it can be used interchangeably with
/// custom tokens in all escrow operations.
#[cfg(test)]
pub fn native_xlm_address(env: &Env) -> Address {
    // In test environments, register the native XLM Stellar Asset Contract
    let sac = env.register_stellar_asset_contract_v2(env.current_contract_address());
    sac.address()
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Escrow(u64),

    //  Escrow Counter
    EscrowCounter,
    FeeCollector,
    FeeBps,
    MinFee,
    ReentrancyLock,
    Admin,
    ProposedAdmin,
    Paused,
    RefundRequest(u64),
    RefundCount,
    EscrowRefunds(u64),
    RefundHistory(u64),
    GlobalRefundHistory,
    InitialValue,
    EscrowHash(BytesN<32>),
    TotalFundedAmount,

    TotalRefundedAmount,
    TotalDisputedCount,
    TotalFeesCollected,
    EscrowIds,

    TotalReleasedAmount,
}

pub const MAX_METADATA_SIZE: u32 = 1024;

/// Maximum number of items per escrow
pub const MAX_ITEMS_PER_ESCROW: u32 = 50;

/// Represents a single item/milestone within an escrow
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowItem {
    /// The amount allocated to this item
    pub amount: i128,
    /// Whether this item has been released
    pub released: bool,
    /// Optional description/metadata for this item (e.g., product ID)
    pub description: Option<Bytes>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Escrow {
    pub buyer: Address,
    pub seller: Address,
    pub token: Address,
    pub amount: i128,
    pub status: EscrowStatus,
    pub metadata: Option<Bytes>,
    pub arbiter: Option<Address>,
    /// Party that proposed mutual cancellation, if any.
    pub cancellation_proposer: Option<Address>,
    /// Individual items/milestones within this escrow
    /// If empty, the entire escrow is treated as a single item
    pub items: Vec<EscrowItem>,
    /// Ledger sequence number at which this escrow was created.
    /// Used to enforce the unfunded expiry window.
    pub created_at: u32,
}

/// Number of ledgers after creation within which an escrow must be funded.
/// After this window, anyone may call `cancel_unfunded` to remove it.
/// ~7 days at ~5s per ledger: 7 * 24 * 3600 / 5 = 120_960 ledgers.
pub const UNFUNDED_EXPIRY_LEDGERS: u32 = 120_960;

#[contractevent(topics = ["escrow_expired"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowExpiredEvent {
    #[topic]
    pub escrow_id: u64,
    pub buyer: Address,
    pub seller: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    Pending,
    Released,
    Refunded,
    Disputed,
}

#[contractevent(topics = ["escrow_created"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowCreatedEvent {
    #[topic]
    pub escrow_id: u64,
    pub buyer: Address,
    pub seller: Address,
    pub token: Address,
    pub amount: i128,
    pub status: EscrowStatus,
    pub arbiter: Option<Address>,
}

#[contractevent(topics = ["funds_released"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundsReleasedEvent {
    #[topic]
    pub escrow_id: u64,
    pub amount: i128,
    pub fee: i128,
}

#[contractevent(topics = ["fee_collected"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeCollectedEvent {
    #[topic]
    pub escrow_id: u64,
    pub fee_collector: Address,
    pub fee: i128,
}

#[contractevent(topics = ["status_change"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusChangeEvent {
    #[topic]
    pub escrow_id: u64,
    pub from_status: EscrowStatus,
    pub to_status: EscrowStatus,
    pub actor: Address,
}

#[contractevent(topics = ["cancellation_proposed"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancellationProposedEvent {
    #[topic]
    pub escrow_id: u64,
    pub actor: Address,
}

#[contractevent(topics = ["fee_changed"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeChangedEvent {
    pub old_fee_bps: u32,
    pub new_fee_bps: u32,
    pub actor: Address,
}

#[contractevent(topics = ["admin_transferred"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferredEvent {
    pub old_admin: Address,
    pub new_admin: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RefundReason {
    ProductNotReceived,
    ProductDefective,
    WrongProduct,
    ChangedMind,
    Other,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RefundStatus {
    Pending,
    Approved,
    Rejected,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefundRequest {
    pub request_id: u64,
    pub escrow_id: u64,
    pub requester: Address,
    pub amount: i128,
    pub reason: RefundReason,
    pub status: RefundStatus,
    pub created_at: u64,
    pub evidence_hash: Option<Bytes>,
    pub counter_evidence_hash: Option<Bytes>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefundHistoryEntry {
    pub refund_id: u64,
    pub escrow_id: u64,
    pub amount: i128,
    pub refunded_at: u64,
}

#[contractevent(topics = ["refund_requested"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefundRequestedEvent {
    pub request_id: u64,
    pub escrow_id: u64,
    pub requester: Address,
    pub evidence_hash: Option<Bytes>,
}

#[contractevent(topics = ["counter_evidence"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CounterEvidenceSubmittedEvent {
    pub request_id: u64,
    pub escrow_id: u64,
    pub responder: Address,
    pub counter_evidence_hash: Option<Bytes>,
}
