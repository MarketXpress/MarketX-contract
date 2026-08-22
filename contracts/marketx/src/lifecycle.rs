//! lifecycle operations for the MarketX escrow contract.

use crate::*;
use soroban_sdk::*;

#[contractimpl]
impl Contract {
    // =========================================================================
    // 🤝 ISSUE #205: DISPUTE MEDIATION PHASE
    // =========================================================================

    /// Open a mediation window for a disputed escrow.
    ///
    /// Called automatically when a dispute is raised (via `refund_escrow`), or
    /// manually by any escrow party. During the window, both parties may call
    /// `propose_mediation_settlement` to agree on a split without arbiter
    /// involvement. The arbiter may only call `resolve_dispute` after the
    /// mediation window has expired.
    ///
    /// If `window_ledgers` is 0, `DEFAULT_MEDIATION_WINDOW_LEDGERS` is used.
    pub fn open_mediation(
        env: Env,
        caller: Address,
        escrow_id: u64,
        window_ledgers: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_disputes_enabled(&env)?;
        caller.require_auth();

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
        }

        let is_party = escrow.buyer == caller
            || escrow.seller == caller
            || escrow.arbiter.as_ref() == Some(&caller);
        if !is_party {
            let admin: Address = env
                .storage()
                .persistent()
                .get(&DataKey::Admin)
                .ok_or(ContractError::NotAdmin)?;
            if admin != caller {
                return Err(ContractError::Unauthorized);
            }
        }

        if window_ledgers > MAX_MEDIATION_WINDOW_LEDGERS {
            return Err(ContractError::InvalidMediationWindow);
        }

        let ledgers = if window_ledgers == 0 {
            DEFAULT_MEDIATION_WINDOW_LEDGERS
        } else {
            window_ledgers
        };

        let now = env.ledger().sequence();
        let phase = MediationPhase {
            escrow_id,
            opened_at: now,
            expires_at: now + ledgers,
            buyer_proposal: None,
            seller_proposal: None,
            concluded: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::MediationPhase(escrow_id), &phase);

        MediationOpenedEvent {
            escrow_id,
            expires_at: now + ledgers,
        }
        .publish(&env);

        Ok(())
    }

    /// Admin escape hatch to cancel/conclude an open mediation phase (#256).
    pub fn cancel_mediation(env: Env, admin: Address, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        admin.require_auth();
        let current_admin = Self::assert_admin(&env)?;
        if current_admin != admin {
            return Err(ContractError::Unauthorized);
        }

        let mut phase: MediationPhase = env
            .storage()
            .persistent()
            .get(&DataKey::MediationPhase(escrow_id))
            .ok_or(ContractError::NoMediationPhase)?;

        if phase.concluded {
            return Err(ContractError::MediationAlreadyConcluded);
        }

        phase.concluded = true;
        env.storage()
            .persistent()
            .set(&DataKey::MediationPhase(escrow_id), &phase);

        Ok(())
    }

    pub fn get_total_refunded_amount(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalRefundedAmount)
            .unwrap_or(0)
    }

    pub fn fund_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        // 1. Load and validate the escrow exists
        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // 2. Validate escrow is in a fundable state
        if escrow.status != EscrowStatus::Pending && escrow.status != EscrowStatus::Funded {
            return Err(ContractError::InvalidEscrowState);
        }

        Self::assert_token_not_paused(&env, &escrow.token)?;

        // 3. Enforce buyer authorization (covers the token transfer below)
        escrow.buyer.require_auth();

        // 4. Transfer funds from buyer into the contract
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        #[allow(clippy::needless_borrows_for_generic_args)]
        token_client.transfer(
            &escrow.buyer,
            &env.current_contract_address(),
            &escrow.amount,
        );

        let current_total: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalFundedAmount)
            .unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::TotalFundedAmount,
            &(current_total + escrow.amount),
        );

        let mut funded_escrow = escrow;
        funded_escrow.status = EscrowStatus::Funded;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &funded_escrow);

        Ok(())
    }

    pub fn release_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        Self::assert_escrow_funded(&escrow)?;

        escrow.buyer.require_auth();
        let actor = escrow.buyer.clone();
        let from_status = escrow.status.clone();

        let fee = Self::process_seller_transfer(
            &env,
            escrow_id,
            escrow.amount,
            &escrow.token,
            &escrow.seller,
            &escrow.buyer,
        )?;

        escrow.status = EscrowStatus::Released;
        escrow.cancellation_proposer = None;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        FundsReleasedEvent {
            escrow_id,
            amount: escrow.amount,
            fee,
        }
        .publish(&env);

        Self::emit_status_change(&env, escrow_id, from_status, escrow.status.clone(), actor);

        Self::add_i128(&env, DataKey::TotalReleasedAmount, escrow.amount);

        Ok(())
    }
    pub fn release_partial(env: Env, _escrow_id: u64, _amount: i128) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_partial_releases_enabled(&env)?;
        Ok(())
    }

    /// Release a specific item from an escrow.
    ///
    /// This allows partial release of escrow funds as individual items are delivered.
    /// Only the buyer can release items. Once all items are released, the escrow
    /// status changes to Released.
    ///
    /// # Arguments
    /// * `escrow_id` - The ID of the escrow
    /// * `item_index` - The index of the item to release (0-based)
    ///
    /// # Errors
    /// * `EscrowNotFound` - If the escrow doesn't exist
    /// * `InvalidEscrowState` - If the escrow is not in Funded state
    /// * `ItemNotFound` - If the item index is out of bounds
    /// * `ItemAlreadyReleased` - If the item has already been released
    pub fn release_item(env: Env, escrow_id: u64, item_index: u32) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_partial_releases_enabled(&env)?;

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        Self::assert_escrow_funded(&escrow)?;

        escrow.buyer.require_auth();

        let mut item = escrow
            .items
            .get(item_index)
            .ok_or(ContractError::ItemNotFound)?;
        if item.released {
            return Err(ContractError::ItemAlreadyReleased);
        }

        item.released = true;
        escrow.items.set(item_index as u32, item.clone());

        let fee = Self::process_seller_transfer(
            &env,
            escrow_id,
            item.amount,
            &escrow.token,
            &escrow.seller,
            &escrow.buyer,
        )?;

        let all_released = escrow.items.iter().all(|i| i.released);

        FundsReleasedEvent {
            escrow_id,
            amount: item.amount,
            fee,
        }
        .publish(&env);

        if all_released {
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

        Self::add_i128(&env, DataKey::TotalReleasedAmount, item.amount);

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Ok(())
    }

    pub fn propose_cancellation(
        env: Env,
        escrow_id: u64,
        actor: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        actor.require_auth();

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if !Self::is_escrow_party(&escrow, &actor) {
            return Err(ContractError::Unauthorized);
        }

        if Self::assert_escrow_funded(&escrow).is_err() || Self::has_released_items(&escrow) {
            return Err(ContractError::InvalidEscrowState);
        }

        if let Some(existing) = &escrow.cancellation_proposer {
            if *existing == actor {
                return Ok(());
            }

            // If the other party already proposed, auto-accept the cancellation
            let from_status = escrow.status.clone();
            Self::cancel_to_buyer(&env, &mut escrow);
            env.storage()
                .persistent()
                .set(&DataKey::Escrow(escrow_id), &escrow);
            Self::emit_status_change(&env, escrow_id, from_status, escrow.status.clone(), actor);
            EscrowCancelledEvent {
                escrow_id,
                buyer: escrow.buyer.clone(),
                seller: escrow.seller.clone(),
                amount: escrow.amount,
                was_funded: true,
            }
            .publish(&env);
            return Ok(());
        }

        escrow.cancellation_proposer = Some(actor.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        CancellationProposedEvent { escrow_id, actor }.publish(&env);

        Ok(())
    }

    pub fn accept_cancellation(
        env: Env,
        escrow_id: u64,
        actor: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        actor.require_auth();

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if !Self::is_escrow_party(&escrow, &actor) {
            return Err(ContractError::Unauthorized);
        }

        if Self::assert_escrow_funded(&escrow).is_err() || Self::has_released_items(&escrow) {
            return Err(ContractError::InvalidEscrowState);
        }

        let proposer = escrow
            .cancellation_proposer
            .clone()
            .ok_or(ContractError::InvalidEscrowState)?;

        if proposer == actor {
            return Err(ContractError::Unauthorized);
        }

        let from_status = escrow.status.clone();
        Self::cancel_to_buyer(&env, &mut escrow);
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);
        Self::emit_status_change(&env, escrow_id, from_status, escrow.status.clone(), actor);
        EscrowCancelledEvent {
            escrow_id,
            buyer: escrow.buyer.clone(),
            seller: escrow.seller.clone(),
            amount: escrow.amount,
            was_funded: true,
        }
        .publish(&env);

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
        Self::assert_disputes_enabled(&env)?;
        initiator.require_auth();

        Self::validate_bytes_size(&evidence_hash, MAX_EVIDENCE_HASH_SIZE)?;

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if initiator != escrow.buyer {
            return Err(ContractError::Unauthorized);
        }

        Self::assert_escrow_funded(&escrow)?;

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
        escrow.cancellation_proposer = None;
        Self::add_u32(&env, DataKey::TotalDisputedCount);
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        let event = RefundRequestedEvent {
            request_id,
            escrow_id,
            requester: initiator.clone(),
            evidence_hash: Some(evidence_hash),
        };
        event.publish(&env);

        Self::emit_status_change(
            &env,
            escrow_id,
            from_status,
            escrow.status.clone(),
            initiator,
        );

        Ok(request_id)
    }

    pub fn bump_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        let max_ttl = env.storage().max_ttl();
        let escrow_key = DataKey::Escrow(escrow_id);
        env.storage()
            .persistent()
            .extend_ttl(&escrow_key, max_ttl, max_ttl);

        let hash_key = DataKey::EscrowHash(Self::generate_escrow_hash(
            &env,
            &escrow.buyer,
            &escrow.seller,
            &escrow.metadata,
        ));
        if env.storage().persistent().has(&hash_key) {
            env.storage()
                .persistent()
                .extend_ttl(&hash_key, max_ttl, max_ttl);
        }

        Ok(())
    }

    /// Cancel an escrow that was never funded after the expiry window has elapsed.
    ///
    /// Anyone may call this once `UNFUNDED_EXPIRY_LEDGERS` ledgers have passed
    /// since the escrow was created without it being funded. The escrow record
    /// and its duplicate-prevention hash are both removed from storage.
    ///
    /// # Arguments
    /// * `escrow_id` - The ID of the escrow to cancel
    ///
    /// # Errors
    /// * `EscrowNotFound` - If the escrow doesn't exist
    /// * `EscrowAlreadyFunded` - If the escrow is not in Pending state (i.e. it was funded)
    /// * `EscrowNotExpired` - If the expiry window has not yet elapsed
    pub fn cancel_unfunded(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Only Pending escrows can be cancelled as unfunded.
        // Any other status means the escrow was already funded/acted upon.
        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::EscrowAlreadyFunded);
        }

        let current_ledger = env.ledger().sequence();
        let expiry_ledger = escrow.created_at.saturating_add(UNFUNDED_EXPIRY_LEDGERS);

        if current_ledger < expiry_ledger {
            return Err(ContractError::EscrowNotExpired);
        }

        // Remove the escrow record
        env.storage()
            .persistent()
            .remove(&DataKey::Escrow(escrow_id));

        // Remove the duplicate-prevention hash so the same escrow can be recreated
        let hash =
            Self::generate_escrow_hash(&env, &escrow.buyer, &escrow.seller, &escrow.metadata);
        env.storage()
            .persistent()
            .remove(&DataKey::EscrowHash(hash));

        Self::add_u32(&env, DataKey::TotalCancelledCount);
        EscrowCancelledEvent {
            escrow_id,
            buyer: escrow.buyer.clone(),
            seller: escrow.seller.clone(),
            amount: escrow.amount,
            was_funded: false,
        }
        .publish(&env);

        Ok(())
    }

    /// Resolve a disputed escrow.
    ///
    /// If the escrow has an assigned arbiter, only that arbiter may call this.
    /// Otherwise, the contract admin may resolve it.
    ///
    /// `resolution`: 0 = release to seller, 1 = refund to buyer
    pub fn resolve_dispute(env: Env, escrow_id: u64, resolution: u32) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_disputes_enabled(&env)?;

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
        }

        // Block arbiter resolution while mediation window is still open (#205)
        if let Some(phase) = env
            .storage()
            .persistent()
            .get::<DataKey, MediationPhase>(&DataKey::MediationPhase(escrow_id))
        {
            if !phase.concluded && env.ledger().sequence() <= phase.expires_at {
                return Err(ContractError::MediationPhaseOpen);
            }
        }

        // Enforce arbiter or admin authorization
        let actor = match &escrow.arbiter {
            Some(arbiter) => {
                arbiter.require_auth();
                arbiter.clone()
            }
            None => Self::assert_admin(&env)?,
        };
        let from_status = escrow.status.clone();

        if resolution == 0 {
            // Release to seller - set claimable ledger
            escrow.status = EscrowStatus::Released;
            escrow.cancellation_proposer = None;

            let current_released_total: i128 = env
                .storage()
                .persistent()
                .get(&DataKey::TotalReleasedAmount)
                .unwrap_or(0);
            env.storage().persistent().set(
                &DataKey::TotalReleasedAmount,
                &(current_released_total + escrow.amount),
            );
        } else if resolution == 1 {
            // Refund to buyer - set claimable ledger
            escrow.status = EscrowStatus::Refunded;
        } else {
            return Err(ContractError::InvalidEscrowState);
        }

        // Set the ledger after which funds can be claimed (Appeal Window)
        let claimable_at = env.ledger().sequence() + APPEAL_WINDOW_LEDGERS;
        env.storage()
            .persistent()
            .set(&DataKey::ClaimableAt(escrow_id), &claimable_at);

        // Update associated refund requests if they exist
        let escrow_refunds: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowRefunds(escrow_id))
            .unwrap_or(Vec::new(&env));

        for req_id in escrow_refunds.iter() {
            if let Some(mut req) = env
                .storage()
                .persistent()
                .get::<DataKey, RefundRequest>(&DataKey::RefundRequest(req_id))
            {
                if req.status == RefundStatus::Pending {
                    req.status = if resolution == 1 {
                        RefundStatus::Approved
                    } else {
                        RefundStatus::Rejected
                    };
                    env.storage()
                        .persistent()
                        .set(&DataKey::RefundRequest(req_id), &req);
                }
            }
        }

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Self::emit_status_change(
            &env,
            escrow_id,
            from_status,
            escrow.status.clone(),
            actor.clone(),
        );

        // #204: record resolution in arbiter reputation
        if let Some(arbiter) = &escrow.arbiter {
            Self::record_arbiter_resolution(&env, arbiter);
        }
        // #201: return stake to arbiter (no active appeal yet)
        Self::return_arbiter_stake(&env, escrow_id);

        Ok(())
    }

    /// Claim funds from a resolved dispute after the appeal window has closed.
    ///
    /// # Arguments
    /// * `escrow_id` - The escrow ID
    pub fn claim_disputed_funds(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        let claimable_at: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::ClaimableAt(escrow_id))
            .ok_or(ContractError::InvalidEscrowState)?;

        if env.ledger().sequence() <= claimable_at {
            return Err(ContractError::AppealWindowNotClosed);
        }

        // Check if there's an active (unresolved) appeal
        if let Some(appeal) = env
            .storage()
            .persistent()
            .get::<DataKey, AppealRecord>(&DataKey::Appeal(escrow_id))
        {
            if !appeal.resolved {
                return Err(ContractError::AppealAlreadyFiled);
            }
        }

        match escrow.status {
            EscrowStatus::Released => {
                let fee = Self::process_seller_transfer(
                    &env,
                    escrow_id,
                    escrow.amount,
                    &escrow.token,
                    &escrow.seller,
                    &escrow.buyer,
                )?;

                FundsReleasedEvent {
                    escrow_id,
                    amount: escrow.amount,
                    fee,
                }
                .publish(&env);

                Self::add_i128(&env, DataKey::TotalReleasedAmount, escrow.amount);
            }
            EscrowStatus::Refunded => {
                Self::refund_buyer(&env, &mut escrow);
            }
            _ => return Err(ContractError::InvalidEscrowState),
        }

        // Clean up
        env.storage()
            .persistent()
            .remove(&DataKey::ClaimableAt(escrow_id));
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Ok(())
    }
}
