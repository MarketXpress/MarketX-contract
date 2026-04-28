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
    /// Group buy deadline has not been reached yet.
    GroupBuyDeadlineNotReached = 160,
    /// Appeal window has not yet closed.
    AppealWindowNotClosed = 161,
    /// Access to escrow metadata is denied (#165).
    MetadataAccessDenied = 162,
    /// Address is zero.
    ZeroAddress = 163,
    /// A governance-controlled feature flag has disabled this operation.
    FeatureDisabled = 164,
    /// Invalid admin transfer request (for example, proposing the current admin).
    InvalidAdminTransfer = 165,
    /// Mediation phase is still open; arbiter cannot act yet (#205).
    MediationPhaseOpen = 166,
    /// No mediation phase exists for this escrow (#205).
    NoMediationPhase = 167,
    /// Mediation phase has already concluded (#205).
    MediationAlreadyConcluded = 168,
    /// The specified token is paused by the circuit breaker (#215).
    TokenPaused = 169,
}
