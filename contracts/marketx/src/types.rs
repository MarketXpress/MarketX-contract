use soroban_sdk::{contracttype, Address};

/// Lifecycle states an escrow can be in.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum EscrowStatus {
    /// Funds deposited; awaiting delivery confirmation.
    Pending,
    /// Buyer confirmed delivery; funds released to seller.
    Released,
    /// Funds returned to buyer (dispute resolved in buyer's favour).
    Refunded,
    /// Dispute raised; awaiting resolution.
    Disputed,
}

/// Core escrow record stored on-chain.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Escrow {
    /// Party depositing funds.
    pub buyer: Address,
    /// Party receiving funds on release.
    pub seller: Address,
    /// Token contract address used for the escrow.
    pub token: Address,
    /// Amount locked in escrow (in the token's base unit).
    pub amount: i128,
    /// Current lifecycle state.
    pub status: EscrowStatus,
}

/// Storage key discriminants for the contract's persistent store.
#[contracttype]
pub enum DataKey {
    /// Maps escrow_id → Escrow record.
    Escrow(u64),
    /// Running count of escrows created; used for ID generation.
    EscrowCount,
}
