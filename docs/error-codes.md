# MarketX Contract — Error Code Reference

Every public function that can fail returns a `ContractError`. On-chain, these are represented as `u32` discriminants. This document maps each code to a human-readable message so that frontends and integrators can display meaningful feedback.

> **Important for maintainers:** Error discriminants are part of the on-chain ABI. Never renumber an existing code — doing so is a breaking change. Add new codes in numeric gaps or at the end of the relevant group. See `CONTRIBUTING.md` for the full convention.

---

## Access Control (1–4)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 1 | `NotAdmin` | Only the contract admin can perform this action. | — |
| 2 | `Unauthorized` | You are not authorized to perform this operation. | Ensure you are a party to the escrow (buyer, seller, or arbiter). |
| 3 | `NotProposedAdmin` | This address is not the proposed new admin. | — |
| 4 | `NotOracle` | Only the registered oracle may call this function. | — |

---

## Escrow State (10–13)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 10 | `EscrowNotFound` | Escrow not found. | Check that the escrow ID is correct. |
| 11 | `InvalidEscrowState` | The escrow is in the wrong state for this action. | Check the escrow status before calling. |
| 13 | `InvalidEscrowAmount` | Invalid escrow amount — must be greater than zero. | — |

---

## Circuit Breaker (31)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 31 | `ContractPaused` | The contract is currently paused. Please try again later. | Wait for the admin to unpause the contract. |

---

## Overflow Protection (40)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 40 | `EscrowIdOverflow` | The maximum number of escrows has been reached. | — |

---

## Fee Configuration (50)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 50 | `InvalidFeeConfig` | The fee configuration is invalid. | Ensure fee values are within allowed bounds. |

---

## Input Validation (60)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 60 | `MetadataTooLarge` | Metadata or description field exceeds the maximum allowed size. | Reduce the size of the metadata, description, tracking ID, or evidence hash. |

Size limits enforced by error 60:

| Field | Limit |
|-------|-------|
| `metadata` | 1 024 bytes |
| Per-item `description` | 256 bytes |
| Milestone `description` | 256 bytes |
| `tracking_id` | 128 bytes |
| `evidence_hash` | 128 bytes |

---

## Duplicate Prevention (70)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 70 | `DuplicateEscrow` | An escrow with the same buyer, seller, and metadata already exists. | Change the metadata to create a distinct escrow. |

---

## Items (80–83)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 80 | `ItemNotFound` | Item index not found in this escrow. | Check the item index. |
| 81 | `ItemAlreadyReleased` | This item has already been released. | — |
| 82 | `TooManyItems` | Too many items per escrow (maximum: 50). | Split the escrow into multiple escrows. |
| 83 | `ItemAmountInvalid` | Item amounts do not sum to the total escrow amount. | Ensure all item amounts add up to the escrow total. |

---

## Expiry and Funding (90–91)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 90 | `EscrowNotExpired` | The escrow has not yet expired. | Wait for the unfunded expiry window (~7 days) to pass. |
| 91 | `EscrowAlreadyFunded` | The escrow has already been funded. | — |

---

## Milestones (100–101)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 100 | `MilestoneNotFound` | Milestone index not found. | Check the milestone index. |
| 101 | `MilestoneAlreadyCompleted` | This milestone has already been completed. | — |

---

## Time-Lock (110–111)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 110 | `TimeLockNotReached` | The time-lock release ledger has not been reached yet. | Wait until the configured release ledger. |
| 111 | `TimeLockNotEnabled` | No time-lock is configured for this escrow. | — |

---

## Group Buy (120–123)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 120 | `GroupBuyNotFunded` | The group buy target amount has not been reached yet. | — |
| 121 | `GroupBuyAlreadyFunded` | The group buy has already been fully funded. | — |
| 122 | `GroupBuyDeadlinePassed` | The group buy funding deadline has passed. | — |
| 123 | `InvalidGroupBuyAmount` | Invalid group buy contribution amount. | — |

---

## Dispute Resolution — Arbiter Staking (130–132)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 130 | `ArbiterStakeInsufficient` | The arbiter stake is below the required minimum. | Increase the stake to meet the minimum. |
| 131 | `ArbiterAlreadyStaked` | The arbiter already has an active stake on this escrow. | — |
| 132 | `ArbiterMismatch` | The caller is not the registered arbiter for this escrow. | — |

---

## Dispute Resolution — Evidence Window (140–142)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 140 | `EvidenceWindowExpired` | The evidence submission window has closed. No further submissions are accepted. | — |
| 141 | `EvidenceWindowNotExpired` | The evidence window has not yet expired. | Wait for the window to expire before forcing closure. |
| 142 | `NoEvidenceWindow` | No evidence window is open for this escrow. | — |

---

## Dispute Resolution — Appeals (150–153)

| Code | Variant | Message | Recovery hint |
|------|---------|---------|---------------|
| 150 | `AppealAlreadyFiled` | An appeal has already been filed for this escrow. | — |
| 151 | `AppealNotFound` | No appeal record exists for this escrow. | — |
| 152 | `AppealWindowClosed` | The appeal window has closed. | — |
| 153 | `AppealAlreadyResolved` | The appeal has already been resolved. | — |

---

## Updating this document

When adding a new `ContractError` variant:
1. Add it to the appropriate section above (or create a new section).
2. Update `sdk/error-codes.ts` to add the corresponding TypeScript entry.
3. Keep codes in ascending numeric order within each section.
