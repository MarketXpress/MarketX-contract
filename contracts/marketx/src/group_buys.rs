//! group buys operations for the MarketX escrow contract.

use crate::*;
use soroban_sdk::*;

#[contractimpl]
impl Contract {
    // =========================
    // 👥 GROUP BUY ESCROW (#175)
    // =========================

    /// Create a group buy escrow where multiple buyers contribute to a single purchase.
    ///
    /// # Arguments
    /// * `seller` - The seller's address
    /// * `token` - The token contract address
    /// * `target_amount` - The total amount needed
    /// * `buyers` - Vector of buyer contributions
    /// * `funding_deadline` - Ledger sequence number deadline for funding
    /// * `metadata` - Optional metadata
    /// * `arbiter` - Optional arbiter
    ///
    /// # Errors
    /// * `TooManyItems` - If `buyers` exceeds `MAX_GROUP_BUY_BUYERS`
    /// * `InvalidGroupBuyAmount` - If buyer contributions don't sum to target amount
    pub fn create_group_buy_escrow(
        env: Env,
        seller: Address,
        token: Address,
        target_amount: i128,
        buyers: Vec<BuyerContribution>,
        funding_deadline: u32,
        metadata: Option<Bytes>,
        arbiter: Option<Address>,
    ) -> Result<u64, ContractError> {
        Self::assert_not_paused(&env)?;

        if buyers.len() > MAX_GROUP_BUY_BUYERS {
            return Err(ContractError::TooManyItems);
        }

        // Validate buyer contributions sum to target
        let contributions_sum: i128 = buyers.iter().map(|b| b.amount).sum();
        if contributions_sum != target_amount {
            return Err(ContractError::InvalidGroupBuyAmount);
        }

        // Use first buyer as primary buyer for escrow creation
        let primary_buyer = buyers
            .get(0)
            .ok_or(ContractError::InvalidGroupBuyAmount)?
            .buyer
            .clone();
        primary_buyer.require_auth();

        let escrow_id = Self::create_escrow_internal(
            env.clone(),
            primary_buyer,
            seller,
            token.clone(),
            target_amount,
            metadata,
            arbiter,
            None,
            None,
        )?;

        // Create group buy configuration
        let group_buy = GroupBuy {
            buyers: buyers.clone(),
            target_amount,
            funded_amount: 0,
            funding_deadline,
        };

        // Update escrow with group buy config. `escrow_id` was just returned
        // by `create_escrow_internal`, which stores the record before
        // returning, so this lookup cannot miss in practice; the typed
        // error is defensive rather than reachable.
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;
        let mut gb_vec = Vec::new(&env);
        gb_vec.push_back(group_buy.clone());
        escrow.group_buy = gb_vec;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);
        env.storage()
            .persistent()
            .set(&DataKey::GroupBuyEscrow(escrow_id), &group_buy);

        Ok(escrow_id)
    }

    /// Fund a group buy escrow as one of the buyers.
    ///
    /// # Arguments
    /// * `escrow_id` - The escrow ID
    /// * `buyer` - The buyer's address
    ///
    /// # Errors
    /// * `EscrowNotFound` - If escrow doesn't exist
    /// * `GroupBuyDeadlinePassed` - If funding deadline has passed
    /// * `GroupBuyAlreadyFunded` - If buyer has already funded
    /// * `Unauthorized` - If caller is not a registered buyer
    pub fn fund_group_buy(env: Env, escrow_id: u64, buyer: Address) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        buyer.require_auth();

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Pending && escrow.status != EscrowStatus::Funded {
            return Err(ContractError::InvalidEscrowState);
        }

        let mut group_buy = escrow
            .group_buy
            .get(0)
            .ok_or(ContractError::InvalidEscrowState)?;

        // Check deadline
        if env.ledger().sequence() > group_buy.funding_deadline {
            return Err(ContractError::GroupBuyDeadlinePassed);
        }

        // Find buyer in contributions list
        let mut buyer_index: Option<u32> = None;
        let mut buyer_amount: i128 = 0;

        for (i, contribution) in group_buy.buyers.iter().enumerate() {
            if contribution.buyer == buyer {
                if contribution.funded {
                    return Err(ContractError::GroupBuyAlreadyFunded);
                }
                buyer_index = Some(i as u32);
                buyer_amount = contribution.amount;
                break;
            }
        }

        let index = buyer_index.ok_or(ContractError::Unauthorized)?;

        // Transfer funds from buyer to contract
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        token_client.transfer(&buyer, env.current_contract_address(), &buyer_amount);

        // Update buyer contribution. `index` was just found via `enumerate()`
        // over this same vec, so it is always in bounds; the typed error is
        // defensive rather than reachable.
        let mut contribution = group_buy
            .buyers
            .get(index)
            .ok_or(ContractError::Unauthorized)?;
        contribution.funded = true;
        group_buy.buyers.set(index, contribution);
        group_buy.funded_amount += buyer_amount;

        // Update escrow
        let mut gb_vec = Vec::new(&env);
        gb_vec.push_back(group_buy.clone());
        escrow.group_buy = gb_vec;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        GroupBuyFundedEvent {
            escrow_id,
            buyer: buyer.clone(),
            amount: buyer_amount,
        }
        .publish(&env);

        // Check if fully funded
        if group_buy.funded_amount >= group_buy.target_amount {
            GroupBuyCompletedEvent {
                escrow_id,
                total_amount: group_buy.funded_amount,
            }
            .publish(&env);
        }

        Self::add_i128(&env, DataKey::TotalFundedAmount, buyer_amount);

        Ok(())
    }

    /// Withdraw a contribution from a failed group buy escrow.
    ///
    /// # Arguments
    /// * `escrow_id` - The escrow ID
    /// * `buyer` - The buyer's address
    ///
    /// # Errors
    /// * `EscrowNotFound` - If escrow doesn't exist
    /// * `GroupBuyDeadlineNotReached` - If funding deadline hasn't passed
    /// * `GroupBuyAlreadyFunded` - If group buy was successfully funded
    pub fn withdraw_group_buy_contribution(
        env: Env,
        escrow_id: u64,
        buyer: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        buyer.require_auth();

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        let mut group_buy = escrow
            .group_buy
            .get(0)
            .ok_or(ContractError::InvalidEscrowState)?;

        // Only allow withdrawal if deadline passed and target NOT met
        if env.ledger().sequence() <= group_buy.funding_deadline {
            return Err(ContractError::GroupBuyDeadlineNotReached);
        }

        if group_buy.funded_amount >= group_buy.target_amount {
            return Err(ContractError::InvalidEscrowState);
        }

        // Find buyer and their contribution
        let mut buyer_index: Option<u32> = None;
        let mut buyer_amount: i128 = 0;

        for (i, contribution) in group_buy.buyers.iter().enumerate() {
            if contribution.buyer == buyer {
                if !contribution.funded {
                    return Err(ContractError::Unauthorized);
                }
                buyer_index = Some(i as u32);
                buyer_amount = contribution.amount;
                break;
            }
        }

        let index = buyer_index.ok_or(ContractError::Unauthorized)?;

        // Update state first (Effect). `index` was just found via
        // `enumerate()` over this same vec, so it is always in bounds; the
        // typed error is defensive rather than reachable.
        let mut contribution = group_buy
            .buyers
            .get(index)
            .ok_or(ContractError::Unauthorized)?;
        contribution.funded = false;
        group_buy.buyers.set(index, contribution);
        group_buy.funded_amount -= buyer_amount;

        let mut gb_vec = Vec::new(&env);
        gb_vec.push_back(group_buy.clone());
        escrow.group_buy = gb_vec;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        // Refund the buyer (Interaction)
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        token_client.transfer(&env.current_contract_address(), &buyer, &buyer_amount);

        // Update global counter
        let current_total: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalFundedAmount)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalFundedAmount, &(current_total - buyer_amount));

        Ok(())
    }

    /// Get group buy configuration for an escrow.
    pub fn get_group_buy(env: Env, escrow_id: u64) -> Option<GroupBuy> {
        let escrow: Option<Escrow> = env.storage().persistent().get(&DataKey::Escrow(escrow_id));
        escrow.and_then(|e| e.group_buy.get(0))
    }

    pub(crate) fn emit_status_change(
        env: &Env,
        escrow_id: u64,
        from: EscrowStatus,
        to: EscrowStatus,
        actor: Address,
    ) {
        StatusChangeEvent {
            escrow_id,
            from_status: from,
            to_status: to,
            actor,
        }
        .publish(env);
    }
}
