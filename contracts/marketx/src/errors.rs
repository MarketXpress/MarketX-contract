use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ContractError {
    /// No escrow exists for the given ID.
    EscrowNotFound = 1,
    /// The requested status transition is not permitted from the current state.
    InvalidTransition = 2,
    /// The caller is not authorized to perform this action.
    Unauthorized = 3,
}
