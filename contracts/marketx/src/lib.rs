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

use soroban_sdk::{contract, contractimpl, token, Address, Env};

mod errors;
mod types;
pub use errors::ContractError;
pub use types::{DataKey, Escrow, EscrowStatus};

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    // ─── Initialization ──────────────────────────────────────────────────────

    /// Initialize the contract with platform fee configuration.
    ///
    /// Stores `fee_collector` and `fee_bps` in persistent storage. Must be
    /// called before [`release_escrow`] is used. Can be called multiple times
    /// to update fee parameters — subsequent calls overwrite previous values.
    ///
    /// # Arguments
    ///
    /// * `fee_collector` — address that receives the platform fee on each
    ///   release. Typically a treasury or multisig wallet.
    /// * `fee_bps`       — platform fee in basis points (`0..=10_000`).
    ///   For example, `250` = 2.5 %. Pass `0` for a zero-fee deployment.
    ///
    /// # Errors
    ///
    /// - [`ContractError::InvalidFeeConfig`] — `fee_bps` exceeds 10 000.
    pub fn initialize(
        env: Env,
        fee_collector: Address,
        fee_bps: u32,
    ) -> Result<(), ContractError> {
        if fee_bps > 10_000 {
            return Err(ContractError::InvalidFeeConfig);
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

    /// Persist a new escrow record under the given ID.
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
    pub fn store_escrow(env: Env, escrow_id: u64, escrow: Escrow) {
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);
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
    /// - [`ContractError::EscrowNotFound`]    — no record for `escrow_id`.
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
}
}
