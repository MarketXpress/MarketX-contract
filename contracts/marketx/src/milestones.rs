//! milestones operations for the MarketX escrow contract.

use crate::*;
use soroban_sdk::*;

#[contractimpl]
impl Contract {
    // =========================
    // 🎯 MILESTONE-BASED PAYMENTS (#173)
    // =========================

    /// Create an escrow with milestone-based payment releases.
    ///
    /// # Arguments
    /// * `buyer` - The buyer's address
    /// * `seller` - The seller's address
    /// * `token` - The token contract address
    /// * `amount` - The total escrow amount
    /// * `milestones` - Vector of milestones with descriptions and amounts
    /// * `metadata` - Optional metadata
    /// * `arbiter` - Optional arbiter
    ///
    /// # Errors
    /// * `TooManyItems` - If `milestones` exceeds `MAX_MILESTONES_PER_ESCROW`
    /// * `ItemAmountInvalid` - If milestone amounts don't sum to total amount
    pub fn create_milestone_escrow(
        env: Env,
        buyer: Address,
        seller: Address,
        token: Address,
        amount: i128,
        milestones: Vec<Milestone>,
        metadata: Option<Bytes>,
        arbiter: Option<Address>,
    ) -> Result<u64, ContractError> {
        Self::assert_not_paused(&env)?;
        buyer.require_auth();

        if milestones.len() > MAX_MILESTONES_PER_ESCROW {
            return Err(ContractError::TooManyItems);
        }

        // Validate milestone amounts sum to total
        let milestone_sum: i128 = milestones.iter().map(|m| m.amount).sum();
        if milestone_sum != amount {
            return Err(ContractError::ItemAmountInvalid);
        }

        // Validate milestone descriptions
        for m in milestones.iter() {
            Self::validate_bytes_size(&m.description, MAX_DESCRIPTION_SIZE)?;
        }

        let escrow_id = Self::create_escrow_internal(
            env.clone(),
            buyer,
            seller,
            token,
            amount,
            metadata,
            arbiter,
            None,
            None,
        )?;

        // Store milestones separately
        env.storage()
            .persistent()
            .set(&DataKey::MilestoneEscrow(escrow_id), &milestones);

        // Update escrow with milestones. `escrow_id` was just returned by
        // `create_escrow_internal`, which stores the record before
        // returning, so this lookup cannot miss in practice; the typed
        // error is defensive rather than reachable.
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;
        escrow.milestones = milestones;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Ok(escrow_id)
    }

    /// Complete a milestone and release the associated payment.
    ///
    /// # Arguments
    /// * `escrow_id` - The escrow ID
    /// * `milestone_index` - The index of the milestone to complete
    ///
    /// # Errors
    /// * `EscrowNotFound` - If escrow doesn't exist
    /// * `MilestoneNotFound` - If milestone index is invalid
    /// * `MilestoneAlreadyCompleted` - If milestone is already completed
    pub fn complete_milestone(
        env: Env,
        escrow_id: u64,
        milestone_index: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        escrow.buyer.require_auth();

        Self::assert_escrow_funded(&escrow)?;

        let mut milestone = escrow
            .milestones
            .get(milestone_index)
            .ok_or(ContractError::MilestoneNotFound)?;
        if milestone.completed {
            return Err(ContractError::MilestoneAlreadyCompleted);
        }

        milestone.completed = true;
        milestone.completed_at = Some(env.ledger().timestamp());
        escrow.milestones.set(milestone_index, milestone.clone());

        let fee = Self::process_seller_transfer(
            &env,
            escrow_id,
            milestone.amount,
            &escrow.token,
            &escrow.seller,
            &escrow.buyer,
        )?;

        let all_completed = escrow.milestones.iter().all(|m| m.completed);
        if all_completed {
            let from_status = escrow.status.clone();
            escrow.status = EscrowStatus::Released;
            Self::emit_status_change(
                &env,
                escrow_id,
                from_status,
                escrow.status.clone(),
                escrow.buyer.clone(),
            );
        }

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        MilestoneCompletedEvent {
            escrow_id,
            milestone_index,
            amount: milestone.amount,
        }
        .publish(&env);

        FundsReleasedEvent {
            escrow_id,
            amount: milestone.amount,
            fee,
        }
        .publish(&env);

        Self::add_i128(&env, DataKey::TotalReleasedAmount, milestone.amount);

        Ok(())
    }

    /// Get milestones for an escrow.
    pub fn get_milestones(env: Env, escrow_id: u64) -> Option<Vec<Milestone>> {
        let escrow: Option<Escrow> = env.storage().persistent().get(&DataKey::Escrow(escrow_id));
        escrow.map(|e| e.milestones)
    }
}
