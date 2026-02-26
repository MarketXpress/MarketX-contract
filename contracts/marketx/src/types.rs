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
}
