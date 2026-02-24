use soroban_sdk::{contracttype, Address, String};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum EscrowStatus {
    Pending,
    Released,
    Refunded,
    Disputed,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum RefundStatus {
    Pending,
    Approved,
    Rejected,
    Completed,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum RefundReason {
    ChangedMind,
    NotAsDescribed,
    DamagedDefective,
    NotReceived,
    SellerFailedToDeliver,
    Other,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct RefundRequest {
    pub refund_id: u64,
    pub escrow_id: u64,
    pub buyer: Address,
    pub refund_amount: i128,
    pub reason: RefundReason,
    pub description: String,
    pub status: RefundStatus,
    pub created_at: u64,
    pub updated_at: u64,
    pub expires_at: u64,
    pub processed_by: Option<Address>,
    pub processed_at: Option<u64>,
    pub rejection_reason: Option<String>,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct RefundHistoryEntry {
    pub refund_id: u64,
    pub escrow_id: u64,
    pub amount: i128,
    pub is_full_refund: bool,
    pub processed_at: u64,
    pub processed_by: Address,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Escrow {
    pub buyer: Address,
    pub seller: Address,
    pub arbiter: Address,
    pub token: Address,
    pub amount: i128,
    pub released_amount: i128,
    pub status: EscrowStatus,
    pub refund_deadline: u64,
    pub allow_partial_refund: bool,
}

#[contracttype]
pub enum DataKey {
    Escrow(u64),
    EscrowCount,
    FeeCollector,
    FeeBps,
    MinFee,
    ReentrancyLock,
    Admin,
    RefundRequest(u64),
    RefundCount,
    EscrowRefunds(u64),
    RefundHistory(u64),
    GlobalRefundHistory,
    InitialValue,
}