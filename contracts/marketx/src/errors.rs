use soroban_sdk::contracterror;

/// Errors returned by the MarketX contract.
///
/// All variants are represented as `u32` discriminants on-chain, which means
/// the numeric values are part of the public ABI — **do not renumber them**.
#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ContractError {
    /// No escrow record exists for the requested ID.
    ///
    /// Returned by [`get_escrow`], [`try_get_escrow`], [`transition_status`],
    /// [`fund_escrow`], [`release_escrow`], and [`refund_escrow`] when
    /// `escrow_id` has never been stored.
    EscrowNotFound = 1,

    /// The requested state transition is not permitted by the escrow lifecycle.
    ///
    /// Returned by [`transition_status`] when the move from the current
    /// [`EscrowStatus`] to `new_status` is not in the valid transition graph.
    /// Also returned for self-transitions (e.g. `Pending → Pending`) and any
    /// transition out of a terminal state (`Released` or `Refunded`).
    InvalidTransition = 2,

    /// The caller is not authorized to perform this action.
    Unauthorized = 3,

    /// The escrow is not in the `Pending` (funded) state.
    ///
    /// Returned by [`release_escrow`] when called on an escrow whose status
    /// is anything other than `Pending`, preventing double-release.
    EscrowNotFunded = 4,

    /// The escrow has already been funded or is not in a fundable state.
    ///
    /// Returned by [`fund_escrow`] when called on an escrow whose status is
    /// not `Pending`, preventing a second token transfer into the same escrow.
    AlreadyFunded = 5,

    /// The fee configuration supplied to [`initialize`] is invalid.
    ///
    /// `fee_bps` must be in the range `0..=10_000` (0 % – 100 %). Values
    /// above 10 000 would result in a fee larger than the escrow amount.
    InvalidFeeConfig = 6,

    /// The escrow amount must be positive.
    ///
    /// Returned by [`store_escrow`] when the amount is zero or negative.
    InvalidEscrowAmount = 7,

    /// The refund amount exceeds the escrow amount.
    ///
    /// Returned when requesting a refund larger than the original escrow amount.
    RefundAmountExceedsEscrow = 8,

    /// The return request has already been processed.
    ///
    /// Returned when attempting to modify an already processed refund request.
    RefundAlreadyProcessed = 9,

    /// Refund request not found.
    ///
    /// Returned when the refund request ID does not exist.
    RefundRequestNotFound = 10,

    /// Refund window has expired.
    ///
    /// Returned when attempting to request a refund after the allowed timeframe.
    RefundWindowExpired = 11,

    /// The caller is not the admin.
    ///
    /// Returned when admin-only functions are called by non-admin.
    NotAdmin = 12,
}
