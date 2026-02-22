use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ContractError {
    /// No escrow exists for the given ID.
    EscrowNotFound = 1,
    /// The requested status transition is not permitted from the current state.
    InvalidTransition = 2,
    /// Escrow is not in a funded state (Pending).
    EscrowNotFunded = 3,
}
