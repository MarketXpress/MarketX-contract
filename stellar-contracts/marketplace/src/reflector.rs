use soroban_sdk::{contractclient, contracttype, Address, Env, Symbol, Vec};

/// Quoted asset definition for Reflector Oracle.
/// Stellar for on-chain tokens, Other for external assets (BTC, ETH, etc.)
#[contracttype(export = false)]
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Asset {
    Stellar(Address),
    Other(Symbol),
}

/// Price record returned by the oracle.
#[contracttype(export = false)]
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct PriceData {
    pub price: i128,
    pub timestamp: u64,
}

/// SEP-40 Oracle interface for Reflector Oracle on Stellar/Soroban.
/// Documentation: https://reflector.network/
/// Testnet Oracle: CAVLP5DH2GJPZMVO7IJY4CVOD5MWEFTJFVPD2YY2FQXOQHRGHK4D6HLP
#[allow(dead_code)]
#[contractclient(name = "ReflectorClient")]
pub trait ReflectorOracle {
    /// Base oracle symbol the price is reported in (usually USD)
    fn base(e: Env) -> Asset;

    /// All assets quoted by the contract
    fn assets(e: Env) -> Vec<Asset>;

    /// Number of decimal places used to represent price for all assets
    fn decimals(e: Env) -> u32;

    /// Quotes asset price in base asset at specific timestamp
    fn price(e: Env, asset: Asset, timestamp: u64) -> Option<PriceData>;

    /// Quotes the most recent price for an asset
    fn lastprice(e: Env, asset: Asset) -> Option<PriceData>;

    /// Quotes last N price records for the given asset
    fn prices(e: Env, asset: Asset, records: u32) -> Option<Vec<PriceData>>;

    /// Quotes the most recent cross price record for the pair of assets
    fn x_last_price(e: Env, base_asset: Asset, quote_asset: Asset) -> Option<PriceData>;

    /// Quotes the cross price for the pair of assets at specific timestamp
    fn x_price(e: Env, base_asset: Asset, quote_asset: Asset, timestamp: u64) -> Option<PriceData>;

    /// Quotes last N cross price records for the pair of assets
    fn x_prices(
        e: Env,
        base_asset: Asset,
        quote_asset: Asset,
        records: u32,
    ) -> Option<Vec<PriceData>>;

    /// Quotes the time-weighted average price for the given asset over N recent records
    fn twap(e: Env, asset: Asset, records: u32) -> Option<i128>;

    /// Quotes the time-weighted average cross price for the given asset pair over N recent records
    fn x_twap(e: Env, base_asset: Asset, quote_asset: Asset, records: u32) -> Option<i128>;

    /// Price feed resolution (tick period timeframe, in seconds - 5 minutes by default)
    fn resolution(e: Env) -> u32;

    /// Historical records retention period, in seconds (24 hours by default)
    fn period(e: Env) -> Option<u64>;

    /// The most recent price update timestamp
    fn last_timestamp(e: Env) -> u64;

    /// Contract protocol version
    fn version(e: Env) -> u32;

    /// Contract admin address
    fn admin(e: Env) -> Option<Address>;
}

/// Convert token address to Oracle Asset type
pub fn stellar_asset(address: Address) -> Asset {
    Asset::Stellar(address)
}

/// Convert symbol to Oracle Asset type
pub fn symbol_asset(symbol: Symbol) -> Asset {
    Asset::Other(symbol)
}

/// Helper functions for interacting with the Reflector Oracle.
pub struct ReflectorHelper;

impl ReflectorHelper {
    /// Fetches the last price for a Stellar asset (XLM, USDC, etc.).
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `oracle_address` - Address of the Reflector oracle contract
    /// * `asset_address` - Address of the Stellar token
    ///
    /// # Returns
    /// * `Option<PriceData>` - Price and timestamp if available
    pub fn get_stellar_asset_price(
        e: &Env,
        oracle_address: &Address,
        asset_address: &Address,
    ) -> Option<PriceData> {
        let client = ReflectorClient::new(e, oracle_address);
        client.lastprice(&stellar_asset(asset_address.clone()))
    }

    /// Fetches the last price for an external asset (BTC, ETH, etc.).
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `oracle_address` - Address of the Reflector oracle contract
    /// * `symbol` - Symbol of the external asset (e.g., "BTC", "ETH")
    ///
    /// # Returns
    /// * `Option<PriceData>` - Price and timestamp if available
    pub fn get_external_asset_price(
        e: &Env,
        oracle_address: &Address,
        symbol: &Symbol,
    ) -> Option<PriceData> {
        let client = ReflectorClient::new(e, oracle_address);
        client.lastprice(&symbol_asset(symbol.clone()))
    }

    /// Fetches the TWAP (Time-Weighted Average Price) for a Stellar asset.
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `oracle_address` - Address of the Reflector oracle contract
    /// * `asset_address` - Address of the Stellar token
    /// * `records` - Number of records to use for TWAP calculation
    ///
    /// # Returns
    /// * `Option<i128>` - TWAP price if available
    pub fn get_stellar_asset_twap(
        e: &Env,
        oracle_address: &Address,
        asset_address: &Address,
        records: u32,
    ) -> Option<i128> {
        let client = ReflectorClient::new(e, oracle_address);
        client.twap(&stellar_asset(asset_address.clone()), &records)
    }
}
