# Contributing to MarketX Contract

Thank you for contributing to the MarketX smart contract. This guide covers everything you need to set up, build, test, and submit quality contributions.

---

## Prerequisites

### 1. Rust (stable toolchain)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable
rustup default stable
```

### 2. WASM targets

```bash
rustup target add wasm32-unknown-unknown  # for cargo test / dev builds
rustup target add wasm32v1-none           # for stellar contract build
```

### 3. Stellar CLI v25

```bash
cargo install stellar-cli --version 25
stellar --version
```

---

## Building

```bash
# Build all contracts as optimised WASM artifacts
make build
# or directly:
stellar contract build
```

For production-ready WASM artifacts with repository-standard optimization flags:

```bash
make build-prod
# or directly:
./scripts/build_wasm.sh
```

---

## Testing

```bash
# Run all unit and integration tests
make test
# or directly:
cargo test
```

All tests must pass before opening a PR. Add tests for every new code path — the existing suite in `src/test.rs` and `tests/integration.rs` shows the patterns to follow.

---

## Formatting and Linting

```bash
# Auto-format all source files
make fmt

# Check that formatting and compilation are clean (no changes made)
make check
```

`make check` must succeed with **zero warnings** before opening a PR. The CI pipeline enforces this automatically.

---

## Code Style Rules

### `no_std` environment
This contract runs in a `no_std` WASM environment. Do **not** use:
- `std::string::String` — use `soroban_sdk::String`
- `std::vec::Vec` — use `soroban_sdk::Vec`
- Heap allocations outside the Soroban SDK
- Any crate that requires `std`

### Soroban SDK patterns
- Authenticate every caller of a state-changing function with `address.require_auth()` or `Self::assert_admin(&env)`.
- Read storage once per function call; avoid redundant reads.
- Emit an event for every observable state change.
- Use `?` for early-return error propagation — never `unwrap()` or `panic!()` in production code paths.

### Input validation
- Validate all `Bytes` fields against the relevant size constant before use:
  - `MAX_METADATA_SIZE` (1 024 bytes) for the top-level `metadata` field
  - `MAX_DESCRIPTION_SIZE` (256 bytes) for per-item and milestone descriptions
  - `MAX_TRACKING_ID_SIZE` (128 bytes) for shipping tracking IDs
  - `MAX_EVIDENCE_HASH_SIZE` (128 bytes) for evidence and counter-evidence hashes
- Use the private `Self::validate_bytes_size(data, max)` helper — it returns `Err(ContractError::MetadataTooLarge)` on violation.

### Error handling
- Return errors via `Result<T, ContractError>`.
- Choose the most specific error variant. Never return a generic error when a specific one exists.
- Add a doc-comment to every new error variant explaining when it fires.

---

## Error Code Conventions

Error discriminants are part of the **on-chain ABI** and are stored in transaction results. Once an error code is assigned it **must never be renumbered** — doing so is a breaking change for all clients.

Rules:
1. New errors go in an existing numeric gap (e.g., code 12 is currently free) **or** at the end of a related block.
2. Document every new variant with a Rust doc-comment.
3. Update `docs/error-codes.md` and `sdk/error-codes.ts` when adding new codes so frontends can display human-readable messages.

---

## Branching and PR Workflow

1. Fork the repository and create a feature branch:
   ```
   git checkout -b feat/your-feature-name
   ```
2. Make your changes and ensure `make check` and `make test` pass.
3. Open a Pull Request against the `main` branch.
4. Link related issues in the PR description using GitHub closing keywords:
   ```
   Closes #123
   Resolves #456
   ```
5. The maintainer will review and may request changes before merging.

### Branch naming conventions

| Prefix | When to use |
|--------|-------------|
| `feat/` | New functionality |
| `fix/` | Bug fix |
| `refactor/` | Code restructuring with no behaviour change |
| `docs/` | Documentation only |
| `test/` | Test additions or fixes |
| `chore/` | Tooling, CI, dependency updates |

---

## Security Checklist

Before submitting a PR that touches core logic, verify:

- [ ] Every public state-changing function calls `require_auth()` or `assert_admin()`.
- [ ] All `Bytes` inputs are validated against the appropriate size constant.
- [ ] No `unwrap()` or `panic!()` in non-test code.
- [ ] No unbounded loops over user-supplied data.
- [ ] Storage entries that should expire set an appropriate TTL or rely on `bump_escrow`.
- [ ] New error variants have doc-comments and are added to `docs/error-codes.md`.
- [ ] `make check` passes with zero warnings.
- [ ] `make test` passes with no failures.

---

## Helpful References

- [Soroban SDK docs](https://docs.rs/soroban-sdk)
- [Stellar Developer Docs](https://developers.stellar.org/docs/smart-contracts)
- [Contract error codes](docs/error-codes.md)
