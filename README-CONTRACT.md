MarketX-contract
# Order Management Contract

This project implements a **fully on-chain order management system** for the Stellar/Soroban blockchain, integrated with an escrow system.

## Features

- Create, modify, and cancel orders
- Track order status: Created → Shipped → Delivered → Disputed → Refunded
- Ship orders with tracking references
- Delivery confirmation
- Dispute handling
- Escrow integration: lock, release, refund funds
- Supports high transaction volume and persistent state
- Fully tested with integration tests

## File Structure
```test
contracts/
├── hello-world/
│ ├── src/
│ │ ├── lib.rs # The Main contract
│ │ ├── order.rs # Order logic
│ │ ├── escrow.rs # Escrow logic
│ │ └── tests.rs # Integration tests
│ └── Cargo.toml
```


## How to Test

```bash
# Run unit and integration tests
cargo test
```
All tests simulate:

1. Order lifecycle: creation, shipping, delivery, escrow release

2. Dispute and refund flow: dispute raised, admin resolves, escrow refunds
