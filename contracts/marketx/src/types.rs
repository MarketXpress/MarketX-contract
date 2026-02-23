use soroban_sdk::{contracttype, Address};

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
}