#![no_std]

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Vec};

mod errors;
mod types;

pub use errors::ContractError;
pub use types::{
    DataKey, Escrow, EscrowStatus, RefundHistoryEntry, RefundReason, RefundRequest, RefundStatus,
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

        env.events()
            .publish((symbol_short!("escrow_cr"), escrow_id), escrow);
        // Track escrow ID for pagination
        let mut escrow_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowIds)
            .unwrap_or(Vec::new(&env));
        escrow_ids.push_back(escrow_id);
        env.storage()
            .persistent()
            .set(&DataKey::EscrowIds, &escrow_ids);

        env.events().publish(
            (symbol_short!("escrow_cr"), escrow_id),
            escrow,
        );
        Ok(())
    }

    /// Retrieve an escrow record by ID.
    ///
    /// # Panics
    ///
    /// Panics (contract trap) if no record exists for `escrow_id`. Prefer
    /// [`try_get_escrow`] for cases where the ID may not exist.
    ///
    /// # Arguments
    ///
    /// * `escrow_id` — identifier previously passed to [`store_escrow`].
    pub fn get_escrow(env: Env, escrow_id: u64) -> Escrow {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .unwrap()
    }

    /// Safely retrieve an escrow record by ID, returning a [`Result`].
    ///
    /// Prefer this over [`get_escrow`] when the caller cannot guarantee that
    /// `escrow_id` has been stored — it avoids a contract trap on a missing key.
    ///
    /// # Arguments
    ///
    /// * `escrow_id` — identifier previously passed to [`store_escrow`].
    ///
    /// # Errors
    ///
    /// - [`ContractError::EscrowNotFound`] — no escrow exists for `escrow_id`.
    pub fn try_get_escrow(env: Env, escrow_id: u64) -> Result<Escrow, ContractError> {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)
    }

    /// Get all escrow IDs with pagination.
    ///
    /// Returns a slice of escrow IDs from the stored vector, starting at
    /// index `start` with up to `limit` elements. Handles out-of-bounds
    /// gracefully by returning an empty list when the range is invalid.
    ///
    /// # Arguments
    ///
    /// * `start` — starting index for pagination (0-based).
    /// * `limit` — maximum number of IDs to return.
    ///
    /// # Returns
    ///
    /// Vector of escrow IDs within the specified range, or empty vector
    /// if start exceeds the total count.
    pub fn get_escrow_ids(env: Env, start: u32, limit: u32) -> Vec<u64> {
        let escrow_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowIds)
            .unwrap_or(Vec::new(&env));

        let total_count = escrow_ids.len();

        // Handle out-of-bounds: return empty vector if start >= total
        if start >= total_count {
            return Vec::new(&env);
        }

        // Calculate end index (capped at total_count)
        let end = (start + limit).min(total_count);

        // Extract the requested slice
        let mut result = Vec::new(&env);
        let start_idx = start as u64;
        let end_idx = end as u64;

        for i in start_idx..end_idx {
            if let Some(id) = escrow_ids.get(i) {
                result.push_back(id);
            }
        }

        result
    }

    // ─── Token Operations ────────────────────────────────────────────────────

    /// Transfer escrowed tokens from the buyer to the contract (fund escrow).
    ///
    /// Requires buyer authorization. After this call the contract holds
    /// `escrow.amount` tokens on behalf of the escrow. The escrow status
    /// remains `Pending` — funding is tracked by the contract's token balance
    /// rather than a separate status flag.
    ///
    /// Calling `fund_escrow` a second time on the same escrow (or on one that
    /// is no longer `Pending`) is rejected with [`ContractError::AlreadyFunded`].
    ///
    /// # Arguments
    ///
    /// * `escrow_id` — identifier of the escrow to fund.
    ///
    /// # Errors
    ///
    /// - [`ContractError::EscrowNotFound`] — no record for `escrow_id`.
    /// - [`ContractError::AlreadyFunded`]  — escrow is not in `Pending` state.
    pub fn fund_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        let escrow = env
            .storage()
            .persistent()
            .get::<DataKey, Escrow>(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::AlreadyFunded);
        }

        // Require explicit buyer authorization at the contract level.
        // The token client will also enforce auth on the token contract,
        // but declaring it here makes the intent auditable in this contract's
        // authorization footprint.
        escrow.buyer.require_auth();

        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &escrow.buyer,
            &env.current_contract_address(),
            &escrow.amount,
        );

        Ok(())
    }

    /// Release funds to the seller, deducting the platform fee.
    ///
    /// Requires buyer authorization. Validates that the escrow is in `Pending`
    /// state, then:
    ///
    /// 1. Computes `fee_amount = escrow.amount * fee_bps / 10_000`.
    /// 2. Transfers `escrow.amount - fee_amount` from the contract to the seller.
    /// 3. Transfers `fee_amount` from the contract to the fee collector
    ///    (skipped when `fee_bps` is zero).
    /// 4. Updates the escrow status to `Released`.
    ///
    /// The status write occurs after the token transfers so that a trap in the
    /// token contract rolls back the entire ledger change atomically.
    ///
    /// # Arguments
    ///
    /// * `escrow_id` — identifier of the escrow to release.
    ///
    /// # Errors
    ///
    /// - [`ContractError::EscrowNotFound`]  — no record for `escrow_id`.
    /// - [`ContractError::EscrowNotFunded`] — escrow is not in `Pending` state.
    pub fn release_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        let mut escrow = env
            .storage()
            .persistent()
            .get::<DataKey, Escrow>(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::EscrowNotFunded);
        }

        escrow.buyer.require_auth();

        let fee_bps: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::FeeBps)
            .unwrap_or(0);

        let fee_collector: Address = env
            .storage()
            .persistent()
            .get(&DataKey::FeeCollector)
            .unwrap();

        // Integer arithmetic: fee_bps is at most 10_000, amount is i128.
        // Cast fee_bps to i128 to avoid overflow in the multiplication.
        let fee_amount = escrow.amount * fee_bps as i128 / 10_000;
        let seller_amount = escrow.amount - fee_amount;

        let token_client = token::Client::new(&env, &escrow.token);
        let contract_address = env.current_contract_address();

        // Transfer seller's share first; if this traps the status never flips.
        token_client.transfer(&contract_address, &escrow.seller, &seller_amount);

        // Transfer platform fee only when non-zero (avoids a no-op token call).
        if fee_amount > 0 {
            token_client.transfer(&contract_address, &fee_collector, &fee_amount);
        }

        // Status advances only after all transfers succeed.
        escrow.status = EscrowStatus::Released;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Ok(())
    }

    /// Return the full escrowed amount to the buyer.
    ///
    /// Valid from `Pending` or `Disputed` states. Authorization rules:
    ///
    /// - **`Pending`** — either the buyer or the seller may initiate the
    ///   refund. The seller path covers voluntary order cancellation; the
    ///   buyer path covers pre-dispute cancellation.
    /// - **`Disputed`** — only the buyer may initiate the refund (dispute
    ///   resolved in the buyer's favour).
    ///
    /// The `initiator` parameter must be the buyer or seller address. The
    /// caller must have authorized this invocation as `initiator` (i.e.
    /// `initiator` must have signed the transaction). This is the idiomatic
    /// Soroban pattern for "caller is one of N addresses" — the contract
    /// validates identity and then calls `initiator.require_auth()`.
    ///
    /// After a successful transfer the escrow status is updated to `Refunded`.
    /// The full `escrow.amount` is returned; no platform fee is deducted on
    /// refunds.
    ///
    /// # Arguments
    ///
    /// * `escrow_id` — identifier of the escrow to refund.
    /// * `initiator` — address of the party initiating the refund (must be
    ///   buyer or seller).
    ///
    /// # Errors
    ///
    /// - [`ContractError::EscrowNotFound`]    — no record for `escrow_id`.
    /// - [`ContractError::InvalidTransition`] — escrow is not in `Pending` or
    ///   `Disputed` state (i.e. already `Released` or `Refunded`).
    /// - [`ContractError::Unauthorized`]      — `initiator` is not the buyer
    ///   or seller, or is the seller on a `Disputed` escrow.
    pub fn refund_escrow(env: Env, escrow_id: u64, initiator: Address) -> Result<(), ContractError> {
        let mut escrow = env
            .storage()
            .persistent()
            .get::<DataKey, Escrow>(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        match &escrow.status {
            EscrowStatus::Pending => {
                // From Pending, either the buyer or seller may initiate a refund.
                // The seller path covers voluntary order cancellation; the buyer
                // path covers pre-dispute cancellation. The idiomatic Soroban
                // pattern for "caller is one of two addresses" is to receive the
                // initiator as a parameter, validate it against the allowed set,
                // then call require_auth() on it — which proves the initiator
                // signed this invocation.
                if initiator != escrow.buyer && initiator != escrow.seller {
                    return Err(ContractError::Unauthorized);
                }
                initiator.require_auth();
            }
            EscrowStatus::Disputed => {
                // From Disputed, only the buyer may claim a refund.
                if initiator != escrow.buyer {
                    return Err(ContractError::Unauthorized);
                }
                initiator.require_auth();
            }
            _ => {
                // Released or Refunded — terminal state, no refund possible.
                return Err(ContractError::InvalidTransition);
            }
        }

        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.buyer,
            &escrow.amount,
        );

        // Status advances only after the transfer succeeds.
        escrow.status = EscrowStatus::Refunded;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Ok(())
    }

    // ─── State Transitions ───────────────────────────────────────────────────

    /// Transition an escrow to a new status, enforcing the valid state graph.
    ///
    /// Emits an `EscrowStatusUpdated` event on successful transition.
    ///
    /// Errors:
    /// - `ContractError::EscrowNotFound`   — no record for `escrow_id`
    /// - `ContractError::InvalidTransition` — move not permitted from current state
    /// This is the low-level state-mutation helper used for status-only changes
    /// (e.g. `Pending → Disputed`). Token-bearing transitions (`Released`,
    /// `Refunded`) should use [`release_escrow`] and [`refund_escrow`]
    /// respectively, which perform the token transfer before advancing the state.
    ///
    /// Steps:
    ///
    /// 1. Loads the escrow record (errors if missing).
    /// 2. Validates the transition via [`EscrowStatus::can_transition_to`].
    /// 3. Requires buyer authorization for all buyer-initiated moves:
    ///    `Pending → Released`, `Pending → Disputed`, `Pending → Refunded`,
    ///    and `Disputed → Refunded`.
    /// 4. Persists the updated record.
    ///
    /// Validation occurs before authorization so that invalid transitions are
    /// rejected cheaply without triggering an unnecessary auth check.
    /// Authorization is **not** required for `Disputed → Released` to allow
    /// an external resolver role in a future iteration.
    ///
    /// # Arguments
    ///
    /// * `escrow_id`  — identifier of the escrow to update.
    /// * `new_status` — target [`EscrowStatus`] to transition into.
    ///
    /// # Errors
    ///
    /// - [`ContractError::EscrowNotFound`]    — no record for `escrow_id`
    /// - [`ContractError::InvalidTransition`] — move not permitted from the
    ///   current state (includes self-transitions and exits from terminal states).
    pub fn transition_status(
        env: Env,
        escrow_id: u64,
        new_status: EscrowStatus,
    ) -> Result<(), ContractError> {
        let mut escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Validate before auth so invalid transitions are rejected without
        // triggering an unnecessary auth check.
        if !escrow.status.can_transition_to(&new_status) {
            return Err(ContractError::InvalidTransition);
        }

        let old_status = escrow.status.clone();
        escrow.status = new_status.clone();
        // Require buyer authorization for buyer-initiated transitions.
        if matches!(
            (&escrow.status, &new_status),
            (EscrowStatus::Pending, EscrowStatus::Released)
                | (EscrowStatus::Pending, EscrowStatus::Disputed)
                | (EscrowStatus::Pending, EscrowStatus::Refunded)
                | (EscrowStatus::Disputed, EscrowStatus::Refunded)
        ) {
            escrow.buyer.require_auth();
        }

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
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Require arbiter authorization
        escrow.arbiter.require_auth();

        // Must be in Disputed state
        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidTransition);
        }

        // Resolution must be either Released or Refunded
        if !matches!(resolution, EscrowStatus::Released | EscrowStatus::Refunded) {
            return Err(ContractError::InvalidTransition);
        }

        escrow.status = resolution;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        // Emit the status update event
        env.events().publish(
            ("EscrowStatusUpdated",),
            EscrowStatusUpdated {
                escrow_id,
                old_status,
                new_status,
            },
        );

        Ok(())
    }

    /// Initialize the contract with an initial value.
    pub fn initialize(env: Env, initial_value: u32) {
        env.storage()
            .persistent()
            .set(&DataKey::InitialValue, &initial_value);
    }

    /// Get the initial value.
    pub fn get_initial_value(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::InitialValue)
            .unwrap_or(0)
    }

    // ─── Admin Functions ─────────────────────────────────────────────────────

    /// Set the admin address for the contract.
    ///
    /// The admin can approve or reject refund requests. Requires existing admin
    /// authorization or no admin set (initial setup).
    ///
    /// # Arguments
    ///
    /// * `admin` — address to set as the contract admin.
    ///
    /// # Errors
    ///
    /// - [`ContractError::Unauthorized`] — caller is not the current admin.
    pub fn set_admin(env: Env, admin: Address) -> Result<(), ContractError> {
        // Check if admin already exists, if so require admin auth
        if let Some(current_admin) = env.storage().persistent().get::<DataKey, Address>(&DataKey::Admin) {
            current_admin.require_auth();
        }
        
        env.storage()
            .persistent()
            .set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Get the current admin address.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::Admin)
    }

    // ─── Fee Management ────────────────────────────────────────────────────────

    /// Set the platform fee percentage (basis points).
    ///
    /// Only callable by the admin. Validates that the fee is within the allowed
    /// range (0-1000 bps = 0-10%). Emits an event on successful fee change.
    ///
    /// # Arguments
    ///
    /// * `fee_bps` — new platform fee in basis points (`0..=1000`).
    ///   For example, `250` = 2.5 %.
    ///
    /// # Errors
    ///
    /// - [`ContractError::NotAdmin`] — caller is not the admin.
    /// - [`ContractError::InvalidFeeConfig`] — `fee_bps` exceeds 1000.
    pub fn set_fee_percentage(env: Env, fee_bps: u32) -> Result<(), ContractError> {
        // Verify admin
        let admin = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Admin)
            .ok_or(ContractError::NotAdmin)?;
        admin.require_auth();

        // Validate fee is within allowed range (max 10% = 1000 bps)
        if fee_bps > 1000 {
            return Err(ContractError::InvalidFeeConfig);
        }

        // Store the new fee
        env.storage()
            .persistent()
            .set(&DataKey::FeeBps, &fee_bps);

        // Emit event for fee change
        env.events().publish(
            symbol_short!("fee_chg"),
            fee_bps,
        );

        Ok(())
    }

    /// Get the current fee percentage in basis points.
    pub fn get_fee_bps(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::FeeBps)
            .unwrap_or(0)
    }

    // ─── Refund Request Functions ───────────────────────────────────────────

    /// Submit a refund request for an escrow.
    ///
    /// Buyers can request a refund within the specified refund deadline.
    /// Supports both full and partial refunds based on escrow configuration.
    ///
    /// # Arguments
    ///
    /// * `escrow_id` — identifier of the escrow to request refund for.
    /// * `refund_amount` — amount to refund (must be positive and <= escrow amount).
    /// * `reason` — reason for the refund request.
    /// * `description` — additional details about the refund request.
    ///
    /// # Errors
    ///
    /// - [`ContractError::EscrowNotFound`] — no escrow exists for `escrow_id`.
    /// - [`ContractError::RefundAmountExceedsEscrow`] — refund amount exceeds escrow amount.
    /// - [`ContractError::RefundWindowExpired`] — refund deadline has passed.
    /// - [`ContractError::InvalidTransition`] — escrow is not in a refundable state.
    pub fn submit_refund_request(
        env: Env,
        escrow_id: u64,
        refund_amount: i128,
        reason: RefundReason,
        description: String,
    ) -> Result<u64, ContractError> {
        let escrow = env
            .storage()
            .persistent()
            .get::<DataKey, Escrow>(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Validate escrow is in a refundable state
        if escrow.status != EscrowStatus::Pending && escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidTransition);
        }

        // Validate refund amount is positive
        if refund_amount <= 0 {
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

        env.events()
            .publish((symbol_short!("escrow_cr"), escrow_id), escrow);

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
            env.events()
                .publish((symbol_short!("escrow_cr"), escrow_id), escrow);

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

        escrow.status = EscrowStatus::Refunded;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

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
        escrow.released_amount = new_total;
        if escrow.released_amount == escrow.amount {
            escrow.status = EscrowStatus::Released;
        }

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        env.events().publish(
            (symbol_short!("escrow_rl"), escrow_id),
            (seller_payout, fee, escrow.released_amount),
        );

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
}