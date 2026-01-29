use soroban_sdk::{Address, Symbol};

#[derive(Clone)]
pub enum DataKey {
    Roles(Address),
    Permissions(Symbol),
    Paused,
    MultisigProposal(u64),
    ProposalNonce,
}
