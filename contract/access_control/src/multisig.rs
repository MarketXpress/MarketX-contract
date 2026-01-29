use soroban_sdk::{Env, Address, Vec};
use crate::{storage::DataKey, errors::AccessError};

#[derive(Clone)]
pub struct Proposal {
    pub approvals: Vec<Address>,
    pub threshold: u32,
}

pub fn create_proposal(env: &Env, threshold: u32) -> u64 {
    let nonce: u64 = env.storage().instance().get(&DataKey::ProposalNonce).unwrap_or(0);
    let proposal = Proposal {
        approvals: Vec::new(env),
        threshold,
    };

    env.storage().instance().set(&DataKey::MultisigProposal(nonce), &proposal);
    env.storage().instance().set(&DataKey::ProposalNonce, &(nonce + 1));
    nonce
}

pub fn approve(env: &Env, id: u64, signer: Address) {
    let mut proposal: Proposal =
        env.storage().instance().get(&DataKey::MultisigProposal(id)).unwrap();

    if !proposal.approvals.contains(&signer) {
        proposal.approvals.push_back(signer);
    }

    env.storage().instance().set(&DataKey::MultisigProposal(id), &proposal);
}

pub fn assert_approved(env: &Env, id: u64) {
    let proposal: Proposal =
        env.storage().instance().get(&DataKey::MultisigProposal(id)).unwrap();

    if proposal.approvals.len() < proposal.threshold {
        panic_with_error!(env, AccessError::MultisigNotApproved);
    }
}
