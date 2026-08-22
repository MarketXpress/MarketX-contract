//! multi arbiter operations for the MarketX escrow contract.

use crate::*;
use soroban_sdk::*;

#[contractimpl]
impl Contract {
    // =========================
    // ⚖️ MULTI-ARBITER DISPUTE RESOLUTION V2 FUNCTIONS
    // =========================
    //
    // These functions enable multiple arbiters to vote on dispute resolution,
    // eliminating the single point of failure from a single arbiter.

    /// Configure multiple arbiters for an escrow to enable consensus-based dispute resolution.
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `escrow_id` - The escrow ID to configure
    /// * `arbiters` - List of arbiter addresses (2-7 recommended)
    /// * `quorum_required` - Number of arbiters needed to reach consensus
    ///
    /// # Returns
    /// Stores the arbiters configuration in persistent storage.
    ///
    /// # Errors
    /// - `EscrowNotFound`: Escrow does not exist
    /// - `InvalidEscrowState`: Escrow already funded or in invalid state
    /// - `ArbiterStakeInsufficient`: Arbiters list is empty
    /// - `TooManyItems`: More arbiters than allowed (default max 7)
    pub fn configure_multi_arbiters(
        env: Env,
        escrow_id: u64,
        arbiters: Vec<Address>,
        quorum_required: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_disputes_enabled(&env)?;

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Can only configure before escrow is funded
        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        // Validate arbiters list
        if arbiters.is_empty() {
            return Err(ContractError::ArbiterStakeInsufficient);
        }

        // Reject any arbiter that is a party to the escrow — an arbiter who
        // is also the buyer or seller could rule in their own favor with no
        // independent check (#243).
        for arbiter in arbiters.iter() {
            if arbiter == escrow.buyer || arbiter == escrow.seller {
                return Err(ContractError::ArbiterConflictOfInterest);
            }
        }

        let max_arbiters = env
            .storage()
            .persistent()
            .get::<_, u32>(&DataKey::MaxArbitersPerEscrow)
            .unwrap_or(DEFAULT_MAX_ARBITERS_PER_ESCROW);

        if arbiters.len() as u32 > max_arbiters {
            return Err(ContractError::TooManyItems);
        }

        // Validate quorum
        if quorum_required == 0 || quorum_required > arbiters.len() as u32 {
            return Err(ContractError::InvalidEscrowState);
        }

        // Require authorization from buyer or seller or current arbiter if exists
        let caller = match &escrow.arbiter {
            Some(arbiter) => {
                arbiter.require_auth();
                arbiter.clone()
            }
            None => {
                // If no arbiter yet, require buyer or seller authorization
                escrow.buyer.require_auth();
                escrow.buyer.clone()
            }
        };

        // Store the configuration
        let config = ArbitersConfig {
            escrow_id,
            arbiters: arbiters.clone(),
            quorum_required,
            created_at: env.ledger().sequence(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::ArbitersConfig(escrow_id), &config);

        // Emit event
        ArbitersConfiguredEvent {
            escrow_id,
            arbiters_count: arbiters.len() as u32,
            quorum_required,
            created_by: caller,
        }
        .publish(&env);

        Ok(())
    }

    /// Get the multi-arbiter configuration for an escrow (if any).
    pub fn get_arbiters_config(env: Env, escrow_id: u64) -> Option<ArbitersConfig> {
        env.storage()
            .persistent()
            .get(&DataKey::ArbitersConfig(escrow_id))
    }

    /// Cast a vote on dispute resolution from an authorized arbiter.
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `escrow_id` - The escrow ID
    /// * `vote` - 0 for release to seller, 1 for refund to buyer
    ///
    /// # Behavior
    /// When consensus is reached (quorum votes agree), the dispute is automatically resolved.
    pub fn arbiter_vote(
        env: Env,
        caller: Address,
        escrow_id: u64,
        vote: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_disputes_enabled(&env)?;
        caller.require_auth();

        // Validate vote value
        if vote > 1 {
            return Err(ContractError::InvalidEscrowState);
        }

        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Escrow must be in Disputed state
        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
        }

        // Get the multi-arbiter configuration
        let config: ArbitersConfig = env
            .storage()
            .persistent()
            .get(&DataKey::ArbitersConfig(escrow_id))
            .ok_or(ContractError::InvalidEscrowState)?; // No multi-arbiter config

        // Verify caller is an authorized arbiter
        let mut is_arbiter = false;
        for arbiter in config.arbiters.iter() {
            if arbiter == caller {
                is_arbiter = true;
                break;
            }
        }

        if !is_arbiter {
            return Err(ContractError::ArbiterMismatch);
        }

        // Require auth from this arbiter
        caller.require_auth();

        // Check if this arbiter already voted (prevent double-voting)
        if env
            .storage()
            .persistent()
            .get::<DataKey, ArbiterVoteRecord>(&DataKey::ArbiterVote(escrow_id, caller.clone()))
            .is_some()
        {
            return Err(ContractError::InvalidEscrowState); // Already voted
        }

        // Record the vote
        let vote_record = ArbiterVoteRecord {
            escrow_id,
            arbiter: caller.clone(),
            vote,
            voted_at: env.ledger().sequence(),
        };

        env.storage().persistent().set(
            &DataKey::ArbiterVote(escrow_id, caller.clone()),
            &vote_record,
        );

        // Emit vote event
        ArbiterVoteCastEvent {
            escrow_id,
            arbiter: caller.clone(),
            vote,
        }
        .publish(&env);

        // Update the voting record and check for consensus
        Self::update_dispute_voting(&env, escrow_id)?;

        Ok(())
    }

    /// Internal: Update dispute voting record and trigger resolution if consensus reached.
    pub(crate) fn update_dispute_voting(env: &Env, escrow_id: u64) -> Result<(), ContractError> {
        let config: ArbitersConfig = env
            .storage()
            .persistent()
            .get(&DataKey::ArbitersConfig(escrow_id))
            .ok_or(ContractError::InvalidEscrowState)?;

        // Get or create voting record
        let mut voting: DisputeVotingRecord = env
            .storage()
            .persistent()
            .get(&DataKey::DisputeVoting(escrow_id))
            .unwrap_or(DisputeVotingRecord {
                escrow_id,
                votes_for_release: 0,
                votes_for_refund: 0,
                total_arbiters: config.arbiters.len() as u32,
                quorum_required: config.quorum_required,
                voting_opened_at: env.ledger().sequence(),
                consensus_resolution: None,
                consensus_at: None,
            });

        // Count current votes from persistent storage
        let mut votes_release = 0u32;
        let mut votes_refund = 0u32;

        for arbiter in config.arbiters.iter() {
            if let Some(vote_record) = env
                .storage()
                .persistent()
                .get::<DataKey, ArbiterVoteRecord>(&DataKey::ArbiterVote(
                    escrow_id,
                    arbiter.clone(),
                ))
            {
                if vote_record.vote == 0 {
                    votes_release += 1;
                } else {
                    votes_refund += 1;
                }
            }
        }

        voting.votes_for_release = votes_release;
        voting.votes_for_refund = votes_refund;

        // Check if consensus has been reached
        let consensus_reached =
            votes_release >= config.quorum_required || votes_refund >= config.quorum_required;

        if consensus_reached && voting.consensus_resolution.is_none() {
            // Determine the resolution (which direction has majority)
            let resolution = if votes_release >= votes_refund { 0 } else { 1 };

            voting.consensus_resolution = Some(resolution);
            voting.consensus_at = Some(env.ledger().sequence());

            // Emit consensus event
            DisputeConsensusReachedEvent {
                escrow_id,
                resolution,
                votes_for_release: votes_release,
                votes_for_refund: votes_refund,
            }
            .publish(env);
        }

        // Store updated voting record
        env.storage()
            .persistent()
            .set(&DataKey::DisputeVoting(escrow_id), &voting);

        Ok(())
    }

    /// Get the voting record for a disputed escrow.
    pub fn get_dispute_voting(env: Env, escrow_id: u64) -> Option<DisputeVotingRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::DisputeVoting(escrow_id))
    }

    /// Check if consensus has been reached on a disputed escrow.
    pub fn has_consensus(env: Env, escrow_id: u64) -> bool {
        if let Some(voting) = env
            .storage()
            .persistent()
            .get::<DataKey, DisputeVotingRecord>(&DataKey::DisputeVoting(escrow_id))
        {
            voting.consensus_resolution.is_some()
        } else {
            false
        }
    }

    /// Resolve a multi-arbiter dispute after consensus is reached.
    ///
    /// Can be called by any of the authorized arbiters once consensus is achieved.
    pub fn resolve_multi_arbiter_dispute(
        env: Env,
        caller: Address,
        escrow_id: u64,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        Self::assert_disputes_enabled(&env)?;
        caller.require_auth();

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
        }

        // Get the configuration and voting record
        let config: ArbitersConfig = env
            .storage()
            .persistent()
            .get(&DataKey::ArbitersConfig(escrow_id))
            .ok_or(ContractError::InvalidEscrowState)?;

        let voting: DisputeVotingRecord = env
            .storage()
            .persistent()
            .get(&DataKey::DisputeVoting(escrow_id))
            .ok_or(ContractError::InvalidEscrowState)?;

        // Verify consensus has been reached
        let resolution = voting
            .consensus_resolution
            .ok_or(ContractError::InvalidEscrowState)?;

        // Verify caller is one of the authorized arbiters
        let mut is_arbiter = false;
        for arbiter in config.arbiters.iter() {
            if arbiter == caller {
                is_arbiter = true;
                break;
            }
        }

        if !is_arbiter {
            return Err(ContractError::ArbiterMismatch);
        }

        caller.require_auth();

        // Apply the consensus resolution
        let from_status = escrow.status.clone();

        if resolution == 0 {
            // Release to seller
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
            // Refund to buyer
            escrow.status = EscrowStatus::Refunded;
        } else {
            return Err(ContractError::InvalidEscrowState);
        }

        // Set the claimable ledger (appeal window)
        let claimable_at = env.ledger().sequence() + APPEAL_WINDOW_LEDGERS;
        env.storage()
            .persistent()
            .set(&DataKey::ClaimableAt(escrow_id), &claimable_at);

        // Update associated refund requests
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
            caller.clone(),
        );

        // Record resolution for each arbiter that participated
        for arbiter in config.arbiters.iter() {
            if env
                .storage()
                .persistent()
                .get::<DataKey, ArbiterVoteRecord>(&DataKey::ArbiterVote(
                    escrow_id,
                    arbiter.clone(),
                ))
                .is_some()
            {
                Self::record_arbiter_resolution(&env, &arbiter);
            }
        }

        // Clean up voting records
        for arbiter in config.arbiters.iter() {
            env.storage()
                .persistent()
                .remove(&DataKey::ArbiterVote(escrow_id, arbiter.clone()));
        }

        Ok(())
    }

    /// Set the minimum number of arbiters required for multi-arbiter escrows (admin only).
    pub fn set_min_arbiters_required(env: Env, min_arbiters: u32) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;

        if min_arbiters < 2 {
            return Err(ContractError::InvalidEscrowState);
        }

        env.storage()
            .persistent()
            .set(&DataKey::MinArbitersRequired, &min_arbiters);
        Ok(())
    }

    /// Get the minimum number of arbiters required.
    pub fn get_min_arbiters_required(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get::<_, u32>(&DataKey::MinArbitersRequired)
            .unwrap_or(DEFAULT_MIN_ARBITERS_REQUIRED)
    }

    /// Set the maximum number of arbiters allowed per escrow (admin only).
    pub fn set_max_arbiters_per_escrow(env: Env, max_arbiters: u32) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;

        if !(2..=20).contains(&max_arbiters) {
            return Err(ContractError::InvalidEscrowState);
        }

        env.storage()
            .persistent()
            .set(&DataKey::MaxArbitersPerEscrow, &max_arbiters);
        Ok(())
    }

    /// Get the maximum number of arbiters allowed per escrow.
    pub fn get_max_arbiters_per_escrow(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get::<_, u32>(&DataKey::MaxArbitersPerEscrow)
            .unwrap_or(DEFAULT_MAX_ARBITERS_PER_ESCROW)
    }

    /// Set the default quorum percentage for arbiter voting (admin only).
    /// For example, 5100 means 51% (divide by 100 to get percentage).
    pub fn set_default_quorum_pct(env: Env, percentage: u32) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;

        if !(5000..=10000).contains(&percentage) {
            // Between 50% and 100%
            return Err(ContractError::InvalidEscrowState);
        }

        env.storage()
            .persistent()
            .set(&DataKey::DefaultArbiterQuorumPercentage, &percentage);
        Ok(())
    }

    /// Get the default quorum percentage for arbiter voting.
    pub fn get_default_quorum_pct(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get::<_, u32>(&DataKey::DefaultArbiterQuorumPercentage)
            .unwrap_or(DEFAULT_ARBITER_QUORUM_PERCENTAGE)
    }
}
