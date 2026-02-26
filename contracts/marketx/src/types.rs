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
}
