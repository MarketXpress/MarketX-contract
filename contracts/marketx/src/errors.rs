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
}

    /// The escrow is not in the `Pending` (funded) state.
    ///
    /// Returned by [`release_escrow`] when called on an escrow whose status
    /// is anything other than `Pending`, preventing double-release.
    EscrowNotFunded = 3,

    /// The escrow has already been funded or is not in a fundable state.
    ///
    /// Returned by [`fund_escrow`] when called on an escrow whose status is
    /// not `Pending`, preventing a second token transfer into the same escrow.
    AlreadyFunded = 4,

    /// The fee configuration supplied to [`initialize`] is invalid.
    ///
    /// `fee_bps` must be in the range `0..=10_000` (0 % – 100 %). Values
    /// above 10 000 would result in a fee larger than the escrow amount.
    InvalidFeeConfig = 5,

    /// The caller is not authorized to perform this action.
    ///
    /// Returned by [`refund_escrow`] when the caller is neither the buyer
    /// nor the seller of the escrow.
    Unauthorized = 6,
}
