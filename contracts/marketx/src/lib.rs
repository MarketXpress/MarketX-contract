#![no_std]

use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};

mod errors;
mod types;

pub use errors::ContractError;
pub use types::{
    DataKey, Escrow, EscrowCreatedEvent, EscrowStatus, FundsReleasedEvent, RefundHistoryEntry,
    RefundReason, RefundRequest, RefundStatus, StatusChangeEvent,
};

#[cfg(test)]
mod test;

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    #[allow(deprecated)]
    pub fn initialize(
        env: Env,
        fee_collector: Address,
        fee_bps: u32,
        min_fee: i128,
    ) -> Result<(), ContractError> {
        if fee_bps > 10_000 || min_fee < 0 {
            return Err(ContractError::InvalidFeeConfig);
        }

        env.storage().persistent().set(&DataKey::FeeCollector, &fee_collector);
        env.storage().persistent().set(&DataKey::FeeBps, &fee_bps);
        env.storage().persistent().set(&DataKey::MinFee, &min_fee);

        Ok(())
    }

    #[allow(deprecated)]
    pub fn store_escrow(env: Env, escrow_id: u64, escrow: Escrow) -> Result<(), ContractError> {
        if escrow.amount <= 0 {
            return Err(ContractError::InvalidEscrowAmount);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Self::emit_escrow_created(&env, escrow_id, &escrow);

        Ok(())
    }

    #[allow(deprecated)]
    pub fn create_escrow(
        env: Env,
        escrow_id: u64,
        buyer: Address,
        seller: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        if amount <= 0 {
            return Err(ContractError::InvalidEscrowAmount);
        }

        let escrow = Escrow {
            buyer,
            seller,
            arbiter: env.current_contract_address(),
            token: env.current_contract_address(),
            amount,
            released_amount: 0,
            status: EscrowStatus::Pending,
            refund_deadline: 0,
            allow_partial_refund: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        let escrow_count: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowCount)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::EscrowCount, &(escrow_count + 1));

        Self::emit_escrow_created(&env, escrow_id, &escrow);

        Ok(())
    }

    #[allow(deprecated)]
    pub fn create_bulk_escrows(
        env: Env,
        buyers: Vec<Address>,
        sellers: Vec<Address>,
        amounts: Vec<i128>,
    ) -> Result<Vec<u64>, ContractError> {
        let len = buyers.len();
        if len != sellers.len() || len != amounts.len() {
            return Err(ContractError::LengthMismatch);
        }

        let mut i: u32 = 0;
        while i < len {
            let amount = amounts.get(i).unwrap();
            if amount <= 0 {
                return Err(ContractError::InvalidEscrowAmount);
            }
            i += 1;
        }

        let mut ids: Vec<u64> = Vec::new(&env);
        let start: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowCount)
            .unwrap_or(0);

        let mut j: u32 = 0;
        while j < len {
            let escrow_id = start + j as u64 + 1;
            let escrow = Escrow {
                buyer: buyers.get(j).unwrap(),
                seller: sellers.get(j).unwrap(),
                arbiter: env.current_contract_address(),
                token: env.current_contract_address(),
                amount: amounts.get(j).unwrap(),
                released_amount: 0,
                status: EscrowStatus::Pending,
                refund_deadline: 0,
                allow_partial_refund: false,
            };

            env.storage()
                .persistent()
                .set(&DataKey::Escrow(escrow_id), &escrow);
            Self::emit_escrow_created(&env, escrow_id, &escrow);

            ids.push_back(escrow_id);
            j += 1;
        }

        env.storage()
            .persistent()
            .set(&DataKey::EscrowCount, &(start + len as u64));

        Ok(ids)
    }

    pub fn get_escrow(env: Env, escrow_id: u64) -> Escrow {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .unwrap()
    }

    pub fn release_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        let escrow = Self::get_escrow_or_err(&env, escrow_id)?;
        escrow.buyer.require_auth();

        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::EscrowNotFunded);
        }

        let remaining = escrow.amount - escrow.released_amount;
        if remaining <= 0 {
            return Err(ContractError::InvalidReleaseAmount);
        }

        Self::release_amount(&env, escrow_id, escrow, remaining)
    }

    pub fn release_partial(env: Env, escrow_id: u64, amount: i128) -> Result<(), ContractError> {
        let escrow = Self::get_escrow_or_err(&env, escrow_id)?;
        escrow.buyer.require_auth();

        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::EscrowNotFunded);
        }

        Self::release_amount(&env, escrow_id, escrow, amount)
    }

    pub fn seller_refund(
        env: Env,
        escrow_id: u64,
        caller: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        let mut escrow = Self::get_escrow_or_err(&env, escrow_id)?;
        if caller != escrow.seller {
            return Err(ContractError::Unauthorized);
        }
        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidTransition);
        }

        let previous = escrow.status.clone();
        escrow.status = EscrowStatus::Refunded;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);
        Self::emit_status_change(&env, escrow_id, previous, EscrowStatus::Refunded, caller);

        Ok(())
    }

    // Test helper to simulate a malicious nested call attempt.
    pub fn simulate_reentrant_release(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        let escrow = Self::get_escrow_or_err(&env, escrow_id)?;
        escrow.buyer.require_auth();

        Self::enter_reentrancy_guard(&env)?;
        let nested_result = Self::release_escrow(env.clone(), escrow_id);
        Self::exit_reentrancy_guard(&env);
        nested_result
    }

    fn get_escrow_or_err(env: &Env, escrow_id: u64) -> Result<Escrow, ContractError> {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)
    }

    #[allow(deprecated)]
    fn release_amount(
        env: &Env,
        escrow_id: u64,
        mut escrow: Escrow,
        amount: i128,
    ) -> Result<(), ContractError> {
        if amount <= 0 {
            return Err(ContractError::InvalidReleaseAmount);
        }

        let new_total = escrow
            .released_amount
            .checked_add(amount)
            .ok_or(ContractError::InvalidReleaseAmount)?;
        if new_total > escrow.amount {
            return Err(ContractError::InvalidReleaseAmount);
        }

        Self::enter_reentrancy_guard(env)?;

        let fee_bps: u32 = env.storage().persistent().get(&DataKey::FeeBps).unwrap_or(0);
        let min_fee: i128 = env.storage().persistent().get(&DataKey::MinFee).unwrap_or(0);

        let fee = amount
            .checked_mul(fee_bps as i128)
            .ok_or(ContractError::InvalidReleaseAmount)?
            / 10_000;

        if fee < min_fee {
            Self::exit_reentrancy_guard(env);
            return Err(ContractError::FeeBelowMinimum);
        }

        let seller_payout = amount - fee;
        let previous = escrow.status.clone();
        escrow.released_amount = new_total;
        if escrow.released_amount == escrow.amount {
            escrow.status = EscrowStatus::Released;
        }

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Self::emit_funds_released(env, escrow_id, &escrow, amount, fee, seller_payout);
        if previous != escrow.status {
            Self::emit_status_change(
                env,
                escrow_id,
                previous,
                escrow.status.clone(),
                escrow.buyer.clone(),
            );
        }

        Self::exit_reentrancy_guard(env);

        Ok(())
    }

    fn enter_reentrancy_guard(env: &Env) -> Result<(), ContractError> {
        let locked: bool = env
            .storage()
            .persistent()
            .get(&DataKey::ReentrancyLock)
            .unwrap_or(false);

        if locked {
            return Err(ContractError::ReentrancyDetected);
        }

        env.storage().persistent().set(&DataKey::ReentrancyLock, &true);
        Ok(())
    }

    fn exit_reentrancy_guard(env: &Env) {
        env.storage().persistent().set(&DataKey::ReentrancyLock, &false);
    }

    fn emit_escrow_created(env: &Env, escrow_id: u64, escrow: &Escrow) {
        env.events().publish(
            (Symbol::new(env, "escrow_created"), escrow_id),
            EscrowCreatedEvent {
                escrow_id,
                buyer: escrow.buyer.clone(),
                seller: escrow.seller.clone(),
                arbiter: escrow.arbiter.clone(),
                token: escrow.token.clone(),
                amount: escrow.amount,
                released_amount: escrow.released_amount,
                status: escrow.status.clone(),
            },
        );
    }

    fn emit_funds_released(
        env: &Env,
        escrow_id: u64,
        escrow: &Escrow,
        gross_amount: i128,
        fee_amount: i128,
        net_amount: i128,
    ) {
        env.events().publish(
            (Symbol::new(env, "funds_released"), escrow_id),
            FundsReleasedEvent {
                escrow_id,
                buyer: escrow.buyer.clone(),
                seller: escrow.seller.clone(),
                gross_amount,
                fee_amount,
                net_amount,
                released_amount: escrow.released_amount,
                total_amount: escrow.amount,
                is_final_release: escrow.status == EscrowStatus::Released,
            },
        );
    }

    fn emit_status_change(
        env: &Env,
        escrow_id: u64,
        from_status: EscrowStatus,
        to_status: EscrowStatus,
        actor: Address,
    ) {
        env.events().publish(
            (Symbol::new(env, "status_change"), escrow_id),
            StatusChangeEvent {
                escrow_id,
                from_status,
                to_status,
                actor,
            },
        );
    }
}
