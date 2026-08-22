//! token controls operations for the MarketX escrow contract.

use crate::*;
use soroban_sdk::*;

#[contractimpl]
impl Contract {
    // =========================================================================
    // 🚦 ISSUE #215: TOKEN-SPECIFIC CIRCUIT BREAKER
    // =========================================================================

    /// Pause all escrow operations for a specific token.
    ///
    /// When a token is paused, `create_escrow`, `fund_escrow`, and
    /// `release_escrow` will reject any escrow denominated in that token.
    /// Existing escrows are not affected until the next state-mutating call.
    ///
    /// Admin-only.
    pub fn pause_token(env: Env, token: Address) -> Result<(), ContractError> {
        let admin = Self::assert_admin(&env)?;
        env.storage()
            .persistent()
            .set(&DataKey::TokenPaused(token.clone()), &true);
        TokenCircuitBreakerEvent {
            token,
            paused: true,
            actor: admin,
        }
        .publish(&env);
        Ok(())
    }

    /// Unpause a previously paused token, re-enabling escrow operations.
    ///
    /// Admin-only.
    pub fn unpause_token(env: Env, token: Address) -> Result<(), ContractError> {
        let admin = Self::assert_admin(&env)?;
        env.storage()
            .persistent()
            .remove(&DataKey::TokenPaused(token.clone()));
        TokenCircuitBreakerEvent {
            token,
            paused: false,
            actor: admin,
        }
        .publish(&env);
        Ok(())
    }

    /// Returns `true` if the given token is currently paused.
    pub fn is_token_paused(env: Env, token: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::TokenPaused(token))
            .unwrap_or(false)
    }

    pub(crate) fn assert_token_not_paused(env: &Env, token: &Address) -> Result<(), ContractError> {
        let paused: bool = env
            .storage()
            .persistent()
            .get(&DataKey::TokenPaused(token.clone()))
            .unwrap_or(false);
        if paused {
            return Err(ContractError::TokenPaused);
        }
        Ok(())
    }
}
