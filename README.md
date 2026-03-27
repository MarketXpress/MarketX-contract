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
    └── marketx/             # Escrow contract for marketplace settlement
        ├── Cargo.toml       # Inherits versions from workspace
        ├── Makefile         # Per-contract shortcuts
        └── src/
            ├── lib.rs       # Contract entrypoints & module-level docs
            ├── errors.rs    # ContractError variants
            ├── types.rs     # Escrow, EscrowStatus, DataKey
            └── test.rs      # Unit & snapshot tests
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

## Deploy to Testnet

### 1. Configure a testnet identity

Generate a keypair and fund it via Friendbot:

```bash
stellar keys generate --global deployer --network testnet
stellar keys fund deployer --network testnet
```

Verify the account address:

```bash
stellar keys address deployer
```

### 2. Deploy the contract

```bash
stellar contract deploy \
  --wasm target/wasm32v1-none/release/marketx.wasm \
  --source deployer \
  --network testnet
```

On success, the CLI outputs a contract ID:

```
CXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
```

Export it for use in subsequent commands:

```bash
export CONTRACT_ID=CXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
```

### 3. Example: invoke a contract function

```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --source deployer \
  --network testnet \
  -- \
  create_escrow \
  --buyer GBUYERADDRESSXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX \
  --seller GSELLERADDRESSXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX \
  --amount 1000000 \
  --token CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC
```

> **Note:** Amounts are in stroops (1 XLM = 10,000,000 stroops).

### 4. Verify deployment

```bash
stellar contract info \
  --id $CONTRACT_ID \
  --network testnet
```

---

## Contract Reference

This section still contains some legacy naming from earlier contract iterations. For the current event model, off-chain indexing guidance, and TTL behavior, use the `Event Schemas`, `Off-Chain Indexing Spec`, `TTL Maintenance`, and `Current Implementation Notes` sections below as the source of truth.

### Storage Model

All state is stored in **persistent** ledger entries (minimum TTL: 4,096 ledgers on testnet, ~5.7 hours at 5 s/ledger). There are three key types:

| Key | Type | Description |
|---|---|---|
| `Escrow(u64)` | `Escrow` | One record per escrow, keyed by caller-assigned ID |
| `EscrowCount` | `u64` | Monotonic counter reserved for future auto-ID generation |
| `InitialValue` | `u32` | Arbitrary value set at initialization; defaults to `0` |

The `Escrow` struct has five fields: `buyer: Address`, `seller: Address`, `token: Address`, `amount: i128` (in the token's base unit, e.g. stroops for XLM), and `status: EscrowStatus`.

### Escrow Lifecycle

An escrow moves through a strict state machine. `Released` and `Refunded` are terminal — no further transitions are permitted once either is reached.

```
Pending ──► Released   buyer confirms delivery
Pending ──► Disputed   dispute raised
Pending ──► Refunded   direct cancellation
Disputed ──► Released  resolved in seller's favour
Disputed ──► Refunded  resolved in buyer's favour
```

All transitions except `Disputed → Released` require **buyer authorization** (`require_auth`).

### Functions

#### `initialize(initial_value: u32)`

Stores an initial `u32` value in persistent storage. Can be called multiple times; subsequent calls overwrite the previous value.

#### `get_initial_value() → u32`

Returns the value set by `initialize`, or `0` if `initialize` has not been called.

#### `store_escrow(escrow_id: u64, escrow: Escrow)`

Writes an `Escrow` record to persistent storage under `escrow_id`. Silently overwrites any existing record — callers are responsible for ID uniqueness.

#### `get_escrow(escrow_id: u64) → Escrow`

Returns the escrow record for `escrow_id`. Traps (panics) if the ID does not exist. Use `try_get_escrow` when the ID may be absent.

#### `try_get_escrow(escrow_id: u64) → Result<Escrow, ContractError>`

Safe variant of `get_escrow`. Returns `ContractError::EscrowNotFound` instead of trapping on a missing ID.

#### `transition_status(escrow_id: u64, new_status: EscrowStatus) → Result<(), ContractError>`

The primary state-mutation entrypoint. Loads the escrow, enforces buyer authorization for buyer-initiated moves, validates the transition against the state graph, and persists the updated record.

| Error | Condition |
|---|---|
| `EscrowNotFound` | No record exists for `escrow_id` |
| `InvalidTransition` | Move not permitted from the current state |

#### `release_escrow(escrow_id: u64) → Result<(), ContractError>`

Convenience wrapper that releases funds to the seller. Validates that the escrow is in `Pending` state before delegating to `transition_status`, surfacing `EscrowNotFunded` as a clearer error than the generic `InvalidTransition`.

| Error | Condition |
|---|---|
| `EscrowNotFound` | No record exists for `escrow_id` |
| `EscrowNotFunded` | Escrow is not in `Pending` state |
| `InvalidTransition` | Transition rejected by state graph (propagated from `transition_status`) |

### Errors

| Variant | Value | Meaning |
|---|---|---|
| `EscrowNotFound` | `1` | No escrow stored for the given ID |
| `InvalidTransition` | `2` | State move not in the valid transition graph |
| `EscrowNotFunded` | `3` | Escrow is not in `Pending` state |

Error discriminant values are part of the on-chain ABI — they must not be renumbered.

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

## Event Schemas (Off-Chain Tracking)

The contract now emits Soroban `#[contractevent]` events using compact `vec` payloads instead of map-style payloads. This avoids per-field name overhead and keeps escrow events free of large string or metadata blobs.

| Event | Topics | Data | Emitted when |
|---|---|---|---|
| `EscrowCreatedEvent` | `("escrow_created", escrow_id)` | `[buyer, seller, token, amount, status, arbiter]` | `create_escrow` |
| `FundsReleasedEvent` | `("funds_released", escrow_id)` | `[amount]` | `release_escrow` |
| `StatusChangeEvent` | `("status_change", escrow_id)` | `[from_status, to_status, actor]` | Every implemented escrow status transition |
| `FeeChangedEvent` | `("fee_changed")` | `[old_fee_bps, new_fee_bps, actor]` | `set_fee_percentage` |

`StatusChangeEvent` is the canonical lifecycle stream. Every implemented escrow status mutation now emits it, including dispute resolution.

## Off-Chain Indexing Spec

There is intentionally no on-chain `EscrowsByBuyer` or `EscrowsBySeller` index. Frontends should derive those views from events.

Recommended indexer flow:

1. Subscribe to all events for the MarketX contract ID.
2. On `EscrowCreatedEvent`, read `escrow_id` from the second topic and decode the data vector as `[buyer, seller, token, amount, status, arbiter]`.
3. Upsert a canonical escrow record keyed by `escrow_id`.
4. Append `escrow_id` to off-chain lookup tables keyed by `buyer` and `seller`.
5. Optionally maintain an arbiter lookup table when `arbiter` is present.
6. On `StatusChangeEvent`, update the escrow status and move the escrow between active and terminal views.

This schema provides everything needed for user escrow lists without an on-chain reverse index:

- `EscrowCreatedEvent` supplies the user addresses, escrow ID, token, amount, initial status, and arbiter.
- `StatusChangeEvent` supplies the full transition history needed to keep active/completed views current.
- If a detail page needs optional metadata, the indexer can fetch `get_escrow` or `get_escrow_metadata` once and cache it off-chain instead of paying to include metadata in every event.

## TTL Maintenance

Persistent entries on Soroban expire unless their TTL is extended. The contract exposes `bump_escrow(escrow_id: u64)` so anyone can refresh long-lived escrow storage before archival.

- `bump_escrow` is permissionless.
- It extends the escrow record itself.
- It also extends the duplicate-prevention hash entry associated with that escrow.
- Integrators can call it periodically for long-running escrows or disputes.

## Current Implementation Notes

The current public flows are `create_escrow`, `fund_escrow`, `release_escrow`, `resolve_dispute`, pause/unpause, fee updates, and `bump_escrow`.

`release_partial`, `refund_escrow`, and broader pending-state transitions are still placeholders and should not yet be treated as production-ready flows.

## License

MIT
