# Task 164: Integer Safety in Fee Basis Points Calculations

## Task Analysis

### Context
Rounding errors or overflows in fee calculations can drain funds from the escrow system.

### Acceptance Criteria (Implied)
1. No integer overflow in fee calculations
2. No rounding errors that can cause fund loss
3. Accurate fee calculation for all amount ranges

---

## Current Issues Identified

### Issue 1: Unchecked Multiplication (Line 787 - OLD CODE)
```rust
let mut fee: i128 = escrow.amount * (fee_bps as i128) / 10_000;
```
**Problem**: Can overflow for large amounts

### Issue 2: Division Before Multiplication (Better Approach)
```rust
let calculated_fee = (escrow.amount / 10_000) * discounted_fee_bps as i128;
```
**Problem**: Loses precision for amounts < 10,000

### Issue 3: Remainder Handling
```rust
let remainder = amount % 10_000;
calculated_fee = calculated_fee.saturating_add((remainder * discounted_fee_bps as i128) / 10_000);
```
**Problem**: Need to verify correctness

---

## Plan to Achieve Task

### Step 1: Use Saturating Arithmetic

Replace regular arithmetic with saturating operations:

```rust
// Instead of:
let fee = amount * fee_bps / 10_000;

// Use:
let fee = amount.saturating_mul(fee_bps as i128) / 10_000;
```

### Step 2: Division-Before-Multiplication Trick

To prevent overflow, divide before multiplying:

```rust
let fee = (amount / 10_000).saturating_mul(fee_bps as i128);
```

### Step 3: Handle Remainder Precisely

Add remainder portion separately:

```rust
let quotient = amount / 10_000;
let remainder = amount % 10_000;
let fee = quotient.saturating_mul(fee_bps as i128)
    + (remainder * fee_bps as i128) / 10_000;  // No overflow possible here
```

### Step 4: Verify All Fee Calculation Points

Find all places where fees are calculated:
1. `release_escrow` function
2. `verify_delivery` function  
3. `release_item` function
4. Any other fee calc in contract

### Step 5: Add Tests

Create tests for edge cases:
- Amount = 1 (minimum)
- Amount = 10,000 (exactly divides)
- Amount = 10,001 (with remainder)
- Large amount near i128::MAX
- Various fee_bps values

### Step 6: Documentation

Update fee calculation documentation with integer safety notes.

---

## Files/Context Involved

| File | Role |
|------|------|
| `src/lib.rs` | Main fee calculations in release_escrow, verify_delivery, release_item |
| `src/test.rs` | Add integer safety tests |
| `src/errors.rs` | May add new error (e.g., FeeCalculationOverflow) |

---

## Things to Consider

### 1. Precision vs Safety Trade-off
- Division-first loses some precision
- Need to handle remainder correctly
- Result must match expected fee exactly

### 2. Multiple Fee Calculation Points
- `release_escrow` - main fee calc
- `verify_delivery` - oracle-triggered fee
- `release_item` - item partial fee
- All must use consistent method

### 3. Min/Max Caps Interaction  
- Apply caps AFTER calculation
- Caps prevent negative results

### 4. Testing Edge Cases
| Amount | Expected Behavior |
|--------|-------------------|
| 1 | fee = 0 (rounds down) |
| 9,999 | fee = 0 (rounds down) |
| 10,000 | fee = exact (1 * fee_bps) |
| 10,001 | fee = 1 + remainder |
| 1,000,000,000 (1B) | No overflow |
| i128::MAX | No panic |

### 5. Verification Formula

For verification, fee should equal:
```
fee = (amount * fee_bps) / 10_000

Which equals:
fee = (amount / 10_000) * fee_bps + (amount % 10_000) * fee_bps / 10_000
```

---

## Implementation Order

1. **Identify** all fee calculation points
2. **Create** helper function for safe calculation
3. **Replace** all calculations with helper
4. **Add** error handling for overflow
5. **Test** edge cases
6. **Document** the method

---

## Summary

This task requires hardening all fee basis points calculations to use integer-safe operations:

1. Use `saturating_mul` instead of regular multiplication
2. Divide-before-multiply to prevent overflow
3. Handle remainder separately for precision
4. Test edge cases thoroughly
5. Apply to all fee calc points uniformly

This is a security hardening task - no new features, just making existing calculations safer.