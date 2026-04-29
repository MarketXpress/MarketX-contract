// Example: Multi-Arbiter Dispute Resolution V2
// 
// This example demonstrates how to use the multiple arbiters voting system
// to resolve disputes with consensus-based decision making.

// ============================================================================
// SCENARIO: A High-Value Product Dispute
// ============================================================================
// 
// Alice (Buyer) orders a $5,000 piece of electronics from Bob (Seller)
// Alice claims the product is defective and requests a refund
// Bob disputes the claim
// 
// To resolve this high-value dispute fairly, they agree to use 3 arbiters
// with a 2-of-3 consensus requirement (66% agreement needed)

// ============================================================================
// STEP 1: Create the Escrow (During Initial Transaction)
// ============================================================================

/*
Contract Call:
create_escrow(
    buyer: alice,
    seller: bob,
    token: xlm_address,
    amount: 500_000_000, // 5000 XLM
    arbiter: None,       // No single arbiter for now
    metadata: Some(product_id),
    items: None
)

Result:
- Escrow ID: 1001
- Status: Pending
- No arbiters yet
*/

// ============================================================================
// STEP 2: Fund the Escrow (Alice transfers funds)
// ============================================================================

/*
Contract Call:
fund_escrow(escrow_id: 1001)

Result:
- Escrow Status: Funded
- Alice's tokens are in the contract
- Bob can now deliver the product
*/

// ============================================================================
// STEP 3: Dispute Raised (Alice claims defect)
// ============================================================================

/*
Contract Call:
request_refund(
    escrow_id: 1001,
    evidence_hash: Some("QmDefectPhotos..."), // IPFS hash of product photos
)

Result:
- Escrow Status: Disputed
- Mediation Phase Opened (48 hour window)
- DisputeVotingRecord initialized:
  - votes_for_release: 0
  - votes_for_refund: 0
  - total_arbiters: 0 (not configured yet)
  - consensus_resolution: None

Next: Mediation window open for settlement negotiation
*/

// ============================================================================
// STEP 4: Configure Multiple Arbiters (During Mediation)
// ============================================================================
//
// During the mediation window, if no settlement is reached,
// both parties agree to escalate to 3 expert arbiters:
// - Arbiter1: Electronics expert from ReputableFirm1
// - Arbiter2: Quality assurance specialist from ReputableFirm2
// - Arbiter3: Consumer protection advocate from NeutralOrg

/*
Contract Call:
configure_multi_arbiters(
    escrow_id: 1001,
    arbiters: vec![
        Address::from_str("GAA...Arbiter1"),  // Electronics expert
        Address::from_str("GBB...Arbiter2"),  // QA specialist
        Address::from_str("GCC...Arbiter3"),  // Advocate
    ],
    quorum_required: 2  // Need 2 of 3 to agree
)

Result:
- ArbitersConfig stored:
  - escrow_id: 1001
  - arbiters: [Arbiter1, Arbiter2, Arbiter3]
  - quorum_required: 2
  - created_at: 1000 (ledger)
  
- ArbitersConfiguredEvent emitted
- Mediation phase concludes
*/

// ============================================================================
// STEP 5: Arbiters Review Evidence & Vote
// ============================================================================
//
// Phase 1: Arbiter1 Votes (Ledger 1001)

/*
Contract Call (from Arbiter1):
arbiter_vote(
    escrow_id: 1001,
    vote: 0  // Vote 0 = Release to Seller (Bob) - product is fine
)

Result:
- ArbiterVoteRecord stored:
  - escrow_id: 1001
  - arbiter: Arbiter1
  - vote: 0
  - voted_at: 1001
  
- DisputeVotingRecord updated:
  - votes_for_release: 1
  - votes_for_refund: 0
  - consensus_resolution: None (need 2 votes)
  
- ArbiterVoteCastEvent emitted
- Consensus NOT reached yet (need quorum of 2)

Status: Voting in progress...
*/

//
// Phase 2: Arbiter2 Votes (Ledger 1002)

/*
Contract Call (from Arbiter2):
arbiter_vote(
    escrow_id: 1001,
    vote: 0  // Vote 0 = Release to Seller - testing shows no defect
)

Result:
- ArbiterVoteRecord stored:
  - escrow_id: 1001
  - arbiter: Arbiter2
  - vote: 0
  - voted_at: 1002
  
- DisputeVotingRecord updated:
  - votes_for_release: 2
  - votes_for_refund: 0
  - consensus_resolution: Some(0)  // ✓ CONSENSUS REACHED!
  - consensus_at: 1002
  
- ArbiterVoteCastEvent emitted
- DisputeConsensusReachedEvent emitted:
  - escrow_id: 1001
  - resolution: 0  (Release to Seller)
  - votes_for_release: 2
  - votes_for_refund: 0

Status: Consensus reached! Dispute resolved (2-of-3 arbiters agree)
*/

//
// Phase 3: Arbiter3 Vote is Irrelevant

/*
Even though Arbiter3 could vote, it's now irrelevant because
consensus has already been reached (2 of 3 arbiters voted for release).

If Arbiter3 still calls arbiter_vote(), the vote is recorded but doesn't
affect the consensus_resolution which is already set.
*/

// ============================================================================
// STEP 6: Finalize the Dispute Resolution
// ============================================================================

/*
Contract Call (any arbiter or frontend can call):
resolve_multi_arbiter_dispute(escrow_id: 1001)

Result:
- Escrow Status: Released (since consensus was for release)
- ClaimableAt: 1002 + 17_280 = 18_282 (Appeal window)
- StatusChangeEvent emitted
- Arbiter reputation updated (for Arbiter1, Arbiter2, Arbiter3)
- Vote records cleaned up

Current State:
- Bob can claim funds after appeal window closes
- Alice cannot appeal (she lost the vote)
- All arbiters' reputation records updated
*/

// ============================================================================
// STEP 7: Claim Funds After Appeal Window
// ============================================================================

/*
Contract Call (after ledger 18_282):
claim_disputed_funds(escrow_id: 1001)

Caller: Bob (Seller)

Result:
- Escrow funds transferred to Bob
- Final status update
- Transaction complete
*/

// ============================================================================
// ALTERNATIVE SCENARIO: Refund Vote
// ============================================================================
//
// If the arbiters had voted to refund instead:

/*
If during voting:
- Arbiter1: vote 1 (Refund to Buyer)
- Arbiter2: vote 1 (Refund to Buyer)
→ Consensus: Refund to Buyer (resolution = 1)
→ Escrow Status becomes: Refunded
→ Alice can claim her $5000 after appeal window
*/

// ============================================================================
// EDGE CASE: Split Vote (No Consensus)
// ============================================================================

/*
Scenario: 3 arbiters, quorum=2, but split vote

- Arbiter1: vote 0 (Release)
- Arbiter2: vote 1 (Refund)
- Arbiter3: vote 0 (Release)

Result:
- votes_for_release: 2
- votes_for_refund: 1
- Consensus: Release (2 ≥ quorum of 2)
- Even with split opinion, consensus rule is satisfied

Note: With quorum=2, consensus happens when any outcome hits 2 votes.
For more conservative approach, could use quorum=3 with 3 arbiters.
*/

// ============================================================================
// CONFIGURATION OPTIONS
// ============================================================================

/*
Admin Functions (can be called by contract admin):

// Adjust minimum arbiters for new escrows
set_min_arbiters_required(3)
// Result: New multi-arbiter escrows need at least 3 arbiters

// Adjust maximum arbiters per escrow
set_max_arbiters_per_escrow(5)
// Result: Cannot create escrow with more than 5 arbiters

// Adjust default quorum percentage
set_default_arbiter_quorum_percentage(6000)  // 60%
// Result: Recommended quorum for new configs is 60%

// Query current settings
let min = get_min_arbiters_required()        // Returns: 3
let max = get_max_arbiters_per_escrow()      // Returns: 5
let quorum_pct = get_default_arbiter_quorum_percentage()  // Returns: 6000
*/

// ============================================================================
// QUERYING DISPUTE STATUS
// ============================================================================

/*
During dispute voting:

// Get the arbiter configuration
let config = get_arbiters_config(1001)
// Returns: ArbitersConfig {
//   escrow_id: 1001,
//   arbiters: [Arbiter1, Arbiter2, Arbiter3],
//   quorum_required: 2,
//   created_at: 1000
// }

// Get the voting status
let voting = get_dispute_voting(1001)
// Returns: DisputeVotingRecord {
//   escrow_id: 1001,
//   votes_for_release: 2,
//   votes_for_refund: 0,
//   total_arbiters: 3,
//   quorum_required: 2,
//   voting_opened_at: 1000,
//   consensus_resolution: Some(0),
//   consensus_at: Some(1002)
// }

// Quick check if consensus reached
let has_consensus = has_consensus(1001)  // Returns: true
*/

// ============================================================================
// BENEFITS OF MULTI-ARBITER CONSENSUS
// ============================================================================

/*
1. NO SINGLE POINT OF FAILURE
   - If one arbiter is compromised, others still ensure correct outcome
   - Eliminates corruption risk from single bad actor

2. DISTRIBUTED TRUST
   - No individual arbiter has unilateral power
   - Decisions must have agreement from majority

3. REDUCED BIAS
   - Multiple perspectives considered
   - Expert specialties combined (e.g., electronics + QA + advocacy)
   - Bias of one arbiter cannot overturn the other two

4. TRANSPARENT VOTING
   - All votes recorded on-chain
   - Cannot change vote after submission
   - Audit trail for dispute outcomes

5. SCALABLE GOVERNANCE
   - Can use different arbiters for different dispute values
   - Configurable quorum for different risk profiles
   - Admin can adjust parameters over time

6. REPUTATION ACCOUNTABILITY
   - Each arbiter's voting history tracked
   - How often their votes were appealed/overturned recorded
   - Community can assess arbiter reliability

7. APPEAL SAFETY NET
   - Even with consensus, admin can still appeal
   - Creates two-layer dispute resolution
   - Final protection against collusion
*/

// ============================================================================
// SECURITY GUARANTEES
// ============================================================================

/*
✓ Immutable Voting
  - Once arbiter votes, cannot change their vote
  - Prevents "flip-flopping" votes

✓ Exclusive Voting
  - Each arbiter votes exactly once per dispute
  - Cannot vote multiple times under different addresses

✓ Consensus Requirement
  - Resolution only when quorum is met
  - Prevents unilateral decisions

✓ Authorization Checks
  - Only authorized arbiters can vote
  - Caller must authenticate with their private key

✓ Stake-Backed Voting
  - Arbiters must stake tokens (separate function)
  - Slashing mechanism punishes bad votes

✓ Transparent Record
  - All votes and consensus on-chain
  - Cannot secretly influence outcomes
  - Enables post-hoc verification
*/

// ============================================================================
// FUTURE ENHANCEMENTS
// ============================================================================

/*
1. Weighted Voting
   - High-reputation arbiters have more voting power
   - E.g., Arbiter1 (reputation: 100) = 2x vote weight

2. Partial Consensus
   - Allow resolution when supermajority achieved
   - E.g., 66% agreement threshold instead of 51%

3. Time-Limited Voting
   - Voting window closes after N ledgers
   - Forces decision instead of indefinite voting

4. Consensus Tiers
   - Different quorum for small/medium/large disputes
   - E.g., $100 dispute: 2-of-3 required
   - E.g., $100,000 dispute: 4-of-5 required

5. Arbiter Insurance Pool
   - Collective fund for slashing
   - Compensates wronged parties
   - Incentivizes arbiter honesty
*/

// ============================================================================
// BACKWARD COMPATIBILITY
// ============================================================================

/*
Existing single-arbiter escrows continue to work unchanged:

// Old escrows with single arbiter
resolve_dispute(escrow_id, resolution)  ← Still works!

// New multi-arbiter escrows use new flow
configure_multi_arbiters(...)
arbiter_vote(...)
resolve_multi_arbiter_dispute(...)

No breaking changes to existing contract functionality.
*/
