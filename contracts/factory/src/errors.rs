use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    FactoryPaused = 4,
    TemplateNotFound = 5,
    TemplateInactive = 6,
    TemplateAlreadyExists = 7,
    InvalidWasmHash = 8,
    DeploymentFailed = 9,
    InsufficientFee = 10,
    InvalidParams = 11,
    RegistryCallFailed = 12,
    InvalidVersion = 13,
}
