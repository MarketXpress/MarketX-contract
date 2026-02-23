use soroban_sdk::{contracttype, Address, Env, Vec};

/// Lifecycle states an escrow can be in.
///
/// Valid transition graph:
/// ```text
/// Pending ──► Released   (buyer confirms delivery; funds released to seller)
/// Pending ──► Disputed   (dispute raised by buyer)
/// Pending ──► Refunded   (cancelled by buyer or seller)
/// Disputed ──► Released  (resolved in seller's favour)
/// Disputed ──► Refunded  (resolved in buyer's favour)
/// Released — terminal —
/// Refunded — terminal —
/// ```
///
/// `Released` and `Refunded` are terminal states — no further transitions are
/// permitted once either is reached. All transitions require buyer authorization
/// except `Pending → Refunded` (which also accepts seller authorization) and
/// `Disputed → Released` (reserved for an external resolver role).
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum EscrowStatus {
    /// Funds deposited; awaiting delivery confirmation.
    Pending,
    /// Buyer confirmed delivery; funds released to seller minus platform fee.
    Released,
    /// Full escrow amount returned to buyer.
    Refunded,
    /// Dispute raised; awaiting resolution.
    Disputed,
}

impl EscrowStatus {
    /// Returns `true` if transitioning from `self` to `next` is permitted.
    ///
    /// This encodes the valid transition graph and is the single source of
    /// truth consulted by [`Contract::transition_status`]. Any transition not
    /// listed here — including all self-transitions and moves out of terminal
    /// states — returns `false`.
    pub fn can_transition_to(&self, next: &EscrowStatus) -> bool {
        matches!(
            (self, next),
            (EscrowStatus::Pending, EscrowStatus::Released)
                | (EscrowStatus::Pending, EscrowStatus::Disputed)
                | (EscrowStatus::Pending, EscrowStatus::Refunded)
                | (EscrowStatus::Disputed, EscrowStatus::Released)
                | (EscrowStatus::Disputed, EscrowStatus::Refunded)
        )
    }
}

/// Status of a refund request.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum RefundStatus {
    /// Refund request has been submitted and is awaiting admin approval.
    Pending,
    /// Refund has been approved and is being processed.
    Approved,
    /// Refund has been rejected by admin.
    Rejected,
    /// Refund has been completed (funds returned to buyer).
    Completed,
    /// Refund request was cancelled by the buyer.
    Cancelled,
}

/// Reasons for requesting a refund.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum RefundReason {
    /// Buyer changed their mind about the purchase.
    ChangedMind,
    /// Item was not as described or expected.
    NotAsDescribed,
    /// Item was damaged or defective.
    DamagedDefective,
    /// Item was not received.
    NotReceived,
    /// Seller failed to deliver within agreed timeframe.
    SellerFailedToDeliver,
    /// Other reason specified by buyer.
    Other,
}

/// Refund request submitted by a buyer.
#[contracttype]
#[derive(Clone, Debug)]
pub struct RefundRequest {
    /// Unique identifier for this refund request.
    pub refund_id: u64,
    /// The escrow ID this refund request is for.
    pub escrow_id: u64,
    /// Address of the buyer who submitted the refund request.
    pub buyer: Address,
    /// Amount being requested for refund (can be partial or full).
    pub refund_amount: i128,
    /// Reason for the refund request.
    pub reason: RefundReason,
    /// Additional description provided by the buyer.
    pub description: String,
    /// Current status of the refund request.
    pub status: RefundStatus,
    /// Timestamp when the refund request was submitted (ledger number).
    pub created_at: u64,
    /// Timestamp when the refund request was last updated.
    pub updated_at: u64,
    /// Timestamp when the refund window expires.
    pub expires_at: u64,
    /// Admin address that approved/rejected the request (if any).
    pub processed_by: Option<Address>,
    /// Timestamp when the request was processed.
    pub processed_at: Option<u64>,
    /// Rejection reason (if rejected).
    pub rejection_reason: Option<String>,
}

/// Refund history entry for tracking all refund events.
#[contracttype]
#[derive(Clone, Debug)]
pub struct RefundHistoryEntry {
    /// The refund request ID.
    pub refund_id: u64,
    /// The escrow ID.
    pub escrow_id: u64,
    /// Amount that was refunded.
    pub amount: i128,
    /// Whether this was a full or partial refund.
    pub is_full_refund: bool,
    /// Timestamp when the refund was processed.
    pub processed_at: u64,
    /// Admin who processed the refund.
    pub processed_by: Address,
}

/// Core escrow record stored on-chain under [`DataKey::Escrow`].
///
/// Serialized as a `map` in XDR/SCVal form. Field names become symbol keys
/// on-chain (see test snapshots for the canonical ledger representation).
#[contracttype]
#[derive(Clone, Debug)]
pub struct Escrow {
    /// Party depositing funds and initiating most state transitions.
    ///
    /// Authorizes: `fund_escrow`, `Pending → Released`, `Pending → Disputed`,
    /// `Pending → Refunded`, and `Disputed → Refunded`.
    pub buyer: Address,
    /// Party receiving funds (minus platform fee) when the escrow is released.
    ///
    /// Also authorized to initiate a refund from `Pending` state, allowing
    /// voluntary cancellation on the seller's side.
    pub seller: Address,
    /// Neutral party who can resolve disputes.
    pub arbiter: Address,
    /// Token contract address used for the escrow.
    pub token: Address,
    /// Amount locked in escrow, denominated in the token's smallest unit
    /// (e.g. stroops for XLM: 1 XLM = 10,000,000 stroops).
    pub amount: i128,
    /// Current lifecycle state. Mutated by [`Contract::fund_escrow`],
    /// [`Contract::release_escrow`], [`Contract::refund_escrow`], and
    /// [`Contract::transition_status`].
    pub status: EscrowStatus,
    /// Deadline for requesting refunds (ledger number). 0 means no deadline.
    pub refund_deadline: u64,
    /// Indicates if this escrow allows partial refunds.
    pub allow_partial_refund: bool,
}

/// Storage key discriminants for the contract's persistent store.
///
/// All keys use `Persistent` durability, meaning they survive ledger
/// expiration as long as their TTL is extended. The minimum persistent
/// entry TTL on testnet is 4,096 ledgers (~5.7 hours at 5 s/ledger).
#[contracttype]
pub enum DataKey {
    /// Maps `escrow_id: u64` → [`Escrow`] record.
    ///
    /// IDs are assigned by the caller and must be unique. There is currently
    /// no on-chain uniqueness enforcement — callers are responsible for ID
    /// management (see [`DataKey::EscrowCount`] for the intended counter).
    Escrow(u64),
    /// Running count of escrows created; intended for use as a monotonic
    /// ID generator to guarantee uniqueness.
    ///
    /// Not yet incremented automatically — reserved for a future
    /// `create_escrow` entrypoint that wraps `store_escrow`.
    #[allow(dead_code)]
    EscrowCount,
    /// Address of the platform fee collector, set during [`Contract::initialize`].
    ///
    /// On [`Contract::release_escrow`], `fee_bps / 10_000` of the escrow
    /// amount is transferred to this address before the remainder goes to
    /// the seller.
    FeeCollector,
    /// Platform fee in basis points (1 bps = 0.01 %), set during
    /// [`Contract::initialize`]. Must be in `0..=10_000`.
    ///
    /// Example: `250` = 2.5 % fee.
    FeeBps,
    /// Admin address for the contract (can approve/refund requests).
    Admin,
    /// Maps `refund_id: u64` → [`RefundRequest`] record.
    RefundRequest(u64),
    /// Running count of refund requests.
    RefundCount,
    /// Maps `escrow_id: u64` → Vec of refund request IDs for that escrow.
    EscrowRefunds(u64),
    /// Maps `escrow_id: u64` → refund history for that escrow.
    RefundHistory(u64),
    /// Contract-level refund history (list of all refund history entries).
    GlobalRefundHistory,
    /// Initial value for the contract (legacy field).
    InitialValue,
    /// Vector of all escrow IDs for pagination.
    EscrowIds,
}