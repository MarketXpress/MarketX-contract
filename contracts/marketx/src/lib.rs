#![no_std]

use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, Symbol, Vec};

mod errors;
mod types;

use soroban_sdk::xdr::ToXdr;

pub use errors::ContractError;
pub use types::{
    CounterEvidenceSubmittedEvent, DataKey, Escrow, EscrowCreatedEvent, EscrowStatus,
    FundsReleasedEvent, RefundHistoryEntry, RefundReason, RefundRequest, RefundRequestedEvent,
    RefundStatus, StatusChangeEvent, MAX_METADATA_SIZE,
};

#[cfg(test)]
mod test;

#[contract]
pub struct Contract;

impl Contract {
    fn assert_admin(env: &Env) -> Result<Address, ContractError> {
        let admin = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Admin)
            .ok_or(ContractError::NotAdmin)?;

        admin.require_auth();
        Ok(admin)
    }

    fn assert_not_paused(env: &Env) -> Result<(), ContractError> {
        let paused: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Paused)
            .unwrap_or(false);

        if paused {
            Err(ContractError::ContractPaused)
        } else {
            Ok(())
        }
    }

    fn next_escrow_id(env: &Env) -> Result<u64, ContractError> {
        let current: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0);

        let next = current
            .checked_add(1)
            .ok_or(ContractError::EscrowIdOverflow)?;

        env.storage()
            .persistent()
            .set(&DataKey::EscrowCounter, &next);

        Ok(next)
    }

    fn next_refund_id(env: &Env) -> Result<u64, ContractError> {
        let current: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::RefundCount)
            .unwrap_or(0);

        let next = current
            .checked_add(1)
            .ok_or(ContractError::EscrowIdOverflow)?;

        env.storage().persistent().set(&DataKey::RefundCount, &next);

        Ok(next)
    }

    fn validate_metadata(metadata: &Option<Bytes>) -> Result<(), ContractError> {
        if let Some(ref data) = metadata {
            if data.len() > MAX_METADATA_SIZE {
                return Err(ContractError::MetadataTooLarge);
            }
        }
        Ok(())
    }

    fn generate_escrow_hash(
        env: &Env,
        buyer: &Address,
        seller: &Address,
        metadata: &Option<Bytes>,
    ) -> BytesN<32> {
        let mut bytes = Bytes::new(env);

        bytes.append(&buyer.to_xdr(env));
        bytes.append(&seller.to_xdr(env));

        if let Some(ref data) = metadata {
            bytes.append(data);
        }

        env.crypto().sha256(&bytes).into()
    }

    fn check_duplicate_escrow(
        env: &Env,
        buyer: &Address,
        seller: &Address,
        metadata: &Option<Bytes>,
    ) -> Result<(), ContractError> {
        let hash = Self::generate_escrow_hash(env, buyer, seller, metadata);

        let existing: Option<u64> = env.storage().persistent().get(&DataKey::EscrowHash(hash));

        if existing.is_some() {
            return Err(ContractError::DuplicateEscrow);
        }

        Ok(())
    }
}

#[contractimpl]
impl Contract {
    pub fn initialize(env: Env, admin: Address, fee_collector: Address, fee_bps: u32) {
        admin.require_auth();

        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::FeeCollector, &fee_collector);
        env.storage().persistent().set(&DataKey::FeeBps, &fee_bps);

        env.storage().persistent().set(&DataKey::Paused, &false);
        env.storage()
            .persistent()
            .set(&DataKey::EscrowCounter, &0u64);
        env.storage().persistent().set(&DataKey::RefundCount, &0u64);
        env.storage()
            .persistent()
            .set(&DataKey::TotalFundedAmount, &0i128);
    }

    pub fn pause(env: Env) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage().persistent().set(&DataKey::Paused, &true);
        Ok(())
    }

    pub fn unpause(env: Env) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage().persistent().set(&DataKey::Paused, &false);
        Ok(())
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    pub fn create_escrow(
        env: Env,
        buyer: Address,
        seller: Address,
        token: Address,
        amount: i128,
        metadata: Option<Bytes>,
    ) -> Result<u64, ContractError> {
        Self::assert_not_paused(&env)?;
        buyer.require_auth();

        Self::validate_metadata(&metadata)?;

        if amount <= 0 {
            return Err(ContractError::InvalidEscrowAmount);
        }

        Self::check_duplicate_escrow(&env, &buyer, &seller, &metadata)?;

        let escrow_id = Self::next_escrow_id(&env)?;

        let escrow = Escrow {
            buyer: buyer.clone(),
            seller: seller.clone(),
            token: token.clone(),
            amount,
            status: EscrowStatus::Pending,
            metadata: metadata.clone(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        let hash = Self::generate_escrow_hash(&env, &buyer, &seller, &metadata);
        env.storage()
            .persistent()
            .set(&DataKey::EscrowHash(hash), &escrow_id);

        let current_total: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalFundedAmount)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalFundedAmount, &(current_total + amount));

        let mut escrow_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowIds)
            .unwrap_or(Vec::new(&env));
        escrow_ids.push_back(escrow_id);
        env.storage()
            .persistent()
            .set(&DataKey::EscrowIds, &escrow_ids);

        let event = EscrowCreatedEvent {
            escrow_id,
            buyer,
            seller,
            token,
            amount,
            status: EscrowStatus::Pending,
        };
        env.events()
            .publish((Symbol::new(&env, "escrow_created"), escrow_id), event);

        Ok(escrow_id)
    }

    pub fn get_escrow(env: Env, escrow_id: u64) -> Option<Escrow> {
        env.storage().persistent().get(&DataKey::Escrow(escrow_id))
    }

    pub fn get_escrow_metadata(env: Env, escrow_id: u64) -> Option<Bytes> {
        let escrow: Option<Escrow> = env.storage().persistent().get(&DataKey::Escrow(escrow_id));
        escrow.and_then(|e| e.metadata)
    }

    pub fn get_total_escrows(env: Env) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0)
    }

    pub fn get_total_funded_amount(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalFundedAmount)
            .unwrap_or(0)
    }

    pub fn fund_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        escrow.buyer.require_auth();

        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &escrow.buyer,
            &env.current_contract_address(),
            &escrow.amount,
        );

        Ok(())
    }

    pub fn release_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        escrow.buyer.require_auth();

        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.seller,
            &escrow.amount,
        );

        let from_status = escrow.status.clone();
        escrow.status = EscrowStatus::Released;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        let event = FundsReleasedEvent {
            escrow_id,
            amount: escrow.amount,
        };
        env.events()
            .publish((Symbol::new(&env, "funds_released"), escrow_id), event);

        let status_event = StatusChangeEvent {
            escrow_id,
            from_status,
            to_status: EscrowStatus::Released,
        };
        env.events().publish(
            (Symbol::new(&env, "status_changed"), escrow_id),
            status_event,
        );

        Ok(())
    }

    pub fn release_partial(env: Env, _escrow_id: u64, _amount: i128) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Ok(())
    }

    pub fn refund_escrow(
        env: Env,
        escrow_id: u64,
        initiator: Address,
        amount: i128,
        reason: RefundReason,
        evidence_hash: Bytes,
    ) -> Result<u64, ContractError> {
        Self::assert_not_paused(&env)?;
        initiator.require_auth();

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if initiator != escrow.buyer {
            return Err(ContractError::Unauthorized);
        }

        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        if amount <= 0 || amount > escrow.amount {
            return Err(ContractError::InvalidEscrowAmount);
        }

        let request_id = Self::next_refund_id(&env)?;

        let refund_request = RefundRequest {
            request_id,
            escrow_id,
            requester: initiator.clone(),
            amount,
            reason,
            status: RefundStatus::Pending,
            created_at: env.ledger().timestamp(),
            evidence_hash: Some(evidence_hash.clone()),
            counter_evidence_hash: None,
        };

        env.storage()
            .persistent()
            .set(&DataKey::RefundRequest(request_id), &refund_request);

        let mut escrow_refunds: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowRefunds(escrow_id))
            .unwrap_or(Vec::new(&env));
        escrow_refunds.push_back(request_id);
        env.storage()
            .persistent()
            .set(&DataKey::EscrowRefunds(escrow_id), &escrow_refunds);

        let from_status = escrow.status.clone();
        escrow.status = EscrowStatus::Disputed;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        let event = RefundRequestedEvent {
            request_id,
            escrow_id,
            requester: initiator,
            evidence_hash: Some(evidence_hash),
        };
        env.events()
            .publish((Symbol::new(&env, "refund_requested"), request_id), event);

        let status_event = StatusChangeEvent {
            escrow_id,
            from_status,
            to_status: EscrowStatus::Disputed,
        };
        env.events().publish(
            (Symbol::new(&env, "status_changed"), escrow_id),
            status_event,
        );

        Ok(request_id)
    }

    pub fn submit_counter_evidence(
        env: Env,
        request_id: u64,
        responder: Address,
        counter_evidence_hash: Bytes,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        responder.require_auth();

        let mut refund_request: RefundRequest = env
            .storage()
            .persistent()
            .get(&DataKey::RefundRequest(request_id))
            .ok_or(ContractError::RefundRequestNotFound)?;

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(refund_request.escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if responder != escrow.seller {
            return Err(ContractError::Unauthorized);
        }

        refund_request.counter_evidence_hash = Some(counter_evidence_hash.clone());

        env.storage()
            .persistent()
            .set(&DataKey::RefundRequest(request_id), &refund_request);

        let event = CounterEvidenceSubmittedEvent {
            request_id,
            escrow_id: refund_request.escrow_id,
            responder,
            counter_evidence_hash: Some(counter_evidence_hash),
        };

        env.events().publish(
            (Symbol::new(&env, "counter_evidence_submitted"), request_id),
            event,
        );

        Ok(())
    }

    pub fn resolve_dispute(env: Env, escrow_id: u64, resolution: u32) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_admin(&env)?;

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
        }

        let from_status = escrow.status.clone();

        match resolution {
            1 => escrow.status = EscrowStatus::Released,
            2 => escrow.status = EscrowStatus::Refunded,
            _ => return Err(ContractError::InvalidDisputeResolution),
        }

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        let status_event = StatusChangeEvent {
            escrow_id,
            from_status,
            to_status: escrow.status.clone(),
        };
        env.events().publish(
            (Symbol::new(&env, "status_changed"), escrow_id),
            status_event,
        );

        Ok(())
    }

    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Admin)
    }

    pub fn set_fee_percentage(env: Env, fee_bps: u32) -> Result<(), ContractError> {
        let admin = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Admin)
            .ok_or(ContractError::NotAdmin)?;
        admin.require_auth();

        if fee_bps > 1000 {
            return Err(ContractError::InvalidFeeConfig);
        }

        env.storage().persistent().set(&DataKey::FeeBps, &fee_bps);

        env.events()
            .publish((Symbol::new(&env, "fee_changed"),), fee_bps);

        Ok(())
    }

    pub fn get_fee_bps(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::FeeBps)
            .unwrap_or(0)
    }
}
