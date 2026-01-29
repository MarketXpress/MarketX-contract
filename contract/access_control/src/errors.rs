use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AccessError {
    Unauthorized = 1,
    ContractPaused = 2,
    RoleNotFound = 3,
    AlreadyHasRole = 4,
    MissingPermission = 5,
    MultisigNotApproved = 6,
}
