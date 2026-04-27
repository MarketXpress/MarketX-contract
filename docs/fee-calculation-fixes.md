# Fee Calculation Fixes - Industry Standard Solutions

## The 7 Critical Flaws (Task-Related)

---

### Flaw 1: Fee Calculated Twice - Logic Conflict

**Problem**: Fee calculated twice with different logic (lines 850-878), second overwrites first.

**Industry Standard Solution**:

```rust
fn calculate_fee(
    env: &Env,
    amount: i128,
    base_fee_bps: u32,
    is_whitelisted: bool,
    min_fee: i128,
    max_fee: i128,
) -> (i128, i128) {
    // Wholesale exemption first - highest priority
    if is_whitelisted {
        return (0, 0);
    }

    // Calculate base fee
    let mut fee = amount * (base_fee_bps as i128) / 10_000;

    // Apply fee caps using saturating operations
    let fee = fee.max(min_fee);
    let fee = if max_fee > 0 { fee.min(max_fee) } else { fee };
    let fee = fee.min(amount); // Never exceed escrow amount

    // Calculate discount bps for event logging
    let discount_bps = base_fee_bps;

    (fee, discount_bps)
}
```

**Key Principles**:
- Single calculation path
- Apply discounts BEFORE caps, or caps wrap discounts correctly
- Return computed fee only

---

### Flaw 2: Variable Mutability Bug (Shadowing)

**Problem**: Multiple `let` declarations with same name shadow each other.

**Industry Standard Solution**:

```rust
// BAD - Shadowing causes confusion
let mut fee_bps: u32 = env.storage()...
let fee_bps: u32 = env.storage()...  // shadows!

// GOOD - Single declaration, clear flow
let fee_bps = match env.storage().persistent().get::<DataKey, Address>(&DataKey::NativeAsset) {
    Some(native) if escrow.token == native => {
        env.storage().persistent().get(&DataKey::NativeFeeBps).unwrap_or(default_fee_bps)
    }
    _ => default_fee_bps,
};
```

**Key Principles**:
- Use match for conditional logic
- Avoid `mut` unless necessary
- One declaration per variable

---

### Flaw 3: Missing Whitelist Check in First Calculation

**Problem**: First fee calc ignores whitelist, creates inconsistent state before overwrite.

**Industry Standard Solution**:

Apply whitelist/exemption FIRST, then calculate discount:

```rust
fn calculate_fee_with_discount(
    env: &Env,
    amount: i128,
    buyer: &Address,
    base_fee_bps: u32,
) -> i128 {
    // Check whitelist FIRST (100% exemption)
    let is_whitelisted: bool = env
        .storage()
        .persistent()
        .get(&DataKey::FeeWhitelist(buyer.clone()))
        .unwrap_or(false);

    if is_whitelisted {
        return 0;
    }

    // Calculate volume discount
    let volume_discount = calculate_volume_discount(env, buyer);

    // Apply volume discount to base fee
    let discounted_fee_bps = base_fee_bps.saturating_sub(volume_discount);
    
    // Calculate final fee
    amount * (discounted_fee_bps as i128) / 10_000
}
```

**Key Principles**:
- Priority: Whitelist (100%) > Volume Discount > Base Fee
- Apply in order of precedence

---

### Flaw 4: No Volume Update After Release

**Problem**: Buyer's volume never incremented - discounts always tier 0.

**Industry Standard Solution**:

```rust
fn update_buyer_volume(env: &Env, buyer: &Address, amount: i128) {
    // Lazy reset check first
    Self::check_and_reset_volume(env);

    // Get current volume
    let current: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::BuyerVolume(buyer.clone()))
        .unwrap_or(0);

    // Increment volume
    let new_volume = current.saturating_add(amount);
    
    env.storage().persistent().set(
        &DataKey::BuyerVolume(buyer.clone()),
        &new_volume,
    );

    // Emit event for indexing
    VolumeUpdatedEvent {
        buyer: buyer.clone(),
        added_amount: amount,
        new_volume,
    }.publish(env);
}
```

**Key Principles**:
- Update AFTER successful transfer (in same atomic tx)
- Use saturating_add to prevent overflow
- Emit event for off-chain tracking

---

### Flaw 5: Min/Max Fee Logic Applied Then Discarded

**Problem**: Caps applied then overwritten by recalculation.

**Industry Standard Solution**:

```rust
fn calculate_final_fee(
    base_fee: i128,
    min_fee: i128,
    max_fee: i128,
    amount: i128,
) -> i128 {
    // Step 1: Ensure within bounds - wrap (don't discard)
    let fee = base_fee.max(min_fee);
    let fee = if max_fee > 0 { fee.min(max_fee) } else { fee };
    let fee = fee.min(amount);  // Never more than escrow amount

    fee
}

// Or use saturating for safety:
fn calculate_final_fee_saturating(
    base_fee: i128,
    min_fee: i128,
    max_fee: i128,
    amount: i128,
) -> i128 {
    base_fee
        .max(min_fee)
        .min(if max_fee > 0 { max_fee } else { i128::MAX })
        .min(amount)
}
```

**Key Principles**:
- Single path, not two calculations
- Apply min/max as final bounds check
- Don't recalculate - just constrain

---

### Flaw 6: Integer Overflow Risk

**Problem**: `amount * fee_bps` could overflow for large values.

**Industry Standard Solution**:

```rust
// Option 1: Use checked operations
let fee = amount
    .checked_mul(fee_bps as i128)
    .ok_or(ContractError::FeeCalculationOverflow)?
    / 10_000;

// Option 2: Use i256 temporarily
let fee_i256 = (i256::from(amount) * i256::from(fee_bps)) / 10_000;
let fee = fee_i256 as i128;  // Safe if within bounds

// Option 3: Divide first (for small fees)
let fee = (amount / 10_000) * fee_bps as i128;
// Note: This loses precision for amounts < 10000
```

**Recommended**: Use Option 1 with `checked_mul`:

```rust
fn calculate_fee_safe(amount: i128, fee_bps: u32) -> i128 {
    let fee = (amount / 10_000)
        .saturating_mul(fee_bps as i128);
    
    // If amount < 10000, this gives 0, so fix:
    let remainder = amount % 10_000;
    let fee = fee.saturating_add(
        (remainder * fee_bps as i128) / 10_000
    );
    
    fee
}
```

**Key Principles**:
- Prefer `saturating_mul` over unchecked
- Consider division order for precision
- Document which approach used

---

### Flaw 7: Whitelist vs Volume Discount Conflict

**Problem**: Both exist - which wins? Current logic is unclear.

**Industry Standard Solution**:

Define clear precedence:

```rust
enum FeeDiscount {
    None,           // Full fee
    Whitelist,       // 100% off (admin granted)
    Volume,         // % based on tier
    // Combined would be Volume (whitelist already checked)
}

// Priority order:
// 1. Whitelist check -> 0% fee (100% discount)
// 2. Volume discount -> apply % discount
// 3. Min/Max caps -> ensure within bounds

fn calculate_fee_with_precedence(
    env: &Env,
    amount: i128,
    buyer: &Address,
) -> i128 {
    // Step 1: Whitelist (100% wins)
    if is_whitelisted(env, buyer) {
        return 0;
    }

    // Step 2: Volume discount
    let volume_discount = get_volume_discount(env, buyer);
    let base_fee_bps = get_base_fee_bps(env);
    let discounted_bps = base_fee_bps.saturating_sub(volume_discount);

    // Step 3: Calculate
    let fee = calculate_fee(amount, discounted_bps);

    // Step 4: Apply caps
    let min_fee = get_min_fee(env);
    let max_fee = get_max_fee(env);
    apply_fee_caps(fee, min_fee, max_fee, amount)
}
```

**Key Principles**:
- Document discount precedence
- Whitelist is always 100% (explicit admin grant)
- Volume is % discount based on activity
- Use saturating_sub so fee never negative

---

## Summary: Fixed Fee Calculation Flow

```
┌─────────────────────��───────────────┐
│ 1. Check Whitelist (100% off)     │ ← If yes: fee = 0, DONE
└─────────────────────────────────────┘
              ↓ No
┌─────────────────────────────────────┐
│ 2. Get Base Fee BPS                │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 3. Calculate Volume Discount      │ ← Get from buyer's tier
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 4. Apply: discounted_bps = base -  │
│    volume_discount (cap at 500)    │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 5. Calculate: fee = amount *       │
│    discounted_bps / 10000          │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 6. Apply Min/Max Caps             │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│ 7. Cap at amount (can't exceed)   │
└─────────────────────────────────────┘
              ↓
    Return Final Fee
```

This flow ensures:
- Single calculation path
- Clear precedence
- No dead code
- Volume tracking after successful release