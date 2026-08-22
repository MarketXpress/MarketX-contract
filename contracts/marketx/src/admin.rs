//! admin operations for the MarketX escrow contract.

use crate::*;
use soroban_sdk::*;

#[contractimpl]
impl Contract {
    // =========================
    // 🔧 ADMIN FUNCTIONS
    // =========================

    /// Propose a contract WASM upgrade, starting the timelock (#242).
    ///
    /// The upgrade cannot take effect until `UPGRADE_TIMELOCK_LEDGERS` have
    /// elapsed, giving escrow participants a window to observe the pending
    /// change and exit. Proposing again replaces any existing proposal and
    /// restarts the delay.
    pub fn propose_upgrade(
        env: Env,
        new_wasm_hash: soroban_sdk::BytesN<32>,
    ) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;

        let now = env.ledger().sequence();
        let pending = PendingUpgrade {
            wasm_hash: new_wasm_hash.clone(),
            proposed_at: now,
            ready_at: now + UPGRADE_TIMELOCK_LEDGERS,
        };

        env.storage()
            .persistent()
            .set(&DataKey::PendingUpgrade, &pending);

        UpgradeProposedEvent {
            wasm_hash: new_wasm_hash,
            proposed_at: pending.proposed_at,
            ready_at: pending.ready_at,
        }
        .publish(&env);

        Ok(())
    }

    /// Cancel a proposed upgrade before it executes (#242).
    ///
    /// This is the escape hatch for a proposal made in error, or one observed
    /// during the timelock window and judged malicious.
    pub fn cancel_upgrade(env: Env) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;

        let pending: PendingUpgrade = env
            .storage()
            .persistent()
            .get(&DataKey::PendingUpgrade)
            .ok_or(ContractError::NoPendingUpgrade)?;

        env.storage().persistent().remove(&DataKey::PendingUpgrade);

        UpgradeCancelledEvent {
            wasm_hash: pending.wasm_hash,
            cancelled_at: env.ledger().sequence(),
        }
        .publish(&env);

        Ok(())
    }

    /// Read the currently proposed upgrade, if any (#242).
    ///
    /// Public and unauthenticated on purpose: the timelock only protects escrow
    /// participants if they can observe a pending WASM swap before it lands.
    pub fn get_pending_upgrade(env: Env) -> Option<PendingUpgrade> {
        env.storage().persistent().get(&DataKey::PendingUpgrade)
    }

    /// Execute a previously proposed upgrade once its timelock has elapsed (#242).
    pub fn execute_upgrade(env: Env) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;

        let pending: PendingUpgrade = env
            .storage()
            .persistent()
            .get(&DataKey::PendingUpgrade)
            .ok_or(ContractError::NoPendingUpgrade)?;

        let now = env.ledger().sequence();
        if now < pending.ready_at {
            return Err(ContractError::UpgradeTimelockNotElapsed);
        }

        env.storage().persistent().remove(&DataKey::PendingUpgrade);
        env.deployer()
            .update_current_contract_wasm(pending.wasm_hash.clone());

        UpgradeExecutedEvent {
            wasm_hash: pending.wasm_hash,
            executed_at: now,
        }
        .publish(&env);

        Ok(())
    }

    /// Get the current schema version for state migration (#216).
    pub(crate) fn get_schema_version(env: &Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::SchemaVersion)
            .unwrap_or(0)
    }

    /// Set the schema version (internal).
    pub(crate) fn set_schema_version(env: &Env, version: u32) {
        env.storage()
            .persistent()
            .set(&DataKey::SchemaVersion, &version);
    }

    /// Get schema version for a specific escrow.
    pub(crate) fn get_escrow_schema_version(env: &Env, escrow_id: u64) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::EscrowSchemaVersion(escrow_id))
            .unwrap_or(0)
    }

    /// Set schema version for a specific escrow.
    pub(crate) fn set_escrow_schema_version(env: &Env, escrow_id: u64, version: u32) {
        env.storage()
            .persistent()
            .set(&DataKey::EscrowSchemaVersion(escrow_id), &version);
    }

    /// Check if migration is needed.
    pub fn migration_needed(env: Env) -> bool {
        Self::get_schema_version(&env) < CURRENT_SCHEMA_VERSION
    }

    /// Get current schema version (public).
    pub fn get_schema_version_public(env: Env) -> u32 {
        Self::get_schema_version(&env)
    }

    /// Migrate contract state to latest schema version (#216).
    ///
    /// This function handles breaking changes to the Escrow struct by migrating
    /// stored data to the new format. Called by admin after upgrading the contract WASM.
    ///
    /// # Arguments
    /// - `target_version` - The target schema version to migrate to
    ///
    /// # Errors
    /// - `MigrationInvalidSourceVersion` if current version is invalid
    /// - `MigrationAlreadyUpToDate` if already at target version
    /// - `MigrationInvalidTargetVersion` if target version is invalid
    pub fn migrate(env: Env, target_version: u32) -> Result<u32, ContractError> {
        Self::assert_admin(&env)?;

        let current_version = Self::get_schema_version(&env);

        if current_version == 0 && target_version > 1 {
            return Err(ContractError::MigrationInvalidSourceVersion);
        }

        if target_version > CURRENT_SCHEMA_VERSION {
            return Err(ContractError::MigrationInvalidTargetVersion);
        }

        if current_version >= target_version {
            return Err(ContractError::MigrationAlreadyUpToDate);
        }

        match (current_version, target_version) {
            (0, 1) => {
                Self::migrate_v0_to_v1()?;
            }
            _ => {
                return Err(ContractError::MigrationInvalidTargetVersion);
            }
        }

        Self::set_schema_version(&env, target_version);

        Ok(target_version)
    }

    /// Migrate from version 0 (pre-migration) to version 1.
    /// This initializes schema version tracking for all existing escrows.
    pub(crate) fn migrate_v0_to_v1() -> Result<(), ContractError> {
        Ok(())
    }

    /// Propose a new admin. The transfer is not complete until the new admin accepts.
    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), ContractError> {
        let current_admin = Self::assert_admin(&env)?;
        if new_admin == current_admin {
            return Err(ContractError::InvalidAdminTransfer);
        }
        env.storage()
            .persistent()
            .set(&DataKey::ProposedAdmin, &new_admin);
        Ok(())
    }

    /// Cancel an in-flight admin transfer proposal.
    pub fn cancel_admin_transfer(env: Env) -> Result<(), ContractError> {
        Self::assert_admin(&env)?;
        env.storage().persistent().remove(&DataKey::ProposedAdmin);
        Ok(())
    }

    /// Accept the administrative role. Must be called by the proposed admin.
    pub fn accept_admin(env: Env) -> Result<(), ContractError> {
        let proposed_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::ProposedAdmin)
            .ok_or(ContractError::NotProposedAdmin)?;

        // The proposed admin must authenticate this transaction
        proposed_admin.require_auth();

        let old_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotAdmin)?;

        // Transfer the admin role
        env.storage()
            .persistent()
            .set(&DataKey::Admin, &proposed_admin);

        // Clean up the proposal
        env.storage().persistent().remove(&DataKey::ProposedAdmin);

        // Emit the event
        AdminTransferredEvent {
            old_admin,
            new_admin: proposed_admin,
        }
        .publish(&env);

        Ok(())
    }

    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Admin)
    }

    pub fn get_proposed_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::ProposedAdmin)
    }
}
