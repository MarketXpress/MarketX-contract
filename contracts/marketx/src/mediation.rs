//! mediation operations for the MarketX escrow contract.

use crate::*;
use soroban_sdk::*;

#[contractimpl]
impl Contract {
    /// Propose a settlement amount during the mediation window.
    ///
    /// `seller_amount` is how much the caller proposes the seller receives.
    /// The remainder (`escrow.amount - seller_amount`) goes back to the buyer.
    ///
    /// If both parties propose the same `seller_amount`, the escrow is
    /// immediately settled without arbiter involvement.
    pub fn propose_mediation_settlement(
        env: Env,
        proposer: Address,
        escrow_id: u64,
        seller_amount: i128,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_disputes_enabled(&env)?;
        proposer.require_auth();

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
        }

        if proposer != escrow.buyer && proposer != escrow.seller {
            return Err(ContractError::Unauthorized);
        }

        if seller_amount < 0 || seller_amount > escrow.amount {
            return Err(ContractError::InvalidEscrowAmount);
        }

        let mut phase: MediationPhase = env
            .storage()
            .persistent()
            .get(&DataKey::MediationPhase(escrow_id))
            .ok_or(ContractError::NoMediationPhase)?;

        if phase.concluded {
            return Err(ContractError::MediationAlreadyConcluded);
        }

        if env.ledger().sequence() > phase.expires_at {
            return Err(ContractError::MediationAlreadyConcluded);
        }

        if proposer == escrow.buyer {
            phase.buyer_proposal = Some(seller_amount);
        } else {
            phase.seller_proposal = Some(seller_amount);
        }

        MediationProposedEvent {
            escrow_id,
            proposer: proposer.clone(),
            amount: seller_amount,
        }
        .publish(&env);

        // Check if both parties agree
        let agreed =
            phase.buyer_proposal == phase.seller_proposal && phase.buyer_proposal.is_some();

        if agreed {
            phase.concluded = true;
            env.storage()
                .persistent()
                .set(&DataKey::MediationPhase(escrow_id), &phase);

            Self::execute_mediation_settlement(&env, escrow_id, &escrow, seller_amount)?;
        } else {
            env.storage()
                .persistent()
                .set(&DataKey::MediationPhase(escrow_id), &phase);
        }

        Ok(())
    }

    pub(crate) fn execute_mediation_settlement(
        env: &Env,
        escrow_id: u64,
        escrow: &Escrow,
        seller_amount: i128,
    ) -> Result<(), ContractError> {
        let buyer_refund = escrow.amount - seller_amount;
        let mut net_seller: i128 = 0;

        let token_client = soroban_sdk::token::Client::new(env, &escrow.token);

        if seller_amount > 0 {
            let fee =
                Self::calculate_fee_internal(env, seller_amount, &escrow.token, &escrow.buyer);
            net_seller = seller_amount - fee;
            token_client.transfer(&env.current_contract_address(), &escrow.seller, &net_seller);
            if fee > 0 {
                let fee_collector: Address = env
                    .storage()
                    .persistent()
                    .get(&DataKey::FeeCollector)
                    .ok_or(ContractError::InvalidFeeConfig)?;
                Self::add_pending_fee(env, fee_collector.clone(), escrow.token.clone(), fee);
                Self::add_i128(env, DataKey::TotalFeesCollected, fee);
                FeeCollectedEvent {
                    escrow_id,
                    fee_collector,
                    fee,
                }
                .publish(env);
            }
        }

        if buyer_refund > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &escrow.buyer,
                &buyer_refund,
            );
        }

        let mut updated_escrow = escrow.clone();
        updated_escrow.status = EscrowStatus::Released;
        updated_escrow.cancellation_proposer = None;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &updated_escrow);

        MediationSettledEvent {
            escrow_id,
            seller_amount,
            buyer_refund,
        }
        .publish(env);

        let fee = seller_amount - net_seller;
        FundsReleasedEvent {
            escrow_id,
            amount: escrow.amount,
            fee,
        }
        .publish(env);

        StatusChangeEvent {
            escrow_id,
            from_status: EscrowStatus::Disputed,
            to_status: EscrowStatus::Released,
            actor: escrow.buyer.clone(),
        }
        .publish(env);

        Self::add_i128(env, DataKey::TotalReleasedAmount, net_seller);
        if buyer_refund > 0 {
            Self::add_i128(env, DataKey::TotalRefundedAmount, buyer_refund);
            Self::add_u32(env, DataKey::TotalRefundedCount);
        }

        Ok(())
    }

    /// Read the mediation phase for an escrow.
    pub fn get_mediation_phase(env: Env, escrow_id: u64) -> Option<MediationPhase> {
        env.storage()
            .persistent()
            .get(&DataKey::MediationPhase(escrow_id))
    }
}
