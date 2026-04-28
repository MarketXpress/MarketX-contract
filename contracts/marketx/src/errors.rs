use soroban_sdk::contracterror;

/// Errors that can be returned by the MarketX contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    NotAdmin = 1,
    Unauthorized = 2,
    NotProposedAdmin = 3,
    NotOracle = 4,
    EscrowNotFound = 10,
    InvalidEscrowState = 11,
    InvalidEscrowAmount = 13,
    ContractPaused = 31,
    EscrowIdOverflow = 40,
    InvalidFeeConfig = 50,
    MetadataTooLarge = 60,
    DuplicateEscrow = 70,
    ItemNotFound = 80,
    ItemAlreadyReleased = 81,
    TooManyItems = 82,
    ItemAmountInvalid = 83,
    EscrowNotExpired = 90,
    EscrowAlreadyFunded = 91,
    MilestoneNotFound = 100,
    MilestoneAlreadyCompleted = 101,
    TimeLockNotReached = 110,
    TimeLockNotEnabled = 111,
    GroupBuyNotFunded = 120,
    GroupBuyAlreadyFunded = 121,
    GroupBuyDeadlinePassed = 122,
    InvalidGroupBuyAmount = 123,

    // ── Dispute Resolution V2 (#201-204) ─────────────────────────────────────
    /// Arbiter stake is below the required minimum (#201).
    ArbiterStakeInsufficient = 130,
    /// An active stake already exists for this arbiter on this escrow (#201).
    ArbiterAlreadyStaked = 131,
    /// The caller is not the registered arbiter for this escrow (#201).
    ArbiterMismatch = 132,
    /// Evidence window has expired; no further submissions accepted (#202).
    EvidenceWindowExpired = 140,
    /// Evidence window has not yet expired; cannot force-expire (#202).
    EvidenceWindowNotExpired = 141,
    /// No evidence window is open for this escrow (#202).
    NoEvidenceWindow = 142,
    /// An appeal has already been filed for this escrow (#203).
    AppealAlreadyFiled = 150,
    /// No appeal record exists for this escrow (#203).
    AppealNotFound = 151,
    /// The appeal window has closed; appeals are no longer accepted (#203).
    AppealWindowClosed = 152,
    /// The appeal has already been resolved (#203).
    AppealAlreadyResolved = 153,
    /// A governance-controlled feature flag has disabled this operation.
    FeatureDisabled = 160,
    /// Invalid admin transfer request (for example, proposing the current admin).
    InvalidAdminTransfer = 161,
}
