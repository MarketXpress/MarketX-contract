use soroban_sdk::contracttype;

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
<<<<<<< HEAD

    pub enum DataKey {
    // Escrow storage
    Escrow(u64),
    EscrowIds,

    // Counters
    EscrowCounter,

    // Fees
    FeeCollector,
    FeeBps,
    MinFee,

    // Security
    Admin,
    ReentrancyLock,

    // 🔒 Circuit Breaker
    Paused,

    // Refunds
    RefundRequest(u64),
    RefundCount,
    EscrowRefunds(u64),
    RefundHistory(u64),
    GlobalRefundHistory,
=======
    InitialValue,
}

pub struct Project {
    pub id: String,
    pub owner: Address,
    pub created_at: u64,
    pub updated_at: u64,
    pub amount: u128,
}

}
    /// Vector of all escrow IDs for pagination.
    EscrowIds,
}

pub struct Project {
    pub id: String,
    pub owner: Address,
    pub created_at: u64,
    pub updated_at: u64,
    pub amount: u128,
>>>>>>> f52d546813b823710d3b5660b191d47bbfa58421
}
