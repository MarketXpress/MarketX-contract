#![no_std]

use soroban_sdk::{
    contract, contractimpl, panic_with_error, Address, Bytes, Env, Symbol, Vec,
};

mod errors;
mod types;

use errors::ContractError;
use types::{
    DataKey, Escrow, EscrowCreatedEvent, EscrowStatus, FundsReleasedEvent, RefundHistoryEntry,
    RefundReason, RefundRequest, RefundStatus, StatusChangeEvent, MAX_METADATA_SIZE,
};

#[cfg(test)]
mod test;

#[contract]
pub struct Contract;

impl Contract {
    // =========================
    // 🔐 INTERNAL GUARDS
    // =========================

    fn assert_admin(env: &Env) -> Result<Address, ContractError> {
        let admin = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Admin)
            .ok_or(ContractError::NotAdmin)?;

        admin.require_auth();
        Ok(admin)
    }

    fn assert_not_paused(env: &Env) -> Result<(), ContractError> {
        let paused: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Paused)
            .unwrap_or(false);

        if paused {
            Err(ContractError::ContractPaused)
        } else {
            Ok(())
        }
    }

    fn next_escrow_id(env: &Env) -> Result<u64, ContractError> {
        let current: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0);

        let next = current
            .checked_add(1)
            .ok_or(ContractError::EscrowIdOverflow)?;

        env.storage()
            .persistent()
            .set(&DataKey::EscrowCounter, &next);

        Ok(next)
    }

    fn validate_metadata(metadata: &Option<Bytes>) -> Result<(), ContractError> {
        if let Some(ref data) = metadata {
            if data.len() > MAX_METADATA_SIZE {
                return Err(ContractError::MetadataTooLarge);
            }
        }
        Ok(())
    }
}

#[contractimpl]
impl Contract {
    // =========================
    // 🚀 INITIALIZATION
    // =========================

    pub fn initialize(
        env: Env,
        admin: Address,
        fee_collector: Address,
        fee_bps: u32,
    ) {
        admin.require_auth();

        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::FeeCollector, &fee_collector);
        env.storage().persistent().set(&DataKey::FeeBps, &fee_bps);

        // 🔒 Circuit breaker default
        env.storage().persistent().set(&DataKey::Paused, &false);

        // 🔢 Counter starts at 0
        env.storage().persistent().set(&DataKey::EscrowCounter, &0u64);
    }

    // =========================
    // 🔒 CIRCUIT BREAKER
    // =========================

    pub fn pause(env: Env) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage().persistent().set(&DataKey::Paused, &true);
        Ok(())
    }

    pub fn unpause(env: Env) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage().persistent().set(&DataKey::Paused, &false);
        Ok(())
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    // =========================
    // 💰 ESCROW ACTIONS
    // =========================

    /// Create a new escrow with optional metadata.
    ///
    /// # Arguments
    /// * `buyer` - The buyer's address
    /// * `seller` - The seller's address
    /// * `token` - The token contract address
    /// * `amount` - The escrow amount
    /// * `metadata` - Optional metadata (max 1KB)
    ///
    /// # Errors
    /// * `MetadataTooLarge` - If metadata exceeds 1KB
    pub fn create_escrow(
        env: Env,
        buyer: Address,
        seller: Address,
        token: Address,
        amount: i128,
        metadata: Option<Bytes>,
    ) -> Result<u64, ContractError> {
        Self::assert_not_paused(&env)?;
        buyer.require_auth();

        // Validate metadata size
        Self::validate_metadata(&metadata)?;

        // Validate amount is positive
        if amount <= 0 {
            return Err(ContractError::InvalidEscrowAmount);
        }

        let escrow_id = Self::next_escrow_id(&env)?;

        let escrow = Escrow {
            buyer: buyer.clone(),
            seller: seller.clone(),
            token: token.clone(),
            amount,
            status: EscrowStatus::Pending,
            metadata,
        };

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

        // Emit event
        let event = EscrowCreatedEvent {
            escrow_id,
            buyer,
            seller,
            token,
            amount,
            status: EscrowStatus::Pending,
        };
        env.events().publish(
            (Symbol::new(&env, "escrow_created"), escrow_id),
            event,
        );

        Ok(escrow_id)
    }

    /// Retrieve an escrow record by ID.
    pub fn get_escrow(env: Env, escrow_id: u64) -> Option<Escrow> {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
    }

    /// Get metadata for an escrow.
    pub fn get_escrow_metadata(env: Env, escrow_id: u64) -> Option<Bytes> {
        let escrow: Option<Escrow> = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id));
        
        escrow.and_then(|e| e.metadata)
    }

    pub fn fund_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        // existing fund logic here
        Ok(())
    }

    pub fn release_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        // existing release logic here
        Ok(())
    }

    pub fn release_partial(
        env: Env,
        escrow_id: u64,
        amount: i128,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        // existing partial release logic here
        Ok(())
    }

    pub fn refund_escrow(
        env: Env,
        escrow_id: u64,
        initiator: Address,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        initiator.require_auth();
        // existing refund logic here
        Ok(())
    }

    pub fn resolve_dispute(
        env: Env,
        escrow_id: u64,
        resolution: u32,
    ) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        // existing dispute resolution logic here
        Ok(())
    }

    // =========================
    // 🔧 ADMIN FUNCTIONS
    // =========================

    /// Get the current admin address.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::Admin)
    }

    /// Set the platform fee percentage (basis points).
    pub fn set_fee_percentage(env: Env, fee_bps: u32) -> Result<(), ContractError> {
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

        env.storage()
            .persistent()
            .set(&DataKey::FeeBps, &fee_bps);

        env.events().publish(
            (Symbol::new(&env, "fee_changed"),),
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
}

pub use errors::ContractError;
pub use types::{
    DataKey, Escrow, EscrowCreatedEvent, EscrowStatus, FundsReleasedEvent, RefundHistoryEntry,
    RefundReason, RefundRequest, RefundStatus, StatusChangeEvent, MAX_METADATA_SIZE,
};
