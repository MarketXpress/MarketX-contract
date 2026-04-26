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
    MaxFee,
    NativeAsset,
    NativeFeeBps,
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
    PendingFee(Address, Address),
    FeeWhitelist(Address),
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

#[contractevent(topics = ["fees_withdrawn"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeesWithdrawnEvent {
    #[topic]
    pub collector: Address,
    #[topic]
    pub token: Address,
    pub amount: i128,
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

#[contractevent(topics = ["fee_caps_changed"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeCapsChangedEvent {
    pub old_min_fee: i128,
    pub new_min_fee: i128,
    pub old_max_fee: i128,
    pub new_max_fee: i128,
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BulkEscrowRequest {
    pub seller: Address,
    pub amount: i128,
    pub metadata: Option<Bytes>,
    pub arbiter: Option<Address>,
    pub items: Option<Vec<EscrowItem>>,
}

#[contractevent(topics = ["bulk_escrow_created"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BulkEscrowCreatedEvent {
    pub buyer: Address,
    pub token: Address,
    pub escrow_ids: Vec<u64>,
}

#[contractevent(topics = ["fee_exemption"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeExemptionEvent {
    pub address: Address,
    pub exempted: bool,
    pub actor: Address,
}
