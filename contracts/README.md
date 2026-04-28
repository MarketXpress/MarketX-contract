# contracts/

This directory is a Cargo workspace — every subdirectory is automatically a workspace member. Each contract is a self-contained Rust crate that compiles to a Soroban WASM artifact.

---

## Directory Layout

```
contracts/
└── marketx/          # Escrow contract for marketplace settlement
    ├── Cargo.toml    # Inherits versions from workspace root
    ├── Makefile      # Per-contract shortcuts (build, test, fmt)
    └── src/
        ├── lib.rs        # Contract entrypoints (all public functions)
        ├── types.rs      # Shared types, events, and constants
        ├── errors.rs     # ContractError variants (stable discriminants)
        ├── test.rs       # Unit & integration tests
        ├── test_integer_safety.rs  # Overflow / boundary tests
        ├── test_volume.rs          # Bulk-operation tests
        └── tarpaulin.rs            # Coverage helpers
```

---

## Contract: `marketx`

The core escrow contract. Handles the full lifecycle of a marketplace transaction between a buyer and a seller, with optional arbiter and fee management.

### Key Capabilities

| Area | Functions |
|---|---|
| Escrow lifecycle | `create_escrow`, `fund_escrow`, `release_escrow`, `cancel_unfunded` |
| Partial releases | `release_item`, `release_partial` |
| Dispute resolution | `refund_escrow`, `resolve_dispute`, `claim_disputed_funds` |
| Mediation phase | `open_mediation`, `propose_mediation_settlement`, `get_mediation_phase` |
| Evidence & appeals | `open_evidence_window`, `submit_evidence`, `expire_evidence_window`, `file_appeal`, `resolve_appeal` |
| Arbiter staking | `stake_as_arbiter`, `slash_arbiter` |
| Milestones | `create_milestone_escrow`, `complete_milestone` |
| Time-locks | `set_time_lock`, `trigger_time_lock_release` |
| Group buys | `create_group_buy_escrow`, `fund_group_buy`, `withdraw_group_buy_contribution` |
| Bulk ops | `create_bulk_escrows`, `batch_collect_fees` |
| Fee management | `set_fee_percentage`, `set_fee_caps`, `set_fee_collector`, `withdraw_fees` |
| Circuit breakers | `pause` / `unpause` (global), `pause_token` / `unpause_token` (per-token) |
| Admin | `initialize`, `transfer_admin`, `accept_admin`, `upgrade` |
| Governance flags | `set_disputes_enabled`, `set_partial_releases_enabled` |
| Read helpers | `get_escrow`, `get_escrow_items`, `get_escrows`, `estimate_storage_rent`, `get_resource_profile` |

### Escrow Lifecycle

```
Pending ──► Funded ──► Released   (buyer releases or oracle verifies)
                  ──► Disputed    (buyer raises refund request)
                  ──► Cancelled   (mutual cancellation)
Disputed ──► Released             (arbiter/admin resolves for seller)
         ──► Refunded             (arbiter/admin resolves for buyer)
         ──► Released             (mediation settlement)
```

`Released` and `Refunded` are terminal states.

### Dispute Mediation Phase (#205)

When a dispute is raised, a **mediation window** opens before the arbiter can act. During this window:

1. Either party calls `open_mediation(caller, escrow_id, window_ledgers)`.
2. Both parties call `propose_mediation_settlement(proposer, escrow_id, seller_amount)`.
3. If both proposals match, the escrow settles immediately — no arbiter needed.
4. If the window expires without agreement, the arbiter may call `resolve_dispute`.

Default window: `DEFAULT_MEDIATION_WINDOW_LEDGERS` (~48 h at 5 s/ledger).

### Token-Specific Circuit Breaker (#215)

The global `pause`/`unpause` halts all operations. The token-level circuit breaker is more surgical:

```
admin → pause_token(token)    # blocks create_escrow + fund_escrow for that token
admin → unpause_token(token)  # restores normal operation
         is_token_paused(token) → bool
```

Use this when a specific token (e.g. USDC) is compromised while other tokens remain healthy.

### Storage Keys

| Key | Type | Description |
|---|---|---|
| `Escrow(u64)` | `Escrow` | One record per escrow |
| `EscrowCounter` | `u64` | Monotonic ID counter |
| `MediationPhase(u64)` | `MediationPhase` | Cooling-off phase for disputed escrow |
| `TokenPaused(Address)` | `bool` | Per-token circuit breaker flag |
| `ArbiterStake(u64)` | `ArbiterStake` | Arbiter stake for a disputed escrow |
| `EvidenceWindow(u64)` | `EvidenceWindow` | Evidence submission window |
| `Appeal(u64)` | `AppealRecord` | Appeal record after arbiter ruling |
| `ArbiterReputation(Address)` | `ArbiterReputation` | Cumulative arbiter stats |
| `PendingFee(Address, Address)` | `i128` | Accrued fees per collector+token |
| `FeeCollector` | `Address` | Current fee collector |
| `FeeBps` | `u32` | Fee in basis points |
| `Admin` | `Address` | Contract administrator |
| `Paused` | `bool` | Global pause flag |

All entries use **persistent** storage. Bump TTL with `bump_escrow(escrow_id)` for long-running escrows.

### Error Codes

Error discriminant values are part of the on-chain ABI and must not be renumbered.

| Code | Variant | Meaning |
|---|---|---|
| 1 | `NotAdmin` | Caller is not the admin |
| 2 | `Unauthorized` | Caller lacks permission |
| 10 | `EscrowNotFound` | No escrow for given ID |
| 11 | `InvalidEscrowState` | Operation not valid in current state |
| 13 | `InvalidEscrowAmount` | Amount is zero, negative, or exceeds escrow |
| 31 | `ContractPaused` | Global pause is active |
| 50 | `InvalidFeeConfig` | Fee configuration is invalid |
| 70 | `DuplicateEscrow` | Identical escrow already exists |
| 130–132 | `ArbiterStake*` | Arbiter staking errors |
| 140–142 | `EvidenceWindow*` | Evidence window errors |
| 150–153 | `Appeal*` | Appeal errors |
| 164 | `FeatureDisabled` | Governance flag disabled this operation |
| 166 | `MediationPhaseOpen` | Arbiter cannot act while mediation is open |
| 167 | `NoMediationPhase` | No mediation phase for this escrow |
| 168 | `MediationAlreadyConcluded` | Mediation phase already concluded |
| 169 | `TokenPaused` | Token is paused by circuit breaker |

---

## Adding a New Contract

```bash
stellar contract init . --name <contract-name>
```

This scaffolds `contracts/<contract-name>/` and adds it to the workspace automatically. Inherit shared dependency versions from the root `Cargo.toml`:

```toml
[dependencies]
soroban-sdk = { workspace = true }
```

---

## Building & Testing

```bash
# Build all contracts
make build

# Run all tests
make test

# Format
make fmt

# Lint
make check
```

Artifacts land at `target/wasm32v1-none/release/<contract-name>.wasm`.
