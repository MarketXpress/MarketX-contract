#![no_std]
#![allow(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_cast)]
#![allow(dead_code)]

//! # MarketX Smart Contract
//!
//! The public contract interface is implemented across domain-focused modules.

use soroban_sdk::contract;

mod admin;
mod arbiters;
mod errors;
mod escrow;
mod fees;
mod group_buys;
mod lifecycle;
mod mediation;
mod milestones;
mod multi_arbiter;
mod time_locks;
mod token_controls;
mod types;
mod utilities;

pub use errors::ContractError;
pub use types::*;

#[cfg(test)]
mod test;

/// The MarketX escrow contract.
///
/// Its public interface is implemented by the domain modules declared above.
#[contract]
pub struct Contract;

soroban_sdk::contractmeta!(key = "name", val = "MarketX Escrow");
soroban_sdk::contractmeta!(
    key = "description",
    val =
        "Soroban escrow contract with milestone releases, dispute handling, and configurable fees."
);
soroban_sdk::contractmeta!(
    key = "homepage",
    val = "https://github.com/MarketXpress/MarketX-contract"
);
soroban_sdk::contractmeta!(
    key = "repository",
    val = "https://github.com/MarketXpress/MarketX-contract"
);
soroban_sdk::contractmeta!(
    key = "source",
    val = "https://github.com/MarketXpress/MarketX-contract/tree/main/contracts/marketx"
);
soroban_sdk::contractmeta!(key = "version", val = "v1.0.0");
