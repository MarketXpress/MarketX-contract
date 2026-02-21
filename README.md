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
- Soroban Smart Contracts  
- soroban-cli  
- Stellar Testnet (initial deployment target)  

---

## Prerequisites

### Install Rust

```bash
rustup update
rustup target add wasm32-unknown-unknown




````markdown
# Soroban Smart Contract Development

## Install Soroban CLI

Follow the official [Stellar Soroban installation documentation](https://soroban.stellar.org/docs/getting-started/installation).

Verify installation:

```bash
soroban --version
````

## Build Contract

```bash
cargo build --target wasm32-unknown-unknown --release
```

The compiled WASM file will be located in:

```text
target/wasm32-unknown-unknown/release/
```

## Run Tests

```bash
cargo test
```

> All contract logic must be covered by unit tests.

## Development Guidelines

* Use explicit authorization checks (`require_auth`)
* Validate all inputs
* Avoid unnecessary storage writes
* Keep state transitions clear and deterministic
* Format code using:

```bash
cargo fmt
```

* Ensure no warnings before submitting changes

## Deployment Target

* **Initial deployment target**: Stellar Testnet
* **Mainnet deployment** will follow thorough testing and review.

## License

MIT (or update as appropriate)


