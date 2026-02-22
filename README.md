# MarketX Contracts

Smart contracts powering the MarketX decentralized marketplace.

This repository contains Soroban smart contracts written in Rust for handling escrow, payments, and core on-chain marketplace logic on the Stellar network.

---

## Overview

MarketX leverages Stellar's Soroban smart contract platform to provide:

- Secure escrow between buyers and sellers
- Controlled fund release and refunds
- Authorization-based state transitions
- On-chain validation of marketplace operations
- Event emission for off-chain indexing and monitoring

The contract layer is designed to be secure, deterministic, and minimal.

---

## Tech Stack

- Rust (stable toolchain)
- Soroban Smart Contracts (soroban-sdk v25)
- stellar-cli v25
- Stellar Testnet (initial deployment target)

---

## Prerequisites

### 1. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update
```

### 2. Add WASM targets

```bash
# Legacy target (used for cargo test / dev builds)
rustup target add wasm32-unknown-unknown

# New Soroban target (used by stellar contract build)
rustup target add wasm32v1-none
```

### 3. Install stellar-cli

```bash
cargo install stellar-cli
```

Verify installation:

```bash
stellar --version
```

---

## Project Structure

This repository is a **Cargo workspace** — every directory under `contracts/` is automatically included as a workspace member. Adding a new contract requires no changes to the root `Cargo.toml`.

```
.
├── Cargo.toml               # Workspace manifest & shared dependencies
├── Cargo.lock               # Locked dependency versions (committed)
├── Makefile                 # Workspace-wide shortcuts (build, test, fmt, check)
└── contracts/
    └── marketx/             # Placeholder contract (replace with real logic)
        ├── Cargo.toml       # Inherits versions from workspace
        ├── Makefile         # Per-contract shortcuts
        └── src/
            ├── lib.rs       # Contract entrypoints
            └── test.rs      # Unit tests
```

### Adding a New Contract

```bash
stellar contract init . --name <contract-name>
```

This scaffolds `contracts/<contract-name>/` and automatically adds it to the workspace.
Shared dependency versions (e.g. `soroban-sdk`) are inherited from `[workspace.dependencies]` in the root `Cargo.toml`.

---

## Build

Build all contracts as optimized WASM artifacts:

```bash
make build
# or directly:
stellar contract build
```

Artifacts land at:

```
target/wasm32v1-none/release/<contract-name>.wasm
```

---

## Test

```bash
make test
# or directly:
cargo test
```

All contract logic must be covered by unit tests.

---

## Development Guidelines

- Use explicit authorization checks (`require_auth`)
- Validate all inputs
- Avoid unnecessary storage writes
- Keep state transitions clear and deterministic
- Format and check before opening a PR:

```bash
make fmt
make check
```

- Ensure no warnings before opening a PR

---

## Deployment Target

- **Initial deployment target**: Stellar Testnet
- **Mainnet deployment** will follow thorough testing and review.

---

## License

MIT
