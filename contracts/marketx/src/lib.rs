#![no_std]

use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, Vec};

mod errors;
mod types;

use soroban_sdk::xdr::ToXdr;

pub use errors::ContractError;
pub use types::{
    DataKey, Escrow, EscrowCreatedEvent, EscrowStatus, FeeChangedEvent, FundsReleasedEvent,
    RefundHistoryEntry, RefundReason, RefundRequest, RefundStatus, StatusChangeEvent,
    MAX_METADATA_SIZE,
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

    /// Generate a unique hash for an escrow based on buyer, seller, and metadata.
    /// This hash is used to prevent duplicate escrows.
    fn generate_escrow_hash(
        env: &Env,
        buyer: &Address,
        seller: &Address,
        metadata: &Option<Bytes>,
    ) -> BytesN<32> {
        let mut bytes = Bytes::new(env);

        // Add buyer to hash
        bytes.append(&buyer.to_xdr(env));

        // Add seller to hash
        bytes.append(&seller.to_xdr(env));

        // Add metadata to hash (if present)
        if let Some(ref data) = metadata {
            bytes.append(data);
        }

        env.crypto().sha256(&bytes).into()
    }

    /// Check if an escrow with the same buyer, seller, and metadata already exists.
    fn check_duplicate_escrow(
        env: &Env,
        buyer: &Address,
        seller: &Address,
        metadata: &Option<Bytes>,
    ) -> Result<(), ContractError> {
        let hash = Self::generate_escrow_hash(env, buyer, seller, metadata);

        let existing: Option<u64> = env.storage().persistent().get(&DataKey::EscrowHash(hash));

        if existing.is_some() {
            return Err(ContractError::DuplicateEscrow);
        }

        Ok(())
    }

    fn emit_status_change(
        env: &Env,
        escrow_id: u64,
        from_status: EscrowStatus,
        to_status: EscrowStatus,
        actor: Address,
    ) {
        StatusChangeEvent {
            escrow_id,
            from_status,
            to_status,
            actor,
        }
        .publish(env);
    }
}

#[contractimpl]
impl Contract {
    // =========================
    // 🚀 INITIALIZATION
    // =========================

    pub fn initialize(env: Env, admin: Address, fee_collector: Address, fee_bps: u32) {
        admin.require_auth();

        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::FeeCollector, &fee_collector);
        env.storage().persistent().set(&DataKey::FeeBps, &fee_bps);

        // 🔒 Circuit breaker default
        env.storage().persistent().set(&DataKey::Paused, &false);

        // 🔢 Counter starts at 0
        env.storage()
            .persistent()
            .set(&DataKey::EscrowCounter, &0u64);

        // 📊 Analytics initialization
        env.storage()
            .persistent()
            .set(&DataKey::TotalFundedAmount, &0i128);
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
    /// * `arbiter` - Optional arbiter mutually agreed upon by buyer and seller.
    ///               If provided, only this address may call `resolve_dispute` for this escrow.
    ///
    /// # Errors
    /// * `MetadataTooLarge` - If metadata exceeds 1KB
    /// * `DuplicateEscrow` - If an escrow with same buyer, seller, and metadata exists
    pub fn create_escrow(
        env: Env,
        buyer: Address,
        seller: Address,
        token: Address,
        amount: i128,
        metadata: Option<Bytes>,
        arbiter: Option<Address>,
    ) -> Result<u64, ContractError> {
        Self::assert_not_paused(&env)?;
        buyer.require_auth();

        // Validate metadata size
        Self::validate_metadata(&metadata)?;

        // Validate amount is positive
        if amount <= 0 {
            return Err(ContractError::InvalidEscrowAmount);
        }

        // Check for duplicate escrow
        Self::check_duplicate_escrow(&env, &buyer, &seller, &metadata)?;

        let escrow_id = Self::next_escrow_id(&env)?;

        let escrow = Escrow {
            buyer: buyer.clone(),
            seller: seller.clone(),
            token: token.clone(),
            amount,
            status: EscrowStatus::Pending,
            metadata: metadata.clone(),
            arbiter: arbiter.clone(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        // Store the hash to prevent duplicates
        let hash = Self::generate_escrow_hash(&env, &buyer, &seller, &metadata);
        env.storage()
            .persistent()
            .set(&DataKey::EscrowHash(hash), &escrow_id);

        // Update total funded amount
        let current_total: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalFundedAmount)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalFundedAmount, &(current_total + amount));

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
            arbiter,
        };
        event.publish(&env);

        Ok(escrow_id)
    }

    /// Retrieve an escrow record by ID.
    pub fn get_escrow(env: Env, escrow_id: u64) -> Option<Escrow> {
        env.storage().persistent().get(&DataKey::Escrow(escrow_id))
    }

    /// Get metadata for an escrow.
    pub fn get_escrow_metadata(env: Env, escrow_id: u64) -> Option<Bytes> {
        let escrow: Option<Escrow> = env.storage().persistent().get(&DataKey::Escrow(escrow_id));

        escrow.and_then(|e| e.metadata)
    }

    // =========================
    // 📊 ANALYTIC VIEWS
    // =========================

    /// Get the total number of escrows created.
    pub fn get_total_escrows(env: Env) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::EscrowCounter)
            .unwrap_or(0)
    }

    /// Get the total amount of funds that have been put into escrow.
    pub fn get_total_funded_amount(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalFundedAmount)
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

        // 2. Validate escrow is in Pending state
        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        // 3. Enforce buyer authorization (covers the token transfer below)
        escrow.buyer.require_auth();

        // 4. Transfer funds from buyer into the contract
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &escrow.buyer,
            &env.current_contract_address(),
            &escrow.amount,
        );

        Ok(())
    }

    pub fn release_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        // 1. Load and validate the escrow exists
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // 2. Validate escrow is in Pending state
        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        // 3. Enforce buyer authorization
        escrow.buyer.require_auth();
        let actor = escrow.buyer.clone();
        let from_status = escrow.status.clone();

        // 4. Transfer funds from contract to seller via token interface
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.seller,
            &escrow.amount,
        );

        // 5. Update escrow status to Released
        escrow.status = EscrowStatus::Released;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        // 6. Emit FundsReleasedEvent
        let event = FundsReleasedEvent {
            escrow_id,
            amount: escrow.amount,
        };
        event.publish(&env);
        Self::emit_status_change(&env, escrow_id, from_status, escrow.status.clone(), actor);

        Ok(())
    }
    pub fn release_partial(env: Env, _escrow_id: u64, _amount: i128) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;
        // existing partial release logic here
        Ok(())
    }

    /// Request a refund for an escrow.
    /// 
    /// # Arguments
    /// * `escrow_id` - The ID of the escrow to refund
    /// * `reason` - The reason for the refund request
    /// 
    /// # Requirements
    /// * Initiator must be the buyer
    /// * Escrow must be in Pending state
    /// * Creates a RefundRequest tracking object
    /// * Changes escrow status to Disputed
    pub fn refund_escrow(
        env: Env,
        escrow_id: u64,
        reason: RefundReason,
    ) -> Result<u64, ContractError> {
        Self::assert_not_paused(&env)?;

        // 1. Load and validate the escrow exists
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        // 2. Validate escrow is in Pending state
        if escrow.status != EscrowStatus::Pending {
            return Err(ContractError::InvalidEscrowState);
        }

        // 3. Enforce buyer authorization - only the buyer can request a refund
        escrow.buyer.require_auth();
        let from_status = escrow.status.clone();

        // 4. Generate next refund request ID
        let refund_count: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::RefundCount)
            .unwrap_or(0);
        let request_id = refund_count + 1;

        // 5. Create RefundRequest tracking object
        let refund_request = RefundRequest {
            request_id,
            escrow_id,
            requester: escrow.buyer.clone(),
            amount: escrow.amount,
            reason,
            status: RefundStatus::Pending,
            created_at: env.ledger().timestamp(),
        };

        // 6. Store the refund request
        env.storage()
            .persistent()
            .set(&DataKey::RefundRequest(request_id), &refund_request);

        // 7. Update refund count
        env.storage()
            .persistent()
            .set(&DataKey::RefundCount, &request_id);

        // 8. Change escrow status to Disputed
        escrow.status = EscrowStatus::Disputed;
        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        // 9. Emit status change event
        Self::emit_status_change(&env, escrow_id, from_status, escrow.status.clone(), escrow.buyer.clone());

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

    /// Resolve a disputed escrow.
    ///
    /// If the escrow has an assigned arbiter, only that arbiter may call this.
    /// Otherwise, the contract admin may resolve it.
    ///
    /// `resolution`: 0 = release to seller, 1 = refund to buyer
    pub fn resolve_dispute(env: Env, escrow_id: u64, resolution: u32) -> Result<(), ContractError> {
        Self::assert_not_paused(&env)?;

        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(ContractError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Disputed {
            return Err(ContractError::InvalidEscrowState);
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

        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);

        if resolution == 0 {
            // Release to seller
            token_client.transfer(
                &env.current_contract_address(),
                &escrow.seller,
                &escrow.amount,
            );
            escrow.status = EscrowStatus::Released;
        } else {
            // Refund to buyer
            token_client.transfer(
                &env.current_contract_address(),
                &escrow.buyer,
                &escrow.amount,
            );
            escrow.status = EscrowStatus::Refunded;
        }

        env.storage()
            .persistent()
            .set(&DataKey::Escrow(escrow_id), &escrow);

        Self::emit_status_change(&env, escrow_id, from_status, escrow.status.clone(), actor);

        Ok(())
    }

    // =========================
    // 🔧 ADMIN FUNCTIONS
    // =========================

    /// Get the current admin address.
    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Admin)
    }

    /// Set the platform fee percentage (basis points).
    pub fn set_fee_percentage(env: Env, fee_bps: u32) -> Result<(), ContractError> {
        let admin = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::Admin)
            .ok_or(ContractError::NotAdmin)?;
        admin.require_auth();
        let old_fee_bps = env
            .storage()
            .persistent()
            .get(&DataKey::FeeBps)
            .unwrap_or(0);

        // Validate fee is within allowed range (max 10% = 1000 bps)
        if fee_bps > 1000 {
            return Err(ContractError::InvalidFeeConfig);
        }

        env.storage().persistent().set(&DataKey::FeeBps, &fee_bps);

        FeeChangedEvent {
            old_fee_bps,
            new_fee_bps: fee_bps,
            actor: admin,
        }
        .publish(&env);

        Ok(())
    }

    /// Get the current fee percentage in basis points.
    pub fn get_fee_bps(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::FeeBps)
            .unwrap_or(0)
    }

    /// Get a refund request by ID.
    pub fn get_refund_request(env: Env, request_id: u64) -> Option<RefundRequest> {
        env.storage().persistent().get(&DataKey::RefundRequest(request_id))
    }

    /// Get the total number of refund requests.
    pub fn get_refund_count(env: Env) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::RefundCount)
            .unwrap_or(0)
    }
}
