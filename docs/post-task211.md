# Web3 Dashboard Mock Documentation
## Task #211 — DevEx & Ecosystem Tooling

This document provides a stakeholder-friendly dashboard blueprint that maps contract behavior into product-level KPIs.

---

## 1) Purpose

The MarketX escrow contract already emits enough information for a complete operational dashboard. This mock documentation translates on-chain concepts into business-facing tiles, charts, and workflow views.

Primary audience:
- Product managers
- Operations / support teams
- Community stakeholders
- Non-technical ecosystem partners

---

## 2) Dashboard Information Architecture

### A. Executive Header (Top Row)

1. **Total Escrow Volume (XLM + tokens)**
   - Source: Aggregate of `FundsReleasedEvent.amount`
2. **Open Escrows**
   - Source: `EscrowStatus == Pending || Disputed`
3. **Settlement Rate**
   - Source: `Released / Created` over selected window
4. **Dispute Rate**
   - Source: `RefundRequestedEvent` + disputed status transitions

### B. Activity & Health (Middle Row)

1. **Escrows Created Over Time** (line chart)
   - Source: `EscrowCreatedEvent`
2. **Release vs Refund Mix** (stacked bars)
   - Source: `StatusChangeEvent` terminal states
3. **Fee Revenue Trend** (area chart)
   - Source: `FeeCollectedEvent`, `FeesWithdrawnEvent`
4. **Median Time to Settlement**
   - Source: `created_ledger -> released/refunded ledger`

### C. Operations Queue (Bottom Row)

1. **Aging Pending Escrows**
   - Highlight by ledger age buckets (0-1d, 1-3d, 3d+)
2. **Disputes Needing Arbiter Attention**
   - Source: disputed escrows without terminal resolution
3. **Recently Expired Unfunded Escrows**
   - Source: `EscrowExpiredEvent`

---

## 3) Contract Data Mapping (Indexer Specification)

### Events to index

- `EscrowCreatedEvent`
- `FundsReleasedEvent`
- `StatusChangeEvent`
- `RefundRequestedEvent`
- `EscrowExpiredEvent`
- `FeeCollectedEvent`
- `FeesWithdrawnEvent`

### Read methods used for enrichment

- `get_escrow(escrow_id)`
- `get_escrow_metadata(escrow_id, caller)`
- `resource_profile()`
- `analytics_summary()` (if enabled in deployment profile)

> Recommendation: keep event ingestion append-only and perform reconciliation jobs with `get_escrow` for any escrow IDs that hit disputed or expired states.

---

## 4) KPI Definitions (Business-safe wording)

- **Gross Settled Volume**: Sum of released amounts before fee withdrawal events.
- **Protocol Fee Accrual**: Sum of `FeeCollectedEvent.fee_amount` by token.
- **Protocol Fee Realization**: Sum of `FeesWithdrawnEvent.amount` by token.
- **Escrow Completion Rate**: Count(Released) / Count(Created).
- **Dispute Escalation Rate**: Count(Disputed transitions) / Count(Created).
- **Refund Rate**: Count(Refunded) / Count(Created).

---

## 5) Dashboard Mock (Text Wireframe)

```text
┌──────────────────────────────────────────────────────────────────────────┐
│ MarketX Protocol Dashboard                               [7d] [30d] [90d]│
├───────────────┬───────────────┬───────────────┬──────────────────────────┤
│ Volume        │ Open Escrows  │ Settlement %  │ Dispute %                │
│ 1,284,331 XLM │ 184           │ 94.2%         │ 1.8%                     │
├──────────────────────────────────────────────────────────────────────────┤
│ Escrows Created (line)                 │ Fee Revenue by Token (stacked) │
├──────────────────────────────────────────────────────────────────────────┤
│ Release vs Refund Mix (bars)           │ Median Settlement Time          │
├──────────────────────────────────────────────────────────────────────────┤
│ Aging Pending Escrows                  │ Disputes Awaiting Resolution    │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## 6) Stakeholder Views

- **Product View**: growth, completion, dispute trends
- **Support View**: pending/disputed queues, top risky escrows
- **Treasury View**: accrued vs withdrawn fees by token
- **Community View**: transparent protocol health snapshots

---

## 7) Delivery Checklist

- [x] Data source mapping to real contract events/methods
- [x] KPI definitions for non-technical audiences
- [x] Operational queues for day-to-day monitoring
- [x] Wireframe mock suitable for design handoff
