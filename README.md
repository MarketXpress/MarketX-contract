MarketX Escrow - Soroban (Stellar) Smart Contract

Features
- Multi-party escrow supporting multiple payers, payees, and arbiters
- Multisignature approvals for release/refund/arbitration
- Time-locked automatic release and timeout-based refunds
- Dispute resolution by arbiters
- Partial release support
- Protocol fee with basis points and fee collector
- Emergency release with multisig emergency admins

Project Layout
- src/lib.rs: Contract implementation and tests
- Cargo.toml: Project configuration and dependencies

Requirements
- Rust toolchain
- Soroban CLI and SDK compatible with dependency versions in Cargo.toml

Build
- cargo build --target wasm32-unknown-unknown --release

Test
- cargo test

Notes
- The contract assumes a standard Soroban token interface. In tests we import a token WASM for mint/transfer.
- Fees are collected on release distributions (payer refunds are fee-free by design).
- Automatic release equally distributes remaining balance among payees at or after auto_release_ts.
- refund_timeout refunds to payers proportionally to deposits order up to remaining balance.
- All state transitions check for closed/disputed states to prevent inconsistent actions.
