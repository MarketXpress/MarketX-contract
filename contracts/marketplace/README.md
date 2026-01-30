# MarketX Marketplace Smart Contract

A production-grade decentralized marketplace contract built on Soroban (Stellar's smart contract platform). This contract enables secure product listing, seller verification, and transaction fee management.

## Features

### Product Management
- **Product Listing**: Verified sellers can list products with metadata, pricing, and stock information
- **Categorization**: Flexible product categorization with category-specific fee rates
- **Product Updates**: Sellers can update prices, stock quantities, and product status
- **Search & Filtering**: Find products by category, seller, or price range

### Seller Management
- **Registration**: New sellers can register with profile metadata
- **Verification**: Admin-controlled seller verification process
- **Status Management**: Suspend/unsuspend sellers for platform violations
- **Reputation System**: Rating system to track seller credibility

### Fee Management
- **Flexible Fee Structure**: Base marketplace fee + category-specific commissions
- **Transparent Calculation**: Basis point-based fee system (100 = 1%)
- **Fee Collection**: Track and manage collected marketplace fees
- **Admin Control**: Update fee rates without contract redeployment

### Marketplace Administration
- **Emergency Pause**: Pause marketplace operations in case of security issues
- **Configuration Updates**: Adjust fee rates and marketplace parameters
- **Event Logging**: Comprehensive event emission for all marketplace activities
- **Storage Management**: Automatic TTL extension for persistent data

## Technical Architecture

### Modular Structure

```
marketplace/
├── src/
│   ├── lib.rs              # Main contract implementation
│   ├── types.rs            # Data structures and enums
│   ├── errors.rs           # Error codes
│   ├── events.rs           # Event definitions
│   ├── storage.rs          # Storage getters/setters
│   └── test.rs             # Unit and integration tests
├── Cargo.toml              # Contract dependencies
└── README.md               # This file
```

### Data Structures

#### MarketplaceConfig
- Admin address
- Base fee rate (basis points)
- Paused status
- Total products and sellers counters
- Last update timestamp

#### Seller
- Address
- Verification status (Unverified/Verified/Suspended)
- Rating (0-500, where 500 = 5 stars)
- Sales statistics
- Metadata (JSON encoded)

#### Product
- Unique ID
- Seller address
- Name and description
- Category ID
- Price (in stroops)
- Stock quantity
- Status (Active/Delisted/OutOfStock)
- Rating
- Purchase count
- Metadata (JSON encoded)

#### Category
- ID
- Name and description
- Commission rate (basis points)
- Active status

### Storage Management

The contract implements efficient storage with automatic TTL extension:
- **Instance Storage**: 30-day TTL for configuration
- **Persistent Storage**: 90-day TTL for user data
- **Key Isolation**: Type-safe storage keys using enums

## API Overview

### Initialization

```rust
pub fn initialize(e: &Env, admin: Address, base_fee_rate: u32) -> Result<(), Error>
```

Initialize the marketplace with an admin address and base fee rate (in basis points).

### Seller Management

```rust
pub fn register_seller(e: &Env, seller: Address, metadata: String) -> Result<(), Error>
pub fn get_seller(e: &Env, seller_address: Address) -> Result<Seller, Error>
pub fn verify_seller(e: &Env, admin: Address, seller_address: Address) -> Result<(), Error>
pub fn suspend_seller(e: &Env, admin: Address, seller_address: Address) -> Result<(), Error>
pub fn unsuspend_seller(e: &Env, admin: Address, seller_address: Address) -> Result<(), Error>
pub fn update_seller_rating(e: &Env, admin: Address, seller_address: Address, new_rating: u32) -> Result<(), Error>
```

### Category Management

```rust
pub fn create_category(
    e: &Env,
    admin: Address,
    id: u32,
    name: String,
    description: String,
    commission_rate: u32,
) -> Result<(), Error>
pub fn get_category(e: &Env, id: u32) -> Result<Category, Error>
```

### Product Management

```rust
pub fn add_product(
    e: &Env,
    seller: Address,
    name: String,
    description: String,
    category_id: u32,
    price: u128,
    stock_quantity: u64,
    metadata: String,
) -> Result<u64, Error>
pub fn get_product(e: &Env, product_id: u64) -> Result<Product, Error>
pub fn update_product(
    e: &Env,
    seller: Address,
    product_id: u64,
    price: u128,
    stock_quantity: u64,
    status: u32,
) -> Result<(), Error>
pub fn delist_product(e: &Env, seller: Address, product_id: u64) -> Result<(), Error>
pub fn update_product_rating(e: &Env, seller: Address, product_id: u64, new_rating: u32) -> Result<(), Error>
```

### Search & Filtering

```rust
pub fn get_products_by_seller(e: &Env, seller_address: Address) -> Result<Vec<u64>, Error>
pub fn get_products_by_category(e: &Env, category_id: u32) -> Result<Vec<u64>, Error>
pub fn get_products_by_price_range(
    e: &Env,
    min_price: u128,
    max_price: u128,
    offset: u32,
    limit: u32,
) -> Result<Vec<Product>, Error>
```

### Fee Management

```rust
pub fn calculate_fee(e: &Env, amount: u128, category_id: Option<u32>) -> Result<u128, Error>
pub fn record_fee_collection(e: &Env, admin: Address, amount: u128) -> Result<(), Error>
pub fn get_total_fees(e: &Env) -> Result<u128, Error>
pub fn set_fee_rate(e: &Env, admin: Address, new_rate: u32) -> Result<(), Error>
pub fn set_category_fee_rate(e: &Env, admin: Address, category_id: u32, rate: u32) -> Result<(), Error>
```

### Marketplace Administration

```rust
pub fn set_paused(e: &Env, admin: Address, paused: bool) -> Result<(), Error>
pub fn is_paused(e: &Env) -> Result<bool, Error>
pub fn get_config(e: &Env) -> Result<MarketplaceConfig, Error>
pub fn get_stats(e: &Env) -> Result<(u64, u64, u128), Error>
```

## Error Handling

The contract implements comprehensive error handling with specific error codes:

| Error | Code | Description |
|-------|------|-------------|
| AlreadyInitialized | 500 | Contract already initialized |
| NotInitialized | 501 | Contract not initialized |
| Unauthorized | 502 | Caller lacks required permissions |
| SellerNotFound | 503 | Seller doesn't exist |
| ProductNotFound | 504 | Product doesn't exist |
| InvalidInput | 505 | Invalid parameters provided |
| InsufficientBalance | 506 | Insufficient balance |
| SellerNotVerified | 507 | Seller not verified |
| ProductAlreadyExists | 508 | Product already exists |
| CategoryNotFound | 509 | Category doesn't exist |
| MarketplacePaused | 510 | Marketplace is paused |
| OutOfStock | 511 | Product out of stock |
| InvalidProductStatus | 512 | Invalid product status value |
| InvalidSellerStatus | 513 | Invalid seller status value |
| FeeOverflow | 514 | Fee calculation overflow |
| PointsOverflow | 515 | Points calculation overflow |
| CategoryAlreadyExists | 516 | Category already exists |
| OperationFailed | 517 | Operation failed |
| InvalidMetadata | 518 | Invalid metadata provided |
| SellerSuspended | 519 | Seller is suspended |

## Events

The contract emits events for all major operations:

- `InitializedEventData`: Marketplace initialization
- `SellerRegisteredEventData`: New seller registration
- `SellerVerifiedEventData`: Seller verification
- `SellerSuspendedEventData`: Seller suspension
- `SellerUnsuspendedEventData`: Seller unsuspension
- `CategoryCreatedEventData`: Category creation
- `ProductListedEventData`: Product listing
- `ProductUpdatedEventData`: Product update
- `ProductDelistedEventData`: Product delisting
- `ProductPurchasedEventData`: Product purchase
- `MarketplacePausedEventData`: Marketplace pause/unpause
- `FeeRateUpdatedEventData`: Fee rate update
- `FeeCollectedEventData`: Fee collection
- `SellerRatingUpdatedEventData`: Seller rating update
- `ProductRatingUpdatedEventData`: Product rating update

## Testing

### Unit Tests

Comprehensive unit test suite covering all major functionality:

```bash
cargo test --lib
```

Tests cover:
- Initialization and configuration management
- Seller registration, verification, and status management
- Category creation and management
- Product listing, updates, and status changes
- Fee calculations and collection
- Marketplace pause/unpause functionality
- Search and filtering operations
- Error handling and edge cases

## Building the Contract

### Prerequisites

- Rust 1.70+ with `wasm32-unknown-unknown` target
- Soroban CLI (optional, for deployment)

### Build for Local Testing

```bash
cd stellar-contracts/marketplace
cargo build --lib
```

### Build WASM for Testnet

```bash
cargo build --target wasm32-unknown-unknown --release
```

The WASM binary will be at: `target/wasm32-unknown-unknown/release/marketx_contract.wasm`

### Deploy to Testnet

1. Get testnet credentials from [Stellar Lab](https://lab.stellar.org/)
2. Use Soroban CLI to deploy:

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/marketx_contract.wasm \
  --network testnet \
  --source YOUR_STELLAR_ADDRESS
```

## Gas Optimization

The contract is optimized for gas efficiency:

1. **Minimal Storage Access**: Batch related operations to reduce storage calls
2. **Efficient Encoding**: Use compact native types (u32, u64, u128)
3. **Smart TTL Management**: Extend TTL only during state mutations
4. **Pagination**: Limit search results to prevent excessive iteration
5. **Type-Safe Keys**: Enum-based storage keys prevent lookup errors

Workspace-level release profile optimization:
```toml
[profile.release]
opt-level = "z"
overflow-checks = true
debug = 0
strip = "symbols"
debug-assertions = false
panic = "abort"
codegen-units = 1
lto = true
```

## Best Practices Implemented

Based on reference contracts analysis:

1. **Modular Architecture**: Separated concerns (types, errors, events, storage, logic)
2. **Comprehensive Error Handling**: Specific error codes for all failure scenarios
3. **Event Emission**: Proper event logging for all state changes
4. **Storage Efficiency**: Type-safe storage keys and TTL management
5. **Documentation**: Extensive inline documentation and examples
6. **Testing**: Unit and integration tests with high coverage
7. **Code Reusability**: Helper functions and trait implementations

## Performance Characteristics

- **Product Search**: O(n) paginated for scalability
- **Seller Lookup**: O(1) direct address-based lookup
- **Category Operations**: O(1) ID-based lookup
- **Fee Calculation**: O(1) constant time
- **Total Fees**: O(1) single storage access

## License

This contract is part of the MarketX platform and is subject to the project's licensing terms.

