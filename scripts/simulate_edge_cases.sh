#!/usr/bin/env bash
# simulate_edge_cases.sh
#
# Issue #210 — Contract Simulation Script for Edge Cases
#
# Measures resource consumption (CPU instructions, memory, ledger entries) for
# large bulk operations and boundary-condition flows on the MarketX contract.
#
# Prerequisites:
#   - stellar-cli v25+ installed and on PATH
#   - A funded testnet identity called "deployer"
#     (stellar keys generate --global deployer --network testnet)
#   - CONTRACT_ID exported, or pass it as the first argument:
#     ./simulate_edge_cases.sh <CONTRACT_ID>
#
# Usage:
#   export CONTRACT_ID=C...
#   ./scripts/simulate_edge_cases.sh
#
# Each scenario prints the stellar-cli resource footprint line so you can
# compare CPU instructions and ledger-entry counts across runs.

set -euo pipefail

NETWORK="${NETWORK:-testnet}"
SOURCE="${SOURCE:-deployer}"
CONTRACT_ID="${1:-${CONTRACT_ID:?'Set CONTRACT_ID or pass it as the first argument'}}"

STELLAR="stellar contract invoke --id $CONTRACT_ID --source $SOURCE --network $NETWORK"

# ─── helpers ──────────────────────────────────────────────────────────────────

log() { echo ""; echo "▶ $*"; }
sep() { echo "────────────────────────────────────────────────────────────"; }

DEPLOYER_ADDR=$(stellar keys address "$SOURCE" --network "$NETWORK")
TOKEN="${TOKEN:-$DEPLOYER_ADDR}"

# ─── Scenario 1: Bulk escrow creation (50 escrows in one tx) ──────────────────

log "Scenario 1 — create_bulk_escrows: 50 escrows"
sep

REQUESTS="["
for i in $(seq 1 50); do
  AMOUNT=$((i * 1000))
  REQUESTS+="{\"seller\":\"$DEPLOYER_ADDR\",\"amount\":$AMOUNT,\"metadata\":null,\"arbiter\":null,\"items\":null}"
  [ "$i" -lt 50 ] && REQUESTS+=","
done
REQUESTS+="]"

$STELLAR -- create_bulk_escrows \
  --buyer "$DEPLOYER_ADDR" \
  --token "$TOKEN" \
  --requests "$REQUESTS" \
  2>&1 | grep -E "instructions|ledger_entries|read_bytes|write_bytes|result|error" || true

# ─── Scenario 2: Max-item escrow (50 items) ───────────────────────────────────

log "Scenario 2 — create_escrow with MAX_ITEMS_PER_ESCROW (50 items)"
sep

ITEMS="["
for i in $(seq 1 50); do
  ITEMS+="{\"amount\":1000,\"released\":false,\"description\":null}"
  [ "$i" -lt 50 ] && ITEMS+=","
done
ITEMS+="]"

$STELLAR -- create_escrow \
  --buyer  "$DEPLOYER_ADDR" \
  --seller "$DEPLOYER_ADDR" \
  --token  "$TOKEN" \
  --amount 50000 \
  --metadata null \
  --arbiter null \
  --items "$ITEMS" \
  --tracking_id null \
  2>&1 | grep -E "instructions|ledger_entries|read_bytes|write_bytes|result|error" || true

# ─── Scenario 3: Max metadata size (1 KB) ────────────────────────────────────

log "Scenario 3 — create_escrow with 1 KB metadata"
sep

METADATA_HEX=$(python3 -c "print('aa' * 1024)" 2>/dev/null || printf '%0.s61' {1..1024})

$STELLAR -- create_escrow \
  --buyer  "$DEPLOYER_ADDR" \
  --seller "$DEPLOYER_ADDR" \
  --token  "$TOKEN" \
  --amount 1000 \
  --metadata "\"$METADATA_HEX\"" \
  --arbiter null \
  --items null \
  --tracking_id null \
  2>&1 | grep -E "instructions|ledger_entries|read_bytes|write_bytes|result|error" || true

# ─── Scenario 4: batch_collect_fees over 20 escrows ──────────────────────────

log "Scenario 4 — batch_collect_fees: 20 escrow IDs"
sep

IDS="["
for i in $(seq 1 20); do
  IDS+="$i"
  [ "$i" -lt 20 ] && IDS+=","
done
IDS+="]"

$STELLAR -- batch_collect_fees \
  --collector "$DEPLOYER_ADDR" \
  --token     "$TOKEN" \
  --escrow_ids "$IDS" \
  2>&1 | grep -E "instructions|ledger_entries|read_bytes|write_bytes|result|error" || true

# ─── Scenario 5: get_escrows pagination (limit 100) ──────────────────────────

log "Scenario 5 — get_escrows: start=1 limit=100"
sep

$STELLAR -- get_escrows \
  --start 1 \
  --limit 100 \
  2>&1 | grep -E "instructions|ledger_entries|read_bytes|write_bytes|result|error" || true

# ─── Scenario 6: estimate_storage_rent ───────────────────────────────────────

log "Scenario 6 — estimate_storage_rent: escrow_id=1"
sep

$STELLAR -- estimate_storage_rent \
  --escrow_id 1 \
  2>&1 | grep -E "instructions|ledger_entries|read_bytes|write_bytes|result|error" || true

# ─── Scenario 7: get_resource_profile ────────────────────────────────────────

log "Scenario 7 — get_resource_profile"
sep

$STELLAR -- get_resource_profile \
  2>&1 | grep -E "instructions|ledger_entries|read_bytes|write_bytes|result|error" || true

# ─── Scenario 8: Token circuit breaker — pause + blocked create ──────────────

log "Scenario 8 — token circuit breaker: pause_token then create_escrow (expect TokenPaused=169)"
sep

$STELLAR -- pause_token \
  --token "$TOKEN" \
  2>&1 | grep -E "instructions|ledger_entries|result|error" || true

$STELLAR -- create_escrow \
  --buyer  "$DEPLOYER_ADDR" \
  --seller "$DEPLOYER_ADDR" \
  --token  "$TOKEN" \
  --amount 1000 \
  --metadata null \
  --arbiter null \
  --items null \
  --tracking_id null \
  2>&1 | grep -E "instructions|ledger_entries|result|error" || true

$STELLAR -- unpause_token \
  --token "$TOKEN" \
  2>&1 | grep -E "instructions|ledger_entries|result|error" || true

# ─── Scenario 9: Mediation phase — open + dual proposal ──────────────────────

log "Scenario 9 — mediation phase: open_mediation + propose_mediation_settlement"
sep

# Assumes escrow_id=1 is in Disputed state on testnet.
ESCROW_ID="${MEDIATION_ESCROW_ID:-1}"

$STELLAR -- open_mediation \
  --caller "$DEPLOYER_ADDR" \
  --escrow_id "$ESCROW_ID" \
  --window_ledgers 0 \
  2>&1 | grep -E "instructions|ledger_entries|result|error" || true

$STELLAR -- propose_mediation_settlement \
  --proposer "$DEPLOYER_ADDR" \
  --escrow_id "$ESCROW_ID" \
  --seller_amount 500 \
  2>&1 | grep -E "instructions|ledger_entries|result|error" || true

# ─── Summary ──────────────────────────────────────────────────────────────────

sep
echo ""
echo "✅  Simulation complete."
echo "    Review the 'instructions' values above to identify expensive operations."
echo "    Soroban limits: ~100M instructions per tx, ~200 ledger entries."
echo ""
