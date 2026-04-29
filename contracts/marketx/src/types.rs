use soroban_sdk::{contractevent, contracttype, Address, Bytes, BytesN, Vec};

#[cfg(test)]
use soroban_sdk::Env;

/// Semantic version of this contract. Matches the `version` field in `Cargo.toml`.
pub const CONTRACT_VERSION: &str = "1.0.0";

/// On-chain version information returned by `get_version()`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

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
    TotalReleasedCount,
    TotalRefundedCount,
    TotalCancelledCount,
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
    /// Ledger at which resolved funds become claimable.
    ClaimableAt(u64),
    /// Metadata visibility setting for an escrow (#165).
    MetadataVisibility(u64),
    /// Governance feature flag: enable/disable dispute lifecycle.
    FeatureDisputesEnabled,
    /// Governance feature flag: enable/disable partial releases (`release_item`/`release_partial`).
    FeaturePartialReleasesEnabled,
    /// Mediation phase record for a disputed escrow (#205).
    MediationPhase(u64),
    /// Token-specific circuit breaker: paused tokens (#215).
    TokenPaused(Address),
    /// Schema version for state migration (#216).
    SchemaVersion,
    /// Escrow schema version for individual escrows.
    EscrowSchemaVersion(u64),
    
    // ── Dispute Resolution V2.1: Multiple Arbiters ─────────────────────────────
    /// Arbiters configuration for a specific escrow (addresses + quorum requirement).
    ArbitersConfig(u64),
    /// Minimum number of arbiters required for multi-arbiter escrows.
    MinArbitersRequired,
    /// Maximum number of arbiters allowed per escrow.
    MaxArbitersPerEscrow,
    /// Default quorum percentage (out of 100) for dispute voting.
    DefaultArbiterQuorumPercentage,
    /// Individual arbiter vote on a dispute resolution (escrow_id, arbiter_address).
    ArbiterVote(u64, Address),
    /// Voting record for a disputed escrow (tracks votes for release/refund).
    DisputeVoting(u64),
}

pub const MAX_METADATA_SIZE: u32 = 1024;

/// Maximum size for per-item and milestone description fields (bytes).
pub const MAX_DESCRIPTION_SIZE: u32 = 256;

/// Current schema version for state migration (#216).
/// Increment this when making breaking changes to stored types.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Maximum size for a shipping tracking ID (bytes).
pub const MAX_TRACKING_ID_SIZE: u32 = 128;

/// Maximum size for an evidence hash (e.g., IPFS CID) submitted during disputes (bytes).
pub const MAX_EVIDENCE_HASH_SIZE: u32 = 128;

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
    Funded,
    Released,
    Refunded,
    Disputed,
    Cancelled,
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

#[contractevent(topics = ["fee_collector_rotated"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeCollectorRotatedEvent {
    pub old_collector: Address,
    pub new_collector: Address,
    pub actor: Address,
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageRentEstimate {
    pub escrow_id: u64,
    pub entry_count: u32,
    pub estimated_bytes: u32,
    pub max_ttl: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractResourceProfile {
    pub max_items_per_escrow: u32,
    pub max_metadata_size: u32,
    pub unfunded_expiry_ledgers: u32,
    pub evidence_window_ledgers: u32,
    pub appeal_window_ledgers: u32,
    pub max_ttl: u32,
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

// ─── Global Dispute Analytics and Transparency Log ─────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalDisputeAnalytics {
    pub total_escrows: u64,
    pub released_count: u32,
    pub refunded_count: u32,
    pub disputed_count: u32,
    pub cancelled_count: u32,
    pub failure_rate_bps: u32,
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
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataVisibility {
    Private,
    Public,
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
    /// Time-lock configuration for auto-release (Vec of 0 or 1 element)
    pub time_lock: Vec<TimeLock>,
    /// Group buy configuration for multi-buyer escrows (Vec of 0 or 1 element)
    pub group_buy: Vec<GroupBuy>,
}

// ─── Issue #205: Dispute Mediation Phase ─────────────────────────────────────

/// Default mediation window length in ledgers (~48 hours at 5 s/ledger).
pub const DEFAULT_MEDIATION_WINDOW_LEDGERS: u32 = 34_560;

/// Mediation phase record for a disputed escrow (#205).
///
/// When a dispute is raised, a mediation window opens before the arbiter can
/// act. During this window both parties may propose a settlement. If they
/// agree, the escrow is resolved without arbiter involvement.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediationPhase {
    pub escrow_id: u64,
    /// Ledger at which the mediation window was opened.
    pub opened_at: u32,
    /// Ledger after which the arbiter may step in.
    pub expires_at: u32,
    /// Settlement amount proposed by the buyer (None = no proposal yet).
    pub buyer_proposal: Option<i128>,
    /// Settlement amount proposed by the seller (None = no proposal yet).
    pub seller_proposal: Option<i128>,
    /// Whether the mediation phase has concluded (settled or expired).
    pub concluded: bool,
}

#[contractevent(topics = ["mediation_opened"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediationOpenedEvent {
    #[topic]
    pub escrow_id: u64,
    pub expires_at: u32,
}

#[contractevent(topics = ["mediation_proposed"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediationProposedEvent {
    #[topic]
    pub escrow_id: u64,
    pub proposer: Address,
    pub amount: i128,
}

#[contractevent(topics = ["mediation_settled"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediationSettledEvent {
    #[topic]
    pub escrow_id: u64,
    pub seller_amount: i128,
    pub buyer_refund: i128,
}

// ─── Issue #215: Token-Specific Circuit Breaker ───────────────────────────────

#[contractevent(topics = ["token_circuit_breaker"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenCircuitBreakerEvent {
    pub token: Address,
    pub paused: bool,
    pub actor: Address,
}

// ─── Dispute Resolution V2.1: Multiple Arbiters ──────────────────────────────
//
// When a dispute is raised and the escrow has multiple arbiters, all arbiters
// must reach consensus (via voting) before resolving. This eliminates the single
// point of failure from a lone arbiter.

/// Represents configuration for multiple arbiters on a single escrow.
/// Both parties or the creator can designate multiple arbiters to act as the
/// final authority in a dispute.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArbitersConfig {
    /// The escrow ID this config is for.
    pub escrow_id: u64,
    /// List of arbiter addresses authorized to vote.
    pub arbiters: Vec<Address>,
    /// Number of arbiters required to reach consensus (voting quorum).
    /// E.g., if there are 3 arbiters and quorum is 2, any 2 arbiters must
    /// agree on the resolution.
    pub quorum_required: u32,
    /// Ledger at which this config was created.
    pub created_at: u32,
}

/// Individual arbiter vote on how to resolve a dispute.
/// A vote of 0 means "Release to seller", 1 means "Refund to buyer".
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArbiterVoteRecord {
    /// The escrow ID being voted on.
    pub escrow_id: u64,
    /// The arbiter who cast this vote.
    pub arbiter: Address,
    /// Vote: 0 = release to seller, 1 = refund to buyer.
    pub vote: u32,
    /// Ledger at which the vote was cast.
    pub voted_at: u32,
}

/// Aggregated voting record for a disputed escrow.
/// Tracks consensus votes from all arbiters.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeVotingRecord {
    /// The escrow ID being disputed.
    pub escrow_id: u64,
    /// Number of votes for releasing to seller (vote = 0).
    pub votes_for_release: u32,
    /// Number of votes for refunding to buyer (vote = 1).
    pub votes_for_refund: u32,
    /// Total number of arbiters (from config).
    pub total_arbiters: u32,
    /// Quorum required to resolve (from config).
    pub quorum_required: u32,
    /// Ledger at which voting started.
    pub voting_opened_at: u32,
    /// If consensus has been reached, this is the final resolution (Some(0 or 1)).
    /// None if voting is still ongoing.
    pub consensus_resolution: Option<u32>,
    /// Ledger at which consensus was reached (if reached).
    pub consensus_at: Option<u32>,
}

/// Default minimum number of arbiters for multi-arbiter escrows.
pub const DEFAULT_MIN_ARBITERS_REQUIRED: u32 = 2;

/// Default maximum number of arbiters allowed per escrow to prevent excessive voting.
pub const DEFAULT_MAX_ARBITERS_PER_ESCROW: u32 = 7;

/// Default quorum percentage for arbiters (e.g., 51 means majority).
/// This is multiplied by 100 (so 5100 = 51%).
pub const DEFAULT_ARBITER_QUORUM_PERCENTAGE: u32 = 5100; // 51%

#[contractevent(topics = ["arbiters_configured"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArbitersConfiguredEvent {
    #[topic]
    pub escrow_id: u64,
    pub arbiters_count: u32,
    pub quorum_required: u32,
    pub created_by: Address,
}

#[contractevent(topics = ["arbiter_vote_cast"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArbiterVoteCastEvent {
    #[topic]
    pub escrow_id: u64,
    pub arbiter: Address,
    pub vote: u32, // 0 = release, 1 = refund
}

#[contractevent(topics = ["dispute_consensus_reached"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeConsensusReachedEvent {
    #[topic]
    pub escrow_id: u64,
    pub resolution: u32, // 0 = release to seller, 1 = refund to buyer
    pub votes_for_release: u32,
    pub votes_for_refund: u32,
}
