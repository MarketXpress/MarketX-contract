#![no_std]

//! # MarketX Contract
//!
//! Soroban smart contract implementing escrow logic for the MarketX
//! decentralized marketplace on the Stellar network.
//!
//! ## Architecture
//!
//! The contract manages the full escrow lifecycle including on-chain token
//! custody. Funds flow through three stages:
//!
//! 1. **Funding** ([`fund_escrow`]) — buyer transfers tokens to the contract.
//! 2. **Release** ([`release_escrow`]) — contract transfers tokens to the
//!    seller (minus platform fee) and fee amount to the fee collector.
//! 3. **Refund** ([`refund_escrow`]) — contract returns the full token amount
//!    to the buyer.
//!
//! State transitions are enforced by [`EscrowStatus`] and the
//! [`transition_status`] helper. Token transfers and status updates within
//! [`release_escrow`] and [`refund_escrow`] are sequenced so that the status
//! only advances after a successful transfer — if the token call traps, the
//! ledger change is rolled back atomically by the Soroban runtime.
//!
//! ## Modules
//!
//! - [`errors`] — [`ContractError`] variants returned by fallible functions.
//! - [`types`]  — [`Escrow`], [`EscrowStatus`], and [`DataKey`] definitions.

use soroban_sdk::{contract, contractimpl, symbol_short, token, Address, Env, Vec};
mod errors;
mod types;
pub use errors::ContractError;
pub use types::{DataKey, Escrow, EscrowStatus, RefundHistoryEntry, RefundReason, RefundRequest, RefundStatus};

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    // ─── Initialization ──────────────────────────────────────────────────────

    /// Initialize the contract with platform fee configuration and admin address.
    ///
    /// Stores `admin`, `fee_collector` and `fee_bps` in persistent storage. Must be
    /// called before [`release_escrow`] is used. Can be called multiple times
    /// to update fee parameters — subsequent calls require admin authorization.
    /// Once the admin is set, it cannot be changed (immutable).
    ///
    /// # Arguments
    ///
    /// * `admin`         — address with admin privileges for fee management.
    /// * `fee_collector` — address that receives the platform fee on each
    ///   release. Typically a treasury or multisig wallet.
    /// * `fee_bps`       — platform fee in basis points (`0..=10_000`).
    ///   For example, `250` = 2.5 %. Pass `0` for a zero-fee deployment.
    ///
    /// # Errors
    ///
    /// - [`ContractError::InvalidFeeConfig`] — `fee_bps` exceeds 10 000.
    /// - [`ContractError::NotAdmin`] — admin already set and caller is not the admin.
    pub fn initialize(
        env: Env,
        admin: Address,
        fee_collector: Address,
        fee_bps: u32,
    ) -> Result<(), ContractError> {
        if fee_bps > 10_000 {
            return Err(ContractError::InvalidFeeConfig);
        }

        // Check if admin is already set - if so, require admin authorization for updates
        // Admin is immutable after initialization, so we just check auth
        if env.storage().persistent().has(&DataKey::Admin) {
            let current_admin = env
                .storage()
                .persistent()
                .get::<DataKey, Address>(&DataKey::Admin)
                .unwrap();
            current_admin.require_auth();
        } else {
            // First initialization - store the admin address
            env.storage()
                .persistent()
                .set(&DataKey::Admin, &admin);
        }

        env.storage()
            .persistent()
            .set(&DataKey::FeeCollector, &fee_collector);
        env.storage()
            .persistent()
            .set(&DataKey::FeeBps, &fee_bps);
        Ok(())
    }

    // ─── Escrow Storage ──────────────────────────────────────────────────────

    /// Persist a new escrow record under the given ID and emit an `escrow_cr`
    /// event with `escrow_id` as the second topic and the full [`Escrow`]
    /// payload as event data.
    ///
    /// Writes `escrow` to persistent storage keyed by `DataKey::Escrow(escrow_id)`.
    /// If a record already exists for `escrow_id` it is silently overwritten —
    /// callers are responsible for ID uniqueness. No authorization is required
    /// by this function directly; access control should be enforced at a higher
    /// layer or via a future `create_escrow` wrapper.
    ///
    /// # Arguments
    ///
    /// * `escrow_id` — caller-assigned unique identifier for this escrow.
    /// * `escrow`    — fully populated [`Escrow`] record to store.
    ///
    /// # Errors
    ///
    /// - [`ContractError::InvalidEscrowAmount`] — amount is zero or negative.
    #[allow(deprecated)]
    pub fn store_escrow(env: Env, escrow_id: u64, escrow: Escrow) -> Result<(), ContractError> {
        // Validate escrow amount must be positive
        if escrow.amount <= 0 {
            return Err(ContractError::InvalidEscrowAmount);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

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

        escrow.status = new_status;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);
        Ok(())
    }

    /// Resolve a dispute by transitioning to Released or Refunded.
    /// Only callable by the arbiter when escrow is in Disputed state.
    ///
    /// Errors:
    /// - `ContractError::EscrowNotFound`   — no record for `escrow_id`
    /// - `ContractError::Unauthorized`     — caller is not the arbiter
    /// - `ContractError::InvalidTransition` — escrow is not in Disputed state or invalid resolution
    pub fn resolve_dispute(
        env: Env,
        escrow_id: u64,
        resolution: EscrowStatus,
    ) -> Result<(), ContractError> {
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

        // Validate refund amount doesn't exceed escrow amount
        if refund_amount > escrow.amount {
            return Err(ContractError::RefundAmountExceedsEscrow);
        }

        // Check refund deadline
        let current_ledger = env.ledger().sequence();
        if escrow.refund_deadline > 0 && current_ledger > escrow.refund_deadline {
            return Err(ContractError::RefundWindowExpired);
        }

        // Validate partial refunds are allowed if not full refund
        if refund_amount < escrow.amount && !escrow.allow_partial_refund {
            return Err(ContractError::RefundAmountExceedsEscrow);
        }

        // Require buyer authorization
        escrow.buyer.require_auth();

        // Generate refund ID
        let refund_count: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::RefundCount)
            .unwrap_or(0);
        let refund_id = refund_count + 1;

        // Calculate expiration ledger (default 7 days = 20160 ledgers at 5s/ledger)
        let expires_at = escrow.refund_deadline.max(current_ledger + 20160);

        // Create refund request
        let refund_request = RefundRequest {
            refund_id,
            escrow_id,
            buyer: escrow.buyer.clone(),
            refund_amount,
            reason,
            description,
            status: RefundStatus::Pending,
            created_at: current_ledger,
            updated_at: current_ledger,
            expires_at,
            processed_by: None,
            processed_at: None,
            rejection_reason: None,
        };

        // Store refund request
        env.storage()
            .persistent()
            .set(&DataKey::RefundRequest(refund_id), &refund_request);

        // Update refund count
        env.storage()
            .persistent()
            .set(&DataKey::RefundCount, &refund_id);

        // Track refund ID in escrow's refund list
        let mut escrow_refunds: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowRefunds(escrow_id))
            .unwrap_or(Vec::new(&env));
        escrow_refunds.push_back(refund_id);
        env.storage()
            .persistent()
            .set(&DataKey::EscrowRefunds(escrow_id), &escrow_refunds);

        // Emit event
        env.events().publish(
            (symbol_short!("refund_req"), refund_id),
            refund_request,
        );

        Ok(refund_id)
    }

    /// Get a refund request by ID.
    pub fn get_refund_request(env: Env, refund_id: u64) -> Result<RefundRequest, ContractError> {
        env.storage()
            .persistent()
            .get(&DataKey::RefundRequest(refund_id))
            .ok_or(ContractError::RefundRequestNotFound)
    }

    /// Get all refund requests for an escrow.
    pub fn get_escrow_refunds(env: Env, escrow_id: u64) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::EscrowRefunds(escrow_id))
            .unwrap_or(Vec::new(&env))
    }

    // ─── Admin Refund Processing ─────────────────────────────────────────────

    /// Approve a refund request.
    ///
    /// Only callable by the admin. After approval, the refund can be processed.
    ///
    /// # Arguments
    ///
    /// * `refund_id` — identifier of the refund request to approve.
    ///
    /// # Errors
    ///
    /// - [`ContractError::RefundRequestNotFound`] — no refund request exists.
    /// - [`ContractError::RefundAlreadyProcessed`] — request already processed.
    /// - [`ContractError::NotAdmin`] — caller is not the admin.
    pub fn approve_refund_request(env: Env, refund_id: u64) -> Result<(), ContractError> {
        // Verify admin
        let admin = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Admin)
            .ok_or(ContractError::NotAdmin)?;
        admin.require_auth();

        // Get refund request
        let mut refund_request = env
            .storage()
            .persistent()
            .get::<DataKey, RefundRequest>(&DataKey::RefundRequest(refund_id))
            .ok_or(ContractError::RefundRequestNotFound)?;

        // Check not already processed
        if refund_request.status != RefundStatus::Pending {
            return Err(ContractError::RefundAlreadyProcessed);
        }

        // Update status
        let current_ledger = env.ledger().sequence();
        refund_request.status = RefundStatus::Approved;
        refund_request.updated_at = current_ledger;
        refund_request.processed_by = Some(admin.clone());
        refund_request.processed_at = Some(current_ledger);

        env.storage()
            .persistent()
            .set(&DataKey::RefundRequest(refund_id), &refund_request);

        // Emit event
        env.events().publish(
            (symbol_short!("refund_appr"), refund_id),
            refund_request,
        );

        Ok(())
    }

    /// Reject a refund request.
    ///
    /// Only callable by the admin.
    ///
    /// # Arguments
    ///
    /// * `refund_id` — identifier of the refund request to reject.
    /// * `reason` — reason for rejection.
    ///
    /// # Errors
    ///
    /// - [`ContractError::RefundRequestNotFound`] — no refund request exists.
    /// - [`ContractError::RefundAlreadyProcessed`] — request already processed.
    /// - [`ContractError::NotAdmin`] — caller is not the admin.
    pub fn reject_refund_request(
        env: Env,
        refund_id: u64,
        reason: String,
    ) -> Result<(), ContractError> {
        // Verify admin
        let admin = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Admin)
            .ok_or(ContractError::NotAdmin)?;
        admin.require_auth();

        // Get refund request
        let mut refund_request = env
            .storage()
            .persistent()
            .get::<DataKey, RefundRequest>(&DataKey::RefundRequest(refund_id))
            .ok_or(ContractError::RefundRequestNotFound)?;

        // Check not already processed
        if refund_request.status != RefundStatus::Pending {
            return Err(ContractError::RefundAlreadyProcessed);
        }

        // Update status
        let current_ledger = env.ledger().sequence();
        refund_request.status = RefundStatus::Rejected;
        refund_request.updated_at = current_ledger;
        refund_request.processed_by = Some(admin.clone());
        refund_request.processed_at = Some(current_ledger);
        refund_request.rejection_reason = Some(reason);

        env.storage()
            .persistent()
            .set(&DataKey::RefundRequest(refund_id), &refund_request);

        // Emit event
        env.events().publish(
            (symbol_short!("refund_rej"), refund_id),
            refund_request,
        );

        Ok(())
    }

    /// Process an approved refund and transfer tokens back to buyer.
    ///
    /// This triggers the Stellar refund transaction. Only callable by admin
    /// after refund request has been approved.
    ///
    /// # Arguments
    ///
    /// * `refund_id` — identifier of the refund request to process.
    ///
    /// # Errors
    ///
    /// - [`ContractError::RefundRequestNotFound`] — no refund request exists.
    /// - [`ContractError::RefundAlreadyProcessed`] — request not in approved state.
    /// - [`ContractError::NotAdmin`] — caller is not the admin.
    pub fn process_refund(env: Env, refund_id: u64) -> Result<(), ContractError> {
        // Verify admin
        let admin = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Admin)
            .ok_or(ContractError::NotAdmin)?;
        admin.require_auth();

        // Get refund request
        let mut refund_request = env
            .storage()
            .persistent()
            .get::<DataKey, RefundRequest>(&DataKey::RefundRequest(refund_id))
            .ok_or(ContractError::RefundRequestNotFound)?;

        // Must be in approved state
        if refund_request.status != RefundStatus::Approved {
            return Err(ContractError::RefundAlreadyProcessed);
        }

        // Get escrow
        let mut escrow = env
            .storage()
            .persistent()
            .get::<DataKey, Escrow>(&DataKey::Escrow(refund_request.escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // Execute the Stellar token transfer (refund transaction)
        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &env.current_contract_address(),
            &refund_request.buyer,
            &refund_request.refund_amount,
        );

        // Update escrow amount if partial refund
        if refund_request.refund_amount < escrow.amount {
            escrow.amount -= refund_request.refund_amount;
        } else {
            // Full refund - update escrow status
            escrow.status = EscrowStatus::Refunded;
        }

        // Persist updated escrow
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(refund_request.escrow_id), &escrow);

        // Update refund request status
        let current_ledger = env.ledger().sequence();
        refund_request.status = RefundStatus::Completed;
        refund_request.updated_at = current_ledger;

        env.storage()
            .persistent()
            .set(&DataKey::RefundRequest(refund_id), &refund_request);

        // Record in refund history
        let is_full_refund = refund_request.refund_amount >= escrow.amount;
        let history_entry = RefundHistoryEntry {
            refund_id,
            escrow_id: refund_request.escrow_id,
            amount: refund_request.refund_amount,
            is_full_refund,
            processed_at: current_ledger,
            processed_by: admin,
        };

        // Add to escrow-specific history
        let mut escrow_history: Vec<RefundHistoryEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::RefundHistory(refund_request.escrow_id))
            .unwrap_or(Vec::new(&env));
        escrow_history.push_back(history_entry.clone());
        env.storage()
            .persistent()
            .set(&DataKey::RefundHistory(refund_request.escrow_id), &escrow_history);

        // Add to global history
        let mut global_history: Vec<RefundHistoryEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::GlobalRefundHistory)
            .unwrap_or(Vec::new(&env));
        global_history.push_back(history_entry);
        env.storage()
            .persistent()
            .set(&DataKey::GlobalRefundHistory, &global_history);

        // Emit completion event
        env.events().publish(
            (symbol_short!("refund_done"), refund_id),
            refund_request,
        );

        Ok(())
    }

    // ─── Refund History Functions ─────────────────────────────────────────────

    /// Get refund history for a specific escrow.
    pub fn get_refund_history(env: Env, escrow_id: u64) -> Vec<RefundHistoryEntry> {
        env.storage()
            .persistent()
            .get(&DataKey::RefundHistory(escrow_id))
            .unwrap_or(Vec::new(&env))
    }

    /// Get all refund history (global).
    pub fn get_all_refund_history(env: Env) -> Vec<RefundHistoryEntry> {
        env.storage()
            .persistent()
            .get(&DataKey::GlobalRefundHistory)
            .unwrap_or(Vec::new(&env))
    }

    /// Cancel a pending refund request (buyer can cancel before approval).
    ///
    /// # Arguments
    ///
    /// * `refund_id` — identifier of the refund request to cancel.
    ///
    /// # Errors
    ///
    /// - [`ContractError::RefundRequestNotFound`] — no refund request exists.
    /// - [`ContractError::RefundAlreadyProcessed`] — request already processed.
    /// - [`ContractError::Unauthorized`] — caller is not the buyer.
    pub fn cancel_refund_request(env: Env, refund_id: u64) -> Result<(), ContractError> {
        // Get refund request
        let mut refund_request = env
            .storage()
            .persistent()
            .get::<DataKey, RefundRequest>(&DataKey::RefundRequest(refund_id))
            .ok_or(ContractError::RefundRequestNotFound)?;

        // Check status is pending
        if refund_request.status != RefundStatus::Pending {
            return Err(ContractError::RefundAlreadyProcessed);
        }

        // Require buyer authorization
        refund_request.buyer.require_auth();

        // Update status
        let current_ledger = env.ledger().sequence();
        refund_request.status = RefundStatus::Cancelled;
        refund_request.updated_at = current_ledger;

        env.storage()
            .persistent()
            .set(&DataKey::RefundRequest(refund_id), &refund_request);

        // Emit event
        env.events().publish(
            (symbol_short!("refund_canc"), refund_id),
            refund_request,
        );

        Ok(())
    }
}
}
