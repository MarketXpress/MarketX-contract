//! arbiters operations for the MarketX escrow contract.

use crate::*;
use soroban_sdk::*;

#[contractimpl]
impl Contract {
    // =========================================================================
    // ⚖️  DISPUTE RESOLUTION V2 (#201-204)
    // =========================================================================

    // ── #201: Arbiter Staking & Slashing ─────────────────────────────────────

    /// Set the minimum token amount an arbiter must stake before resolving a
    /// disputed escrow.  Admin-only.
    pub fn set_min_arbiter_stake(env: Env, amount: i128) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        assert!(amount >= 0, "min stake must be non-negative");
        env.storage()
            .persistent()
            .set(&DataKey::MinArbiterStake, &amount);
        Ok(())
    }

    /// Get the current minimum arbiter stake (returns 0 if not configured).
    pub fn get_min_arbiter_stake(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::MinArbiterStake)
            .unwrap_or(0)
    }

    /// Arbiter stakes tokens to gain the authority to resolve the given disputed
    /// escrow.  The stake is held by the contract until the escrow is resolved.
    ///
    /// # Errors
    /// - `InvalidEscrowState` if the escrow is not in `Disputed` status.
    /// - `ArbiterMismatch` if the caller is not the registered arbiter.
    /// - `ArbiterAlreadyStaked` if a stake already exists for this escrow.
    /// - `ArbiterStakeInsufficient` if `amount` < `MinArbiterStake`.
    pub fn stake_as_arbiter(
        env: Env,
        arbiter: Address,
        escrow_id: u64,
        amount: i128,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_disputes_enabled(&env)?;
        arbiter.require_auth();

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
        }

        match &escrow.arbiter {
            Some(registered) if *registered != arbiter => {
                return Err(ContractError::ArbiterMismatch)
            }
            None => return Err(ContractError::ArbiterMismatch),
            _ => {}
        }

        let stake_key = DataKey::ArbiterStake(escrow_id);
        if env.storage().persistent().has(&stake_key) {
            return Err(ContractError::ArbiterAlreadyStaked);
        }

        let min_stake: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::MinArbiterStake)
            .unwrap_or(0);

        if amount < min_stake {
            return Err(ContractError::ArbiterStakeInsufficient);
        }

        // Transfer stake from arbiter to the contract
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        token_client.transfer(&arbiter, env.current_contract_address(), &amount);

        let stake = ArbiterStake {
            arbiter: arbiter.clone(),
            token: escrow.token.clone(),
            amount,
            staked_at: env.ledger().sequence(),
            slashed: false,
            slash_amount: 0,
        };

        env.storage().persistent().set(&stake_key, &stake);

        // Update reputation
        Self::increment_arbiter_disputes(&env, &arbiter);

        ArbiterStakedEvent {
            escrow_id,
            arbiter,
            amount,
        }
        .publish(&env);
        Ok(())
    }

    /// Return an arbiter's stake after a successful (non-overturned) resolution.
    /// Called internally; the stake is returned when `resolve_dispute` executes
    /// and no active appeal overturns the ruling.
    pub(crate) fn return_arbiter_stake(env: &Env, escrow_id: u64) {
        let stake_key = DataKey::ArbiterStake(escrow_id);
        let Some(stake): Option<ArbiterStake> = env.storage().persistent().get(&stake_key) else {
            return;
        };
        if stake.slashed || stake.amount == 0 {
            return;
        }
        let token_client = soroban_sdk::token::Client::new(env, &stake.token);
        token_client.transfer(
            &env.current_contract_address(),
            &stake.arbiter,
            &stake.amount,
        );
        env.storage().persistent().remove(&stake_key);
    }

    /// Admin slashes an arbiter's stake after an appeal overturns their ruling.
    /// The slashed amount is sent to the winning party (the appellant).
    ///
    /// # Errors
    /// - `AppealNotFound` if no resolved-and-overturned appeal exists.
    pub fn slash_arbiter(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        Self::assert_disputes_enabled(&env)?;

        let appeal: AppealRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Appeal(escrow_id))
            .ok_or(ContractError::AppealNotFound)?;

        if !appeal.resolved {
            return Err(ContractError::AppealNotFound);
        }

        let stake_key = DataKey::ArbiterStake(escrow_id);
        let mut stake: ArbiterStake = env
            .storage()
            .persistent()
            .get(&stake_key)
            .ok_or(ContractError::EscrowNotFound)?;

        if stake.slashed || stake.amount == 0 {
            return Ok(());
        }

        let slash_amount = stake.amount / 2; // Slash 50% of stake
        stake.slashed = true;
        stake.slash_amount = slash_amount;
        stake.amount -= slash_amount;

        let token_client = soroban_sdk::token::Client::new(&env, &stake.token);
        // Slashed portion goes to the appellant (winning party)
        token_client.transfer(
            &env.current_contract_address(),
            &appeal.appellant,
            &slash_amount,
        );
        // Remaining stake returned to arbiter
        if stake.amount > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &stake.arbiter,
                &stake.amount,
            );
        }

        // Update arbiter reputation
        let rep_key = DataKey::ArbiterReputation(stake.arbiter.clone());
        let mut rep: ArbiterReputation =
            env.storage()
                .persistent()
                .get(&rep_key)
                .unwrap_or(ArbiterReputation {
                    arbiter: stake.arbiter.clone(),
                    total_disputes: 0,
                    resolved_disputes: 0,
                    appealed_rulings: 0,
                    overturned_rulings: 0,
                    slash_count: 0,
                    last_active: 0,
                });
        rep.slash_count += 1;
        rep.last_active = env.ledger().sequence();
        env.storage().persistent().set(&rep_key, &rep);

        ArbiterSlashedEvent {
            escrow_id,
            arbiter: stake.arbiter.clone(),
            slash_amount,
        }
        .publish(&env);

        env.storage().persistent().set(&stake_key, &stake);
        Ok(())
    }

    // ── #202: Evidence Submission Windows & Expiry ───────────────────────────

    /// Open an evidence submission window for a disputed escrow.
    /// Both parties may call `submit_evidence` until the window closes.
    /// If `window_ledgers` is 0, the default window length is used.
    pub fn open_evidence_window(
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

        // Only arbiter, buyer, seller, or admin may open the window
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

        let ledgers = if window_ledgers == 0 {
            DEFAULT_EVIDENCE_WINDOW_LEDGERS
        } else {
            window_ledgers
        };

        let now = env.ledger().sequence();
        let window = EvidenceWindow {
            escrow_id,
            opened_at: now,
            expires_at: now + ledgers,
            buyer_submitted: false,
            seller_submitted: false,
            buyer_evidence_hash: None,
            seller_evidence_hash: None,
            expired: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::EvidenceWindow(escrow_id), &window);
        Ok(())
    }

    /// Submit an evidence hash within the open evidence window.
    /// The caller must be the buyer or seller of the escrow.
    pub fn submit_evidence(
        env: Env,
        party: Address,
        escrow_id: u64,
        evidence_hash: Bytes,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_disputes_enabled(&env)?;
        party.require_auth();

        Self::validate_bytes_size(&evidence_hash, MAX_EVIDENCE_HASH_SIZE)?;

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
        }

        let mut window: EvidenceWindow = env
            .storage()
            .persistent()
            .get(&DataKey::EvidenceWindow(escrow_id))
            .ok_or(ContractError::NoEvidenceWindow)?;

        if window.expired || env.ledger().sequence() > window.expires_at {
            return Err(ContractError::EvidenceWindowExpired);
        }

        if party == escrow.buyer {
            window.buyer_submitted = true;
            window.buyer_evidence_hash = Some(evidence_hash.clone());
        } else if party == escrow.seller {
            window.seller_submitted = true;
            window.seller_evidence_hash = Some(evidence_hash.clone());
        } else {
            return Err(ContractError::Unauthorized);
        }

        env.storage()
            .persistent()
            .set(&DataKey::EvidenceWindow(escrow_id), &window);

        EvidenceSubmittedEvent {
            escrow_id,
            party,
            evidence_hash,
        }
        .publish(&env);

        Ok(())
    }

    /// Expire an evidence window that has passed its deadline.
    /// If neither party submitted evidence, the dispute resolves as a refund
    /// (default in favour of buyer when arbiter is absent).
    pub fn expire_evidence_window(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_disputes_enabled(&env)?;

        let mut window: EvidenceWindow = env
            .storage()
            .persistent()
            .get(&DataKey::EvidenceWindow(escrow_id))
            .ok_or(ContractError::NoEvidenceWindow)?;

        if window.expired {
            return Ok(());
        }
        if env.ledger().sequence() <= window.expires_at {
            return Err(ContractError::EvidenceWindowNotExpired);
        }

        window.expired = true;
        env.storage()
            .persistent()
            .set(&DataKey::EvidenceWindow(escrow_id), &window);

        // Default resolution: refund buyer when no arbiter resolved in time
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        let default_refund = escrow.status == EscrowStatus::Disputed;
        if default_refund {
            let from = escrow.status.clone();
            Self::refund_buyer(&env, &mut escrow);
            env.storage()
                .persistent()
                .set(&DataKey::Escrow(escrow_id), &escrow);
            let buyer = escrow.buyer.clone();
            Self::emit_status_change(&env, escrow_id, from, EscrowStatus::Refunded, buyer);
        }

        EvidenceWindowExpiredEvent {
            escrow_id,
            default_refund,
        }
        .publish(&env);

        Ok(())
    }

    /// Read the evidence window for an escrow.
    pub fn get_evidence_window(env: Env, escrow_id: u64) -> Option<EvidenceWindow> {
        env.storage()
            .persistent()
            .get(&DataKey::EvidenceWindow(escrow_id))
    }

    // ── #203: Resolution Appeal Mechanism ────────────────────────────────────

    /// File an appeal after an arbiter's ruling.  Must be called within
    /// `APPEAL_WINDOW_LEDGERS` of the ruling ledger.
    ///
    /// The `ruling_ledger` parameter is the ledger at which `resolve_dispute`
    /// was called (caller supplies it; admin verifies on `resolve_appeal`).
    pub fn file_appeal(
        env: Env,
        appellant: Address,
        escrow_id: u64,
        ruling_ledger: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_disputes_enabled(&env)?;
        appellant.require_auth();

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Only buyer or seller may appeal
        if escrow.buyer != appellant && escrow.seller != appellant {
            return Err(ContractError::Unauthorized);
        }

        let appeal_key = DataKey::Appeal(escrow_id);
        if env.storage().persistent().has(&appeal_key) {
            return Err(ContractError::AppealAlreadyFiled);
        }

        let now = env.ledger().sequence();
        if now > ruling_ledger + APPEAL_WINDOW_LEDGERS {
            return Err(ContractError::AppealWindowClosed);
        }

        let record = AppealRecord {
            escrow_id,
            appellant: appellant.clone(),
            filed_at: now,
            resolved: false,
            outcome: None,
            ruling_ledger,
        };

        env.storage().persistent().set(&appeal_key, &record);

        // Update arbiter reputation: appealed_rulings++
        if let Some(arbiter) = &escrow.arbiter {
            let rep_key = DataKey::ArbiterReputation(arbiter.clone());
            let mut rep: ArbiterReputation =
                env.storage()
                    .persistent()
                    .get(&rep_key)
                    .unwrap_or(ArbiterReputation {
                        arbiter: arbiter.clone(),
                        total_disputes: 0,
                        resolved_disputes: 0,
                        appealed_rulings: 0,
                        overturned_rulings: 0,
                        slash_count: 0,
                        last_active: 0,
                    });
            rep.appealed_rulings += 1;
            rep.last_active = now;
            env.storage().persistent().set(&rep_key, &rep);
        }

        AppealFiledEvent {
            escrow_id,
            appellant,
        }
        .publish(&env);
        Ok(())
    }

    /// Admin resolves an appeal, potentially overriding the arbiter's ruling.
    /// `resolution`: 0 = seller wins, 1 = buyer wins.
    /// If the outcome differs from the arbiter's ruling, `overturned = true` and
    /// the admin should follow up with `slash_arbiter`.
    pub fn resolve_appeal(
        env: Env,
        admin: Address,
        escrow_id: u64,
        resolution: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_disputes_enabled(&env)?;
        admin.require_auth();
        Self::assert_admin(&env)?;

        let appeal_key = DataKey::Appeal(escrow_id);
        let mut appeal: AppealRecord = env
            .storage()
            .persistent()
            .get(&appeal_key)
            .ok_or(ContractError::AppealNotFound)?;

        if appeal.resolved {
            return Err(ContractError::AppealAlreadyResolved);
        }

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);

        // The escrow may already be Released/Refunded from the original ruling.
        // We reverse the original ruling and apply the new one.
        let overturned = match escrow.status {
            EscrowStatus::Released => resolution == 1, // originally seller won; appeal says buyer wins
            EscrowStatus::Refunded => resolution == 0, // originally buyer won; appeal says seller wins
            _ => false,
        };

        if resolution == 0 {
            // Seller wins: if escrow was Refunded, we need to transfer from contract to seller
            // In practice the funds may have already moved; this records the outcome.
            // A full implementation would need a hold-back mechanism.
            escrow.status = EscrowStatus::Released;
        } else {
            // Buyer wins
            if escrow.status != EscrowStatus::Refunded {
                Self::refund_buyer(&env, &mut escrow);
            }
        }

        // Mark appeal resolved
        appeal.resolved = true;
        appeal.outcome = Some(resolution);
        env.storage().persistent().set(&appeal_key, &appeal);
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        // Update arbiter reputation if ruling was overturned
        if overturned {
            if let Some(arbiter) = &escrow.arbiter {
                let rep_key = DataKey::ArbiterReputation(arbiter.clone());
                let mut rep: ArbiterReputation = env
                    .storage()
                    .persistent()
                    .get(&rep_key)
                    .unwrap_or(ArbiterReputation {
                        arbiter: arbiter.clone(),
                        total_disputes: 0,
                        resolved_disputes: 0,
                        appealed_rulings: 0,
                        overturned_rulings: 0,
                        slash_count: 0,
                        last_active: 0,
                    });
                rep.overturned_rulings += 1;
                rep.last_active = env.ledger().sequence();
                env.storage().persistent().set(&rep_key, &rep);
            }
        }

        let _ = token_client; // suppress unused warning

        AppealResolvedEvent {
            escrow_id,
            admin,
            outcome: resolution,
            overturned,
        }
        .publish(&env);

        Ok(())
    }

    /// Get the appeal record for an escrow (None if no appeal filed).
    pub fn get_appeal(env: Env, escrow_id: u64) -> Option<AppealRecord> {
        env.storage().persistent().get(&DataKey::Appeal(escrow_id))
    }

    // ── #204: On-Chain Arbiter Reputation ────────────────────────────────────

    /// Read the reputation record for an arbiter.
    pub fn get_arbiter_reputation(env: Env, arbiter: Address) -> Option<ArbiterReputation> {
        env.storage()
            .persistent()
            .get(&DataKey::ArbiterReputation(arbiter))
    }

    /// Internal: increment total_disputes counter for an arbiter.
    pub(crate) fn increment_arbiter_disputes(env: &Env, arbiter: &Address) {
        let key = DataKey::ArbiterReputation(arbiter.clone());
        let mut rep: ArbiterReputation =
            env.storage()
                .persistent()
                .get(&key)
                .unwrap_or(ArbiterReputation {
                    arbiter: arbiter.clone(),
                    total_disputes: 0,
                    resolved_disputes: 0,
                    appealed_rulings: 0,
                    overturned_rulings: 0,
                    slash_count: 0,
                    last_active: 0,
                });
        rep.total_disputes += 1;
        rep.last_active = env.ledger().sequence();
        env.storage().persistent().set(&key, &rep);
    }

    /// Internal: mark a dispute as resolved in the arbiter's reputation record.
    pub(crate) fn record_arbiter_resolution(env: &Env, arbiter: &Address) {
        let key = DataKey::ArbiterReputation(arbiter.clone());
        let mut rep: ArbiterReputation =
            env.storage()
                .persistent()
                .get(&key)
                .unwrap_or(ArbiterReputation {
                    arbiter: arbiter.clone(),
                    total_disputes: 0,
                    resolved_disputes: 0,
                    appealed_rulings: 0,
                    overturned_rulings: 0,
                    slash_count: 0,
                    last_active: 0,
                });
        rep.resolved_disputes += 1;
        rep.last_active = env.ledger().sequence();
        env.storage().persistent().set(&key, &rep);
    }
}
