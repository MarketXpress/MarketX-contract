use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 100,
    NotInitialized = 101,
    Unauthorized = 102,
    ContractNotFound = 103,
    ContractAlreadyRegistered = 104,
    ContractArchived = 105,
    ContractDeleted = 106,
    InvalidStatus = 107,
    InvalidAddress = 108,
    CallerNotFactory = 109,
    PermissionDenied = 110,
    InvalidPagination = 111,
}
