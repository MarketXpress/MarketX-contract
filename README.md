
# MarketXpress Auction Contract

A comprehensive auction and bidding smart contract built on Stellar's Soroban platform, enabling competitive pricing through time-based auctions with automatic bid processing, reserve prices, and buy-now options.

## Features

- **Time-Based Auctions**: Auctions with configurable start and end times using Stellar's ledger timestamp
- **Reserve Price Mechanism**: Set minimum acceptable prices for auctions
- **Buy-Now Option**: Allow instant purchases at a fixed price
- **Automatic Bid Processing**: Handle concurrent bidding with proper fund escrow
- **Bid History Tracking**: Complete audit trail of all bids
- **Auction Settlement**: Automatic fee calculation and fund distribution
- **Secure Fund Escrow**: Token locking during active auctions with automatic refunds

## Project Structure

```
MarketX-contract/
├── contracts/
│   └── auction/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs          # Main contract implementation
│           ├── types.rs         # Data structures and enums
│           ├── storage.rs       # Storage management functions
│           ├── admin.rs         # Admin authorization
│           ├── test.rs          # Test module entry
│           └── test/
│               ├── mod.rs              # Test utilities
│               ├── auction_test.rs     # Auction creation tests
│               ├── bidding_test.rs     # Bidding mechanism tests
│               └── settlement_test.rs  # Settlement logic tests
└── README.md
```

## Contract Functions

### Initialization

```rust
pub fn initialize(env: Env, admin: Address) -> Result<(), Error>
```

Initialize the contract with an admin address.

### Auction Management

```rust
pub fn create_auction(
    env: Env,
    seller: Address,
    token: Address,
    starting_price: i128,
    reserve_price: i128,
    buy_now_price: Option<i128>,
    duration_seconds: u64,
    fee_bps: u32,
) -> Result<u64, Error>
```

Create a new auction with specified parameters. Returns the auction ID.

```rust
pub fn cancel_auction(
    env: Env,
    auction_id: u64,
    seller: Address,
) -> Result<(), Error>
```

Cancel an auction (only possible if no bids have been placed).

### Bidding

```rust
pub fn place_bid(
    env: Env,
    auction_id: u64,
    bidder: Address,
    amount: i128,
) -> Result<(), Error>
```

Place a bid on an active auction. Automatically refunds previous bidder.

```rust
pub fn buy_now(
    env: Env,
    auction_id: u64,
    buyer: Address,
) -> Result<(), Error>
```

Purchase immediately at the buy-now price, ending the auction.

### Settlement

```rust
pub fn settle_auction(env: Env, auction_id: u64) -> Result<(), Error>
```

Settle an ended auction, distributing funds to seller and admin (fees).

### Query Functions

```rust
pub fn get_auction(env: Env, auction_id: u64) -> Result<Auction, Error>
pub fn get_bid_history(env: Env, auction_id: u64) -> Result<Vec<Bid>, Error>
pub fn get_highest_bid(env: Env, auction_id: u64) -> Result<(Option<Address>, i128), Error>
```

## Building the Contract

### Prerequisites

- Rust toolchain
- Soroban CLI
- wasm32-unknown-unknown target

### Build Commands

```bash
# Navigate to contract directory
cd contracts/auction

# Build for release
cargo build --target wasm32-unknown-unknown --release

# Run tests
cargo test

# Check for errors
cargo check
```

The compiled WASM file will be at `target/wasm32-unknown-unknown/release/marketx_auction.wasm`.

## Testing

The contract includes comprehensive test coverage:

- **Auction Tests**: Creation, validation, cancellation
- **Bidding Tests**: Valid bids, rejections, multiple bidders, refunds
- **Settlement Tests**: Reserve price handling, fee calculation, edge cases
- **Time-Based Tests**: Auction lifecycle, expiration handling

Run tests with:

```bash
cargo test
```

## Usage Example

```rust
// Initialize contract
client.initialize(&admin);

// Create auction
let auction_id = client.create_auction(
    &seller,
    &token_address,
    &1000,           // starting price
    &1500,           // reserve price
    &Some(5000),     // buy-now price
    &86400,          // duration (24 hours)
    &250,            // fee (2.5%)
);

// Place bid
client.place_bid(&auction_id, &bidder, &2000);

// Settle after auction ends
client.settle_auction(&auction_id);
```

## Fee Structure

Fees are calculated in basis points (bps):
- 100 bps = 1%
- 250 bps = 2.5%
- 1000 bps = 10%

Fees are deducted from the final sale price and sent to the admin address.

## Security Features

- **Authorization Checks**: All sensitive operations require proper authentication
- **Concurrent Bidding Safety**: Proper handling of simultaneous bids
- **Fund Escrow**: Tokens are locked in contract during bidding
- **Automatic Refunds**: Previous bidders are refunded when outbid
- **Reserve Price Protection**: Auctions below reserve price are cancelled with refunds

## Error Handling

The contract includes comprehensive error types:

- `NotInitialized`: Contract not initialized
- `AuctionNotFound`: Invalid auction ID
- `AuctionNotActive`: Auction not in active state
- `BidTooLow`: Bid amount insufficient
- `ReservePriceNotMet`: Final bid below reserve
- `InvalidReservePrice`: Reserve below starting price
- `CannotCancelWithBids`: Cannot cancel auction with active bids

## License

Apache 2.0

## Contributing

Contributions are welcome! Please ensure all tests pass before submitting pull requests.

## Links

- [Stellar Documentation](https://developers.stellar.org/)
- [Soroban Documentation](https://soroban.stellar.org/)
- [GitHub Repository](https://github.com/MarketXpress/MarketX-contract)
