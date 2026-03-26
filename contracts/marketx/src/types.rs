use soroban_sdk::{contracttype, Address, Bytes, BytesN};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    // Escrow storage
    Escrow(u64),
    EscrowIds,

    // 🔢 Escrow Counter
    EscrowCounter,

    // Fees
    FeeCollector,
    FeeBps,
    MinFee,

    // Security
    ReentrancyLock,
    Admin,
    Paused,

    // Refunds
    RefundRequest(u64),
    RefundCount,
    EscrowRefunds(u64),
    RefundHistory(u64),
    GlobalRefundHistory,

    // Initial value for testing
    InitialValue,

    // Escrow uniqueness hash (buyer + seller + metadata hash -> escrow_id)
    EscrowHash(BytesN<32>),

    // Analytics
    TotalFundedAmount,
}

/// Maximum metadata size in bytes (1 KB)
pub const MAX_METADATA_SIZE: u32 = 1024;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Escrow {
    pub buyer: Address,
    pub seller: Address,
    pub token: Address,
    pub amount: i128,
    pub status: EscrowStatus,
    pub metadata: Option<Bytes>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    Pending,
    Released,
    Refunded,
    Disputed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowCreatedEvent {
    pub escrow_id: u64,
    pub buyer: Address,
    pub seller: Address,
    pub token: Address,
    pub amount: i128,
    pub status: EscrowStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundsReleasedEvent {
    pub escrow_id: u64,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusChangeEvent {
    pub escrow_id: u64,
    pub from_status: EscrowStatus,
    pub to_status: EscrowStatus,
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
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefundHistoryEntry {
    pub refund_id: u64,
    pub escrow_id: u64,
    pub amount: i128,
    pub refunded_at: u64,
}

#[derive(Debug, Clone)]
pub enum EscrowStatus {
    Pending,
    Locked,
    Released,
    Refunded,
    PartiallyReleased, // new
}

#[derive(Debug, Clone)]
pub struct Escrow {
    pub id: String,
    pub buyer: String,
    pub seller: String,
    pub amount: u64,
    pub released_amount: u64,
    pub refunded_amount: u64,
    pub status: EscrowStatus,
}
