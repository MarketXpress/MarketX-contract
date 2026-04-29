# Dispute Resolution V2: Multi-Arbiter Consensus System

## Overview

**Theme:** Part 11: Dispute Resolution V2

**Objective:** Eliminate the single point of failure inherent in single-arbiter dispute resolution by implementing a consensus-based system where multiple arbiters must vote on dispute outcomes.

## Problem Statement

The original dispute resolution system relied on a single arbiter for dispute resolution. This creates:
- **Single Point of Failure:** If the arbiter is compromised, biased, or unavailable, there's no recourse
- **Trust Concentration:** All resolution authority is vested in one entity
- **Appeal Complexity:** Appeals must go through the admin (governor), creating a two-tier system

## Solution: Multi-Arbiter Consensus

Dispute Resolution V2 introduces multiple arbiters who must reach consensus before resolving a dispute. This provides:
- **Distributed Trust:** No single arbiter can unilaterally decide
- **Fault Tolerance:** If one arbiter is compromised, others can still reach correct consensus
- **Democratic Governance:** Decisions require majority or configurable quorum
- **Scalability:** Can support 2-7 arbiters per escrow (configurable)

## Key Components

### 1. ArbitersConfig
Stores the configuration for multiple arbiters on a specific escrow:
```rust
pub struct ArbitersConfig {
    pub escrow_id: u64,
    pub arbiters: Vec<Address>,           // List of authorized arbiters
    pub quorum_required: u32,             // Number needed for consensus
    pub created_at: u32,                  // Ledger sequence at creation
}
```

**Example:** An escrow with 3 arbiters and quorum=2 means any 2 of the 3 arbiters voting the same way resolves the dispute.

### 2. ArbiterVoteRecord
Records individual votes from arbiters:
```rust
pub struct ArbiterVoteRecord {
    pub escrow_id: u64,
    pub arbiter: Address,
    pub vote: u32,        // 0=release to seller, 1=refund to buyer
    pub voted_at: u32,    // Ledger of vote
}
```

### 3. DisputeVotingRecord
Tracks aggregate voting on a disputed escrow:
```rust
pub struct DisputeVotingRecord {
    pub escrow_id: u64,
    pub votes_for_release: u32,
    pub votes_for_refund: u32,
    pub total_arbiters: u32,
    pub quorum_required: u32,
    pub voting_opened_at: u32,
    pub consensus_resolution: Option<u32>,   // Some(0) or Some(1) when reached
    pub consensus_at: Option<u32>,
}
```

## Public Functions

### Configuration Functions

#### `configure_multi_arbiters(escrow_id, arbiters, quorum_required)`
Sets up multiple arbiters for an escrow before it's funded.

**Parameters:**
- `escrow_id`: The target escrow
- `arbiters`: Vec of arbiter addresses (2-7 recommended, configurable max)
- `quorum_required`: Minimum arbiters needed for consensus (≥ 1, ≤ arbiters.len())

**Access Control:**
- Original arbiter (if exists), or buyer/seller (if no arbiter yet)

**Constraints:**
- Escrow must be in Pending state
- At least 2 arbiters required
- Quorum must be between 1 and the number of arbiters
- Cannot exceed `MAX_ARBITERS_PER_ESCROW` (default 7)

**Emits:** `ArbitersConfiguredEvent`

#### `get_arbiters_config(escrow_id)`
Retrieves the multi-arbiter configuration for an escrow.

**Returns:** `Option<ArbitersConfig>` - None if no multi-arbiter config exists

---

### Voting Functions

#### `arbiter_vote(escrow_id, vote)`
Cast a vote on how to resolve a dispute.

**Parameters:**
- `escrow_id`: The disputed escrow
- `vote`: 0 (release to seller) or 1 (refund to buyer)

**Access Control:**
- Only authorized arbiters (in the ArbitersConfig list)
- Each arbiter can vote exactly once

**Behavior:**
- Records the vote in persistent storage
- Updates the `DisputeVotingRecord` aggregate
- **Automatically resolves dispute if consensus reached**

**Emits:** 
- `ArbiterVoteCastEvent` (per vote)
- `DisputeConsensusReachedEvent` (when consensus achieved)

**Example Flow:**
```
Escrow with 3 arbiters, quorum=2, dispute raised

Ledger 1000: Arbiter A votes 0 (release) → votes: {release: 1, refund: 0}
Ledger 1001: Arbiter B votes 0 (release) → votes: {release: 2, refund: 0}
  → Consensus reached! (2 ≥ quorum of 2)
  → DisputeConsensusReachedEvent emitted
  → Automatic resolution triggered (release to seller)

Ledger 1002: Arbiter C's vote is irrelevant (consensus already reached)
```

#### `get_dispute_voting(escrow_id)`
Retrieve the voting record for a disputed escrow.

**Returns:** `Option<DisputeVotingRecord>` with current vote counts and consensus status

#### `has_consensus(escrow_id)`
Quick check if consensus has been reached on a dispute.

**Returns:** `bool` - True if voting record shows `consensus_resolution.is_some()`

---

### Resolution Functions

#### `resolve_multi_arbiter_dispute(escrow_id)`
Applies the consensus resolution to the escrow.

**Access Control:**
- Any authorized arbiter can call (they already voted, presumably)

**Preconditions:**
- Escrow must be in Disputed state
- Multi-arbiter config must exist
- Consensus must have been reached (via voting)

**Operations:**
- Updates escrow to Released (vote=0) or Refunded (vote=1)
- Sets claimable funds ledger (appeal window)
- Updates associated refund requests
- Records resolution in each participating arbiter's reputation
- Cleans up vote records

**Emits:** `StatusChangeEvent`

**Note:** This can be called automatically by frontends after consensus, or manually by arbiters.

---

### Configuration & Governance

#### `set_min_arbiters_required(min_arbiters)` [Admin]
Set minimum arbiters for multi-arbiter escrows.

#### `get_min_arbiters_required()`
Get current minimum (default: `DEFAULT_MIN_ARBITERS_REQUIRED` = 2)

#### `set_max_arbiters_per_escrow(max_arbiters)` [Admin]
Set maximum arbiters per escrow (prevents excessive voting overhead).

**Constraints:** 2 ≤ max ≤ 20

#### `get_max_arbiters_per_escrow()`
Get current maximum (default: `DEFAULT_MAX_ARBITERS_PER_ESCROW` = 7)

#### `set_default_arbiter_quorum_percentage(percentage)` [Admin]
Set default quorum percentage (stored as percentage × 100, e.g., 5100 = 51%).

**Constraints:** 5000 ≤ percentage ≤ 10000 (50% to 100%)

#### `get_default_arbiter_quorum_percentage()`
Get current default (default: `DEFAULT_ARBITER_QUORUM_PERCENTAGE` = 5100 = 51%)

---

## Data Flow: Dispute Resolution Process

### Step 1: Create Escrow
```
create_escrow() → Escrow created with single arbiter (or none)
```

### Step 2: Configure Multi-Arbiters (Optional)
```
configure_multi_arbiters(escrow_id, [arbiter1, arbiter2, arbiter3], quorum=2)
  → ArbitersConfig stored
  → ArbitersConfiguredEvent emitted
```

### Step 3: Raise Dispute (if product issue)
```
request_refund() → Escrow moves to Disputed
  → Mediation window opens (48h)
  → DisputeVotingRecord initialized (if multi-arbiter config exists)
```

### Step 4: Mediation (Optional)
```
Both parties can propose settlements during mediation window
If agreement reached → dispute settled without voting
```

### Step 5: Arbiters Vote
```
arbiter_vote(escrow_id, 0 or 1)  [Called by each arbiter]
  → ArbiterVoteRecord stored
  → Aggregate voting updated
  → When quorum reached:
    → consensus_resolution set
    → DisputeConsensusReachedEvent emitted
    → Potential auto-resolution
```

### Step 6: Resolve Dispute
```
resolve_multi_arbiter_dispute(escrow_id)  [Called by any arbiter]
  → Escrow state updated (Released or Refunded)
  → ClaimableAt set to current_ledger + APPEAL_WINDOW_LEDGERS
  → Arbiters' reputation updated
  → StatusChangeEvent emitted
```

### Step 7: Appeal (if desired)
```
file_appeal() → Opens appeal process (admin is final authority)
```

### Step 8: Claim Funds
```
After appeal window closes:
claim_disputed_funds(escrow_id) → Funds transferred to designated party
```

---

## Events Emitted

| Event | When |
|-------|------|
| `ArbitersConfiguredEvent` | Multi-arbiters configured for escrow |
| `ArbiterVoteCastEvent` | Each arbiter votes |
| `DisputeConsensusReachedEvent` | Consensus threshold reached |
| `StatusChangeEvent` | Escrow status changes (via resolve) |

---

## Error Handling

New errors for multi-arbiter disputes:

| Error | Cause |
|-------|-------|
| `ArbiterStakeInsufficient` | Empty arbiters list |
| `ArbiterMismatch` | Caller is not an authorized arbiter |
| `TooManyItems` | More arbiters than `MAX_ARBITERS_PER_ESCROW` |
| `InvalidEscrowState` | Various validation failures |

---

## Security Considerations

### 1. **Sybil Attack Prevention**
- All arbiters must stake tokens (via existing `stake_as_arbiter()`)
- Slashing mechanism punishes incorrect votes
- Reputation tracking across disputes

### 2. **Consensus Requirement**
- No single arbiter can force a resolution
- Requires majority or configured quorum
- Reduces bias risk

### 3. **Immutable Voting**
- Each arbiter votes exactly once per dispute
- Votes recorded on-chain and cannot be changed
- Prevents vote flipping

### 4. **Appeal Window**
- Even with consensus, funds are claimable after appeal window (24h default)
- Admin can overturn if consensus is fraudulent
- Two-layer dispute resolution

### 5. **Reputation System**
- Tracks each arbiter's dispute history
- Records how many times their decisions were overturned
- Enables reputation-based penalties

---

## Configuration Recommendations

### For 2-3 Arbiters (Small Disputes)
```rust
arbiters: [arbiter1, arbiter2, arbiter3]
quorum_required: 2  // 2/3 = 66% consensus
```

### For 5 Arbiters (Medium Disputes)
```rust
arbiters: [a1, a2, a3, a4, a5]
quorum_required: 3  // 3/5 = 60% consensus
```

### For 7 Arbiters (Large/Critical Disputes)
```rust
arbiters: [a1, a2, a3, a4, a5, a6, a7]
quorum_required: 4  // 4/7 = 57% consensus
```

---

## Backward Compatibility

- **Existing Single-Arbiter Escrows:** Unchanged
- **Existing `resolve_dispute()`:** Still works for single arbiters
- **Multi-Arbiter Support:** Opt-in via `configure_multi_arbiters()`
- **No Breaking Changes:** Old escrows continue functioning

---

## Future Enhancements

1. **Weighted Voting:** Arbiters with higher reputation have more voting power
2. **Time-Locked Consensus:** Allow voting for a configurable window before auto-resolve
3. **Consensus Intervals:** Different quorum percentages for different dispute amounts
4. **Arbiter Insurance Pool:** Collective slashing fund for misbehavior
5. **Decentralized Governance:** Adjust parameters via DAO voting

---

## Testing

### Unit Tests
- Verify consensus calculations
- Test quorum edge cases
- Validate vote counting
- Check authorization

### Integration Tests
- Multi-arbiter workflow end-to-end
- Consensus reached scenarios
- Partial voting (below quorum)
- Appeal after consensus

### Edge Cases
- 2 arbiters: simple majority
- Unanimous voting
- Split voting (no consensus)
- Single arbiter fallback

---

## Summary

Dispute Resolution V2 eliminates the single point of failure through:
✅ **Multiple Arbiters** - Distributed decision-making
✅ **Consensus Voting** - Requires agreement on resolution
✅ **Configurable Quorum** - Flexible trust models
✅ **Reputation Tracking** - Accountability for arbiters
✅ **Appeal Window** - Admin oversight remains
✅ **Backward Compatible** - Old system still works

This implements a robust dispute resolution framework suitable for high-value, high-trust-requirement escrows.
