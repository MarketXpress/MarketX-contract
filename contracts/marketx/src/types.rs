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
    Oracle,
    MilestoneEscrow(u64),
    TimeLockEscrow(u64),
    GroupBuyEscrow(u64),

    // ── Dispute Resolution V2 ─────────────────────────────────────────────────
    /// Stake record for an arbiter on a specific escrow (#201).
    ArbiterStake(u64),
    /// Minimum stake required to act as arbiter (#201).
    MinArbiterStake,
    /// Evidence submission window for a disputed escrow (#202).
    EvidenceWindow(u64),
    /// Appeal record for a resolved dispute (#203).
    Appeal(u64),
    /// Cumulative on-chain reputation for an arbiter address (#204).
    ArbiterReputation(Address),
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
    /// Optional shipping tracking ID for oracle verification.
    pub tracking_id: Option<Bytes>,
    /// Milestones for milestone-based payment releases
    pub milestones: Vec<Milestone>,
    /// Time-lock configuration for auto-release
    pub time_lock: Option<TimeLock>,
    /// Group buy configuration for multi-buyer escrows
    pub group_buy: Option<GroupBuy>,
}

/// Number of ledgers after creation within which an escrow must be funded.
/// After this window, anyone may call `cancel_unfunded` to remove it.
/// ~7 days at ~5s per ledger: 7 * 24 * 3600 / 5 = 120_960 ledgers.
pub const UNFUNDED_EXPIRY_LEDGERS: u32 = 120_960;

/// Milestone for milestone-based payment releases
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Milestone {
    /// Description of the milestone
    pub description: Bytes,
    /// Amount to be released upon milestone completion
    pub amount: i128,
    /// Whether this milestone has been completed
    pub completed: bool,
    /// Timestamp when milestone was completed (if completed)
    pub completed_at: Option<u64>,
}

/// Time-lock configuration for auto-release
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeLock {
    /// Ledger sequence number when funds should auto-release
    pub release_ledger: u32,
    /// Whether auto-release is enabled
    pub enabled: bool,
}

/// Group buy configuration for multi-buyer escrows
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupBuy {
    /// List of buyers and their contributions
    pub buyers: Vec<BuyerContribution>,
    /// Total amount needed
    pub target_amount: i128,
    /// Current amount funded
    pub funded_amount: i128,
    /// Deadline ledger for funding
    pub funding_deadline: u32,
}

/// Individual buyer contribution in a group buy
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuyerContribution {
    /// Buyer address
    pub buyer: Address,
    /// Amount contributed
    pub amount: i128,
    /// Whether this buyer has funded their contribution
    pub funded: bool,
}

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
    pub tracking_id: Option<Bytes>,
}

#[contractevent(topics = ["funds_released"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundsReleasedEvent {
    #[topic]
    pub escrow_id: u64,
    pub amount: i128,
    pub fee: i128,
}

#[contractevent(topics = ["delivery_verified"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryVerifiedEvent {
    #[topic]
    pub escrow_id: u64,
    pub tracking_id: Bytes,
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

#[contractevent(topics = ["milestone_completed"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneCompletedEvent {
    #[topic]
    pub escrow_id: u64,
    pub milestone_index: u32,
    pub amount: i128,
}

#[contractevent(topics = ["time_lock_released"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeLockReleasedEvent {
    #[topic]
    pub escrow_id: u64,
    pub amount: i128,
}

#[contractevent(topics = ["group_buy_funded"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupBuyFundedEvent {
    #[topic]
    pub escrow_id: u64,
    pub buyer: Address,
    pub amount: i128,
}

#[contractevent(topics = ["group_buy_completed"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupBuyCompletedEvent {
    #[topic]
    pub escrow_id: u64,
    pub total_amount: i128,
}

#[contractevent(topics = ["batch_fees_collected"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchFeesCollectedEvent {
    pub collector: Address,
    pub token: Address,
    pub total_amount: i128,
    pub escrow_count: u32,
}

// ─── Dispute Resolution V2 types ─────────────────────────────────────────────

/// Arbiter stake record for a specific escrow (#201).
///
/// An arbiter must lock `amount` tokens before being authorised to resolve
/// the dispute. Slashing deducts from `amount` and is applied by the admin
/// when an appeal overturns the arbiter's ruling.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArbiterStake {
    /// Arbiter address.
    pub arbiter: Address,
    /// Token address the stake is denominated in (same as escrow token).
    pub token: Address,
    /// Amount currently staked.
    pub amount: i128,
    /// Ledger at which the stake was placed.
    pub staked_at: u32,
    /// Whether this stake has been slashed by an appeal override.
    pub slashed: bool,
    /// Slash amount deducted (0 if not slashed).
    pub slash_amount: i128,
}

/// Evidence submission window for a disputed escrow (#202).
///
/// When a dispute is opened, an evidence window is set.  Both parties may
/// submit evidence hashes within the window.  If the window expires without
/// resolution the dispute auto-resolves in favour of the buyer (refund).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceWindow {
    pub escrow_id: u64,
    /// Ledger at which the window was opened.
    pub opened_at: u32,
    /// Ledger after which no new evidence is accepted.
    pub expires_at: u32,
    /// Whether the buyer submitted evidence.
    pub buyer_submitted: bool,
    /// Whether the seller submitted evidence.
    pub seller_submitted: bool,
    /// Hash of buyer evidence (off-chain IPFS / content hash).
    pub buyer_evidence_hash: Option<Bytes>,
    /// Hash of seller evidence.
    pub seller_evidence_hash: Option<Bytes>,
    /// Whether the window has expired and the default resolution applied.
    pub expired: bool,
}

/// Default evidence window length in ledgers (~48 hours at 5 s/ledger).
pub const DEFAULT_EVIDENCE_WINDOW_LEDGERS: u32 = 34_560;

/// Appeal record for a resolved dispute (#203).
///
/// Either party may file an appeal within `APPEAL_WINDOW_LEDGERS` after the
/// arbiter's ruling.  The admin acts as the final appellate authority.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppealRecord {
    pub escrow_id: u64,
    /// Party who filed the appeal (buyer or seller).
    pub appellant: Address,
    /// Ledger at which the appeal was filed.
    pub filed_at: u32,
    /// Whether the appeal has been resolved.
    pub resolved: bool,
    /// Final outcome: 0 = seller wins, 1 = buyer wins (None if not yet resolved).
    pub outcome: Option<u32>,
    /// Ledger at which the original arbiter ruling was made (for window check).
    pub ruling_ledger: u32,
}

/// Ledgers after a ruling within which an appeal may be filed (~24 h).
pub const APPEAL_WINDOW_LEDGERS: u32 = 17_280;

/// On-chain arbiter reputation record (#204).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArbiterReputation {
    /// Arbiter address.
    pub arbiter: Address,
    /// Total number of disputes accepted.
    pub total_disputes: u32,
    /// Number successfully resolved (not overturned on appeal).
    pub resolved_disputes: u32,
    /// Number of rulings that were appealed.
    pub appealed_rulings: u32,
    /// Number of rulings overturned on appeal.
    pub overturned_rulings: u32,
    /// Number of times the arbiter's stake was slashed.
    pub slash_count: u32,
    /// Ledger of the most recent activity.
    pub last_active: u32,
}

// ─── Dispute Resolution V2 events ────────────────────────────────────────────

#[contractevent(topics = ["arbiter_staked"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArbiterStakedEvent {
    #[topic]
    pub escrow_id: u64,
    pub arbiter: Address,
    pub amount: i128,
}

#[contractevent(topics = ["arbiter_slashed"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArbiterSlashedEvent {
    #[topic]
    pub escrow_id: u64,
    pub arbiter: Address,
    pub slash_amount: i128,
}

#[contractevent(topics = ["evidence_submitted"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceSubmittedEvent {
    #[topic]
    pub escrow_id: u64,
    pub party: Address,
    pub evidence_hash: Bytes,
}

#[contractevent(topics = ["evidence_window_expired"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceWindowExpiredEvent {
    #[topic]
    pub escrow_id: u64,
    pub default_refund: bool,
}

#[contractevent(topics = ["appeal_filed"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppealFiledEvent {
    #[topic]
    pub escrow_id: u64,
    pub appellant: Address,
}

#[contractevent(topics = ["appeal_resolved"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppealResolvedEvent {
    #[topic]
    pub escrow_id: u64,
    pub admin: Address,
    pub outcome: u32,
    pub overturned: bool,
}
