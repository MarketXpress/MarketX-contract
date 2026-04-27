# Advanced Fee Structures - Volume-based Discount Implementation Plan

## Task Overview

Implement a tiered fee discount system where high-volume traders receive reduced transaction fees. This incentivizes platform usage and rewards loyal buyers.

---

## 1. Requirements Summary

| Requirement | Detail |
|-------------|--------|
| **Volume metric** | Total amount traded (i128) |
| **Reset policy** | Monthly (per ledger month) |
| **Discount recipient** | Buyer only |
| **Maximum discount** | 50% |

---

## 2. Current Fee System

The contract already has fee structures in place:
- `FeeBps` - Base fee in basis points (e.g., 500 bps = 5%)
- `MinFee` / `MaxFee` - Fee caps
- `NativeFeeBps` - Special rate for native XLM
- `FeeWhitelist` - Fee exempt addresses (100% discount)

We will add volume-based discounts as an additional discount layer.

---

## 3. Fee Discount Tiers

| Tier | Volume Range (XLM/stroops) | Discount |
|------|---------------------------|----------|
| 0 | 0 - 99,999 | 0% |
| 1 | 100,000 - 999,999 | 10% |
| 2 | 1,000,000 - 9,999,999 | 25% |
| 3 | 10,000,000+ | 50% (max) |

**Note**: Discounts are capped at 50% even for Tier 3.

---

## 4. Storage Model Changes

### 4.1 New DataKey Variants

Add to `src/types.rs`:

```rust
// Monthly volume tracking
DataKey::BuyerVolume(Address),           // Current month's volume
DataKey::VolumeResetLedger(u32),           // Ledger when volume was last reset
DataKey::VolumeTiers(VolumeTierConfig),    // Tier threshold configuration
```

### 4.2 New Types

```rust
#[contracttype]
#[derive(Clone)]
pub struct VolumeTierConfig {
    pub tier_1_threshold: i128,  // 100,000
    pub tier_2_threshold: i128,  // 1,000,000
    pub tier_3_threshold: i128,  // 10,000,000
    pub tier_1_discount_bps: u32, // 100 (10%)
    pub tier_2_discount_bps: u32, // 250 (25%)
    pub tier_3_discount_bps: u32, // 500 (50% - max)
    pub reset_ledger: u32,        // Last reset point
}

#[contracttype]
#[derive(Clone)]
pub struct BuyerVolumeData {
    pub volume: i128,
    pub tier: u8,
    pub last_reset_ledger: u32,
}
```

---

## 5. Functions to Implement

### 5.1 Admin Functions

| Function | Description |
|----------|-------------|
| `set_volume_tiers(config: VolumeTierConfig)` | Configure tier thresholds and discount percentages |
| `get_volume_tiers() -> VolumeTierConfig` | View current tier configuration |

### 5.2 Volume Management

| Function | Description |
|----------|-------------|
| `get_buyer_volume(buyer: Address) -> i128` | Get buyer's current monthly volume |
| `get_buyer_tier(buyer: Address) -> u8` | Get buyer's current tier (0-3) |
| `is_volume_reset_needed(env: Env) -> bool` | Check if monthly reset is due |

### 5.3 Internal Helpers

| Function | Description |
|----------|-------------|
| `calculate_volume_discount(buyer: Address) -> u32` | Calculate discount bps based on volume |
| `update_buyer_volume(buyer: Address, amount: i128)` | Add to volume after release |
| `reset_volume_if_needed(env: Env)` | Monthly volume reset logic |

---

## 6. Fixed Fee Calculation Architecture

### 6.1 The 7 Critical Flaws in Current Code

Before implementing volume discounts, we must fix theseexisting bugs in `release_escrow`:

| # | Flaw | Impact |
|---|------|--------|
| 1 | Fee calculated twice | Inconsistent results, min/max caps ignored |
| 2 | Variable shadowing | Confusing code, dead code paths |
| 3 | Missing whitelist check first | Whitelist user may still be charged |
| 4 | No volume update after release | Discounts always tier 0 |
| 5 | Min/max logic discarded | Fee caps never applied |
| 6 | Integer overflow risk | Could fail on large amounts |
| 7 | Whitelist vs volume conflict | Unclear precedence |

### 6.2 Industry Standard Solutions

#### Flaw 1 & 5: Single Calculation Path

```rust
fn calculate_fee_with_all_factors(
    env: &Env,
    amount: i128,
    buyer: &Address,
    base_fee_bps: u32,
) -> (i128, u32) {
    // Step 1: Check whitelist FIRST (100% exemption)
    let is_whitelisted: bool = env
        .storage()
        .persistent()
        .get(&DataKey::FeeWhitelist(buyer.clone()))
        .unwrap_or(false);

    if is_whitelisted {
        return (0, 0);
    }

    // Step 2: Calculate volume discount
    let volume_discount = Self::calculate_volume_discount(env, buyer);
    let max_discount: u32 = 500; // 50% cap
    let actual_discount = volume_discount.min(max_discount);

    // Step 3: Apply discount to base fee
    let discounted_fee_bps = base_fee_bps.saturating_sub(actual_discount);

    // Step 4: Calculate base fee
    let mut fee = (amount / 10_000).saturating_mul(discounted_fee_bps as i128);
    let remainder = amount % 10_000;
    fee = fee.saturating_add((remainder * discounted_fee_bps as i128) / 10_000);

    // Step 5: Apply min/max caps
    let min_fee: i128 = env.storage().persistent().get(&DataKey::MinFee).unwrap_or(0);
    let max_fee: i128 = env.storage().persistent().get(&DataKey::MaxFee).unwrap_or(0);

    fee = fee.max(min_fee);
    if max_fee > 0 {
        fee = fee.min(max_fee);
    }

    // Step 6: Ensure fee doesn't exceed amount
    fee = fee.min(amount);

    (fee, actual_discount)
}
```

#### Flaw 2: Avoid Variable Shadowing

```rust
// BAD - Causes confusion
let mut fee_bps: u32 = env.storage().get(&DataKey::FeeBps);
let fee_bps: u32 = env.storage().get(&DataKey::FeeBps);

// GOOD - Clear conditional flow
let effective_fee_bps = if escrow.token == native_asset {
    env.storage().persistent().get(&DataKey::NativeFeeBps).unwrap_or(base_fee_bps)
} else {
    base_fee_bps
};
```

#### Flaw 4: Update Volume After Release

```rust
fn update_buyer_volume(env: &Env, buyer: &Address, amount: i128) {
    // Lazy reset check
    Self::check_volume_reset(env);

    // Get and increment
    let current: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::BuyerVolume(buyer.clone()))
        .unwrap_or(0);

    let new_volume = current.saturating_add(amount);

    env.storage().persistent().set(
        &DataKey::BuyerVolume(buyer.clone()),
        &new_volume,
    );

    // Emit event
    VolumeUpdatedEvent {
        buyer: buyer.clone(),
        added_amount: amount,
        new_volume,
    }.publish(env);
}
```

#### Flaw 6: Integer Overflow Prevention

```rust
// Divide first to prevent overflow
let fee = (amount / 10_000).saturating_mul(fee_bps as i128);
// Plus remainder calculation
let remainder_fee = (amount % 10_000 * fee_bps) / 10_000;
fee = fee.saturating_add(remainder_fee);

// Or use checked operations
let fee = amount
    .checked_mul(fee_bps as i128)
    .ok_or(ContractError::FeeOverflow)? / 10_000;
```

#### Flaw 7: Clear Precedence

```
Priority Order (HIGHEST to LOWEST):
┌─────────────────────────────────────┐
│ 1. Whitelist    → 100% discount    │ ← Always wins if present
├─────────────────────────────────────┤
│ 2. Volume       → Tiered % discount  │ ← After whitelist check
├─────────────────────────────────────┤
│ 3. Base Fee     → Normal bps         │ ← Default
├─────────────────────────────────────┤
│ 4. Min/Max Caps → Bounds check       │ ← Applied last
└─────────────────────��───────────────┘
```

---

## 7. Modified Functions

### 7.1 `release_escrow` (src/lib.rs:801-943)

**Current behavior**: Calculate fee using `FeeBps`, apply whitelist exemption

**FIXED behavior**:
1. Check whitelist first (if yes: fee = 0)
2. Get base fee bps (check for native XLM special rate)
3. Calculate volume discount from buyer's tier
4. Apply discount to base (capped at 50%)
5. Calculate fee with overflow protection
6. Apply min/max caps
7. Cap at escrow amount
8. Update buyer's volume AFTER successful release

**Pseudo-code**:
```rust
pub fn release_escrow(env: Env, escrow_id: u64) -> Result<(), ContractError> {
    // ... validation ...

    // FAILED FEE CALCULATION - Replace entirely!
    let (fee, discount_applied) = Self::calculate_fee_with_all_factors(
        &env,
        escrow.amount,
        &escrow.buyer,
        base_fee_bps,
    )?;

    // ... transfer funds ...

    // Update volume AFTER successful release
    Self::update_buyer_volume(&env, &escrow.buyer, escrow.amount);

    Ok(())
}
```

### 7.2 `initialize` (src/lib.rs:310-345)

Initialize default volume tier config:
```rust
env.storage().persistent().set(&DataKey::VolumeTiers(VolumeTierConfig {
    tier_1_threshold: 100_000,
    tier_2_threshold: 1_000_000,
    tier_3_threshold: 10_000_000,
    tier_1_discount_bps: 100,
    tier_2_discount_bps: 250,
    tier_3_discount_bps: 500,
    reset_ledger: env.ledger().sequence(),
}));
```

---

## 8. Files to Modify

| File | Changes |
|------|---------|
| `src/types.rs` | Add `VolumeTierConfig`, `BuyerVolumeData`, new `DataKey` variants, `VolumeUpdatedEvent` |
| `src/lib.rs` | Rewrite fee calculation, add volume functions, modify `release_escrow`, modify `initialize` |
| `src/errors.rs` | Add `FeeCalculationOverflow` if using checked operations |
| `src/test.rs` | Add tests for volume tracking and discount calculation |

---

## 9. Monthly Reset Logic

### 9.1 Reset Trigger

Soroban uses ledger sequence for timing. Monthly reset can be based on:
- **Fixed interval**: Every ~1,576,800 ledgers (~30 days at 5s/ledger)
- **Calendar month**: Use ledger timestamp / timestamp-based month detection

**Recommended**: Fixed interval approach for simplicity.

```rust
const VOLUME_RESET_INTERVAL: u32 = 1_576_800; // ~30 days

fn should_reset_volume(env: &Env, last_reset: u32) -> bool {
    let current_ledger = env.ledger().sequence();
    current_ledger.saturating_sub(last_reset) >= VOLUME_RESET_INTERVAL
}
```

### 9.2 Reset Implementation

```rust
fn reset_volume_if_needed(env: &Env) {
    let config: VolumeTierConfig = env.storage().persistent()
        .get(&DataKey::VolumeTiers(...))
        .unwrap_or(default_config());

    if Self::should_reset_volume(env, config.reset_ledger) {
        // Note: Individual buyer volumes reset on next access (lazy reset)
        // This avoids iterating all addresses
        config.reset_ledger = env.ledger().sequence();
        env.storage().persistent().set(&DataKey::VolumeTiers(...), &config);
    }
}
```

### 9.3 Lazy Reset per Buyer

When fetching buyer's volume:
```rust
fn get_buyer_volume(env: &Env, buyer: &Address) -> i128 {
    let volume: i128 = env.storage().persistent()
        .get(&DataKey::BuyerVolume(buyer.clone()))
        .unwrap_or(0);

    let config = Self::get_volume_tiers(env);

    // Lazy reset if needed
    if Self::should_reset_volume(env, config.reset_ledger) {
        return 0; // Volume reset
    }

    volume
}
```

---

## 10. Event Emissions

Add new event for volume tracking:

```rust
#[contractevent(topics = ["volume_updated"], data_format = "vec")]
#[derive(Clone)]
pub struct VolumeUpdatedEvent {
    pub buyer: Address,
    pub added_amount: i128,
    pub new_volume: i128,
    pub new_tier: u8,
}
```

---

## 11. Edge Cases and Considerations

### 11.1 Fee Never Negative
- Use `saturating_sub` for discount application
- Fee bps after discount must be >= 0

### 11.2 Discount Never Exceeds 50%
- Hard cap: `actual_discount = volume_discount.min(500)`

### 11.3 Volume Added After Release Only
- Only increment volume when escrow successfully transitions to `Released`
- Not on `Pending` or `Disputed` states

### 11.4 Bulk Escrow Consideration
- For `create_bulk_escrows`, which buyer gets volume discount?
- Apply volume after each individual escrow release

### 11.5 Native XLM Volume
- Should native XLM volume count separately?
- **Recommendation**: Yes, track all tokens equivalently in stroops

### 11.6 Storage Optimization
- Consider TTL for buyer volume entries
- Old volumes can be auto-archived after reset

---

## 12. Testing Requirements

Create tests that verify:

| Test | Expected Behavior |
|------|------------------|
| `test_volume_tier_0_no_discount` | Buyer with <100,000 volume pays full fee |
| `test_volume_tier_1_10_percent` | Buyer with 100,000+ gets 10% discount |
| `test_volume_tier_2_25_percent` | Buyer with 1,000,000+ gets 25% discount |
| `test_volume_tier_3_max_50_percent` | Buyer with 10,000,000+ gets 50% discount (capped) |
| `test_volume_reset_monthly` | Volume resets after reset interval |
| `test_volume_accumulates_after_release` | Volume increases after each release |
| `test_seller_no_discount` | Seller receives no discount even if high volume |
| `test_discount_compounded_with_whitelist` | 100% exempt still applies (whitelist takes precedence) |
| `test_discount_with_native_fee` | Volume discount works with native XLM special fee rate |

---

## 13. Admin Controls

| Function | Access | Description |
|----------|--------|-------------|
| `set_volume_tiers` | Admin only | Configure thresholds and discounts |
| `get_volume_tiers` | Anyone | View current config |
| `get_buyer_volume` | Anyone | Query buyer's volume |

---

## 14. Implementation Order

1. **Phase 1**: Fix the 7 fee calculation flaws in `release_escrow`
2. **Phase 2**: Add types and storage keys (`src/types.rs`)
3. **Phase 3**: Add volume functions (`src/lib.rs`)
4. **Phase 4**: Modify `release_escrow` to apply discount
5. **Phase 5**: Add admin configuration functions
6. **Phase 6**: Add tests

---

## 15. Summary

This implementation adds a sophisticated volume-based discount system that:

- Tracks buyer trading volume in stroops
- Applies tiered discounts (0%, 10%, 25%, 50% max)
- Resets monthly
- Only benefits buyers
- Integrates with existing fee system
- Uses industry-standard fee calculation (single path, clear precedence)

All changes maintain backward compatibility with the existing whitelist and fee cap systems.

---

## Appendix: Fixed Fee Calculation Flow

```
┌─────────────────────────────────────┐
│ 1. Check Whitelist (100% off)        │ ← If yes: fee = 0, DONE
└─────────────────────────────────────┘
              ↓ No
┌─────────────────────────────────────┐
│ 2. Get Base Fee BPS                  │
│    (check native XLM special)       │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 3. Calculate Volume Discount       │ ← Get from buyer's tier
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 4. Apply: capped_discount = min(    │
│    volume_discount, 500)           │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 5. Calculate: discounted_bps =     │
│    base_fee_bps - capped_discount   │
│    (saturating_sub)                │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 6. Calculate: fee = amount *       │
│    discounted_bps / 10000           │
│    (overflow safe)                 │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 7. Apply Min/Max Caps             │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 8. Cap at amount                  │
│    (can't exceed escrow)             │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 9. Return (fee, discount_applied)  │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 10. Transfer funds                │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 11. Update buyer volume           │ ← AFTER successful release
└─────────────────────────────────────┘
```

This flow ensures:
- Single calculation path
- Clear precedence (whitelist > volume > caps)
- No dead code
- Volume tracking after successful release
- Overflow-safe arithmetic