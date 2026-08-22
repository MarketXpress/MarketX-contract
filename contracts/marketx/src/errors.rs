use soroban_sdk::contracterror;

/// Errors that can be returned by the MarketX contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    NotAdmin = 1,
    Unauthorized = 2,
    /// Caller is not the proposed administrator.
    NotProposedAdmin = 3,
    /// Caller is not the configured oracle.
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
    /// The requested milestone does not exist.
    MilestoneNotFound = 100,
    /// The requested milestone was already completed.
    MilestoneAlreadyCompleted = 101,
    /// The escrow timelock has not been reached.
    TimeLockNotReached = 110,
    /// Timelock release is not enabled for this escrow.
    TimeLockNotEnabled = 111,
    /// The group buy has not received enough funding.
    GroupBuyNotFunded = 120,
    /// The group buy has already been funded.
    GroupBuyAlreadyFunded = 121,
    /// The group buy deadline has passed.
    GroupBuyDeadlinePassed = 122,
    /// The group buy amount is invalid.
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
    /// Migration failed: invalid source version.
    MigrationInvalidSourceVersion = 170,
    /// Migration failed: already at latest version.
    MigrationAlreadyUpToDate = 171,
    /// Migration failed: escrow not found during migration.
    MigrationEscrowNotFound = 172,
    /// Migration failed: storage error during migration.
    MigrationStorageError = 173,
    /// Migration failed: invalid migration target version.
    MigrationInvalidTargetVersion = 174,
    /// Oracle already has an unresolved pending release recorded for this escrow (#244).
    OracleReleasePending = 175,
    /// No pending oracle release exists for this escrow (#244).
    NoPendingOracleRelease = 176,
    /// The oracle challenge window has not yet elapsed (#244).
    OracleChallengeWindowOpen = 177,
    /// Arbiter address matches the escrow's buyer or seller (#243).
    ArbiterConflictOfInterest = 178,
    /// Mediation window_ledgers exceeds MAX_MEDIATION_WINDOW_LEDGERS (#256).
    InvalidMediationWindow = 179,
    /// No upgrade has been proposed, so there is nothing to execute or cancel (#242).
    NoPendingUpgrade = 180,
    /// The upgrade timelock has not yet elapsed (#242).
    UpgradeTimelockNotElapsed = 181,
}
