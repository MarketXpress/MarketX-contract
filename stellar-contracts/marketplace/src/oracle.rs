use soroban_sdk::{Address, Env, Symbol};

use crate::errors::Error;
use crate::reflector::{PriceData, ReflectorHelper};
use crate::storage::{
    add_external_price_record, add_price_record, get_external_price_history, get_last_price_update,
    get_oracle_config, get_price_history, set_last_price_update,
};
use crate::types::{OracleConfig, PriceRecord, PriceSource};

/// Oracle service for fetching and validating prices from the Reflector Oracle.
/// Provides price fetching, staleness checks, manipulation detection, and validation.
pub struct OracleService;

impl OracleService {
    /// Checks if a price is stale based on the staleness threshold.
    ///
    /// # Arguments
    /// * `price_timestamp` - Timestamp when the price was recorded
    /// * `current_timestamp` - Current ledger timestamp
    /// * `threshold` - Maximum allowed age in seconds
    ///
    /// # Returns
    /// * `true` if the price is older than the threshold
    pub fn is_price_stale(
        price_timestamp: u64,
        current_timestamp: u64,
        threshold: u64,
    ) -> bool {
        current_timestamp.saturating_sub(price_timestamp) > threshold
    }

    /// Detects potential price manipulation by comparing current price to TWAP.
    ///
    /// # Arguments
    /// * `current_price` - The current spot price
    /// * `twap` - Time-weighted average price
    /// * `threshold_bps` - Maximum allowed deviation in basis points (e.g., 1000 = 10%)
    ///
    /// # Returns
    /// * `true` if manipulation is detected (deviation exceeds threshold)
    pub fn detect_manipulation(
        current_price: i128,
        twap: i128,
        threshold_bps: u32,
    ) -> bool {
        if twap == 0 {
            return false;
        }
        let deviation = ((current_price - twap).abs() * 10000) / twap;
        deviation > threshold_bps as i128
    }

    /// Validates that a product price is within acceptable range of oracle price.
    ///
    /// # Arguments
    /// * `oracle_price` - Reference price from the oracle
    /// * `product_price` - Proposed product price to validate
    /// * `tolerance_bps` - Maximum allowed deviation in basis points (e.g., 2000 = 20%)
    ///
    /// # Returns
    /// * `Ok(())` if price is within tolerance
    /// * `Err(PriceOutOfRange)` if price deviates too much
    pub fn validate_product_price(
        oracle_price: i128,
        product_price: u128,
        tolerance_bps: u32,
    ) -> Result<(), Error> {
        if oracle_price <= 0 {
            return Ok(());
        }

        let product_price_i128 = i128::try_from(product_price).map_err(|_| Error::PriceOutOfRange)?;

        let tolerance = (oracle_price.abs() * tolerance_bps as i128) / 10000;
        let min_price = oracle_price.saturating_sub(tolerance);
        let max_price = oracle_price.saturating_add(tolerance);

        if product_price_i128 < min_price || product_price_i128 > max_price {
            return Err(Error::PriceOutOfRange);
        }

        Ok(())
    }

    /// Fetches the current price for a Stellar asset with staleness validation.
    /// Falls back to cached price if oracle price is stale but cache is fresh.
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `asset_address` - Address of the Stellar token
    ///
    /// # Returns
    /// * `Ok(PriceData)` - Price and timestamp
    /// * `Err(OracleNotConfigured)` - Oracle not set up or disabled
    /// * `Err(OraclePriceUnavailable)` - Price not available from oracle
    /// * `Err(OraclePriceStale)` - Price is too old
    pub fn get_stellar_asset_price(
        e: &Env,
        asset_address: &Address,
    ) -> Result<PriceData, Error> {
        let config = get_oracle_config(e).ok_or(Error::OracleNotConfigured)?;

        if !config.is_enabled {
            return Err(Error::OracleNotConfigured);
        }

        let current_time = e.ledger().timestamp();

        // Enforce update frequency - return cached price if fetching too soon
        if config.update_frequency > 0 {
            let last_update = get_last_price_update(e);
            if current_time.saturating_sub(last_update) < config.update_frequency {
                let history = get_price_history(e, asset_address);
                if !history.is_empty() {
                    let last_record = history.last().unwrap();
                    return Ok(PriceData {
                        price: last_record.price,
                        timestamp: last_record.timestamp,
                    });
                }
            }
        }

        let price_data = ReflectorHelper::get_stellar_asset_price(
            e,
            &config.stellar_oracle,
            asset_address,
        )
        .ok_or(Error::OraclePriceUnavailable)?;
        if Self::is_price_stale(price_data.timestamp, current_time, config.staleness_threshold) {
            let history = get_price_history(e, asset_address);
            if !history.is_empty() {
                let last_record = history.last().unwrap();
                if !Self::is_price_stale(
                    last_record.timestamp,
                    current_time,
                    config.staleness_threshold,
                ) {
                    return Ok(PriceData {
                        price: last_record.price,
                        timestamp: last_record.timestamp,
                    });
                }
            }
            return Err(Error::OraclePriceStale);
        }

        // Check for price manipulation if threshold is configured
        if config.price_deviation_threshold > 0 {
            if let Some(twap) = ReflectorHelper::get_stellar_asset_twap(
                e,
                &config.stellar_oracle,
                asset_address,
                5, // Use 5 periods for TWAP
            ) {
                if Self::detect_manipulation(
                    price_data.price,
                    twap,
                    config.price_deviation_threshold,
                ) {
                    return Err(Error::OraclePriceManipulated);
                }
            }
        }

        let record = PriceRecord {
            price: price_data.price,
            timestamp: price_data.timestamp,
            source: PriceSource::Oracle,
        };
        add_price_record(e, asset_address, &record);
        set_last_price_update(e, current_time);

        Ok(price_data)
    }

    /// Fetches the current price for an external asset (BTC, ETH, etc.).
    /// Falls back to cached price if oracle price is stale but cache is fresh.
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `symbol` - Symbol of the external asset (e.g., "BTC", "ETH")
    ///
    /// # Returns
    /// * `Ok(PriceData)` - Price and timestamp
    /// * `Err(OracleNotConfigured)` - Oracle not set up or disabled
    /// * `Err(OraclePriceUnavailable)` - Price not available from oracle
    /// * `Err(OraclePriceStale)` - Price is too old
    pub fn get_external_asset_price(
        e: &Env,
        symbol: &Symbol,
    ) -> Result<PriceData, Error> {
        let config = get_oracle_config(e).ok_or(Error::OracleNotConfigured)?;

        if !config.is_enabled {
            return Err(Error::OracleNotConfigured);
        }

        let current_time = e.ledger().timestamp();

        // Enforce update frequency - return cached price if fetching too soon
        if config.update_frequency > 0 {
            let last_update = get_last_price_update(e);
            if current_time.saturating_sub(last_update) < config.update_frequency {
                let history = get_external_price_history(e, symbol);
                if !history.is_empty() {
                    let last_record = history.last().unwrap();
                    return Ok(PriceData {
                        price: last_record.price,
                        timestamp: last_record.timestamp,
                    });
                }
            }
        }

        let price_data = ReflectorHelper::get_external_asset_price(
            e,
            &config.external_oracle,
            symbol,
        )
        .ok_or(Error::OraclePriceUnavailable)?;

        if Self::is_price_stale(price_data.timestamp, current_time, config.staleness_threshold) {
            // Try fallback to cached price
            let history = get_external_price_history(e, symbol);
            if !history.is_empty() {
                let last_record = history.last().unwrap();
                if !Self::is_price_stale(
                    last_record.timestamp,
                    current_time,
                    config.staleness_threshold,
                ) {
                    return Ok(PriceData {
                        price: last_record.price,
                        timestamp: last_record.timestamp,
                    });
                }
            }
            return Err(Error::OraclePriceStale);
        }

        // Cache the price
        let record = PriceRecord {
            price: price_data.price,
            timestamp: price_data.timestamp,
            source: PriceSource::Oracle,
        };
        add_external_price_record(e, symbol, &record);

        Ok(price_data)
    }

    /// Fetches the TWAP for a Stellar asset from the oracle.
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `asset_address` - Address of the Stellar token
    /// * `records` - Number of records to use for TWAP calculation
    ///
    /// # Returns
    /// * `Ok(i128)` - TWAP price
    /// * `Err(OracleNotConfigured)` - Oracle not set up or disabled
    /// * `Err(OraclePriceUnavailable)` - TWAP not available
    pub fn get_stellar_asset_twap(
        e: &Env,
        asset_address: &Address,
        records: u32,
    ) -> Result<i128, Error> {
        let config = get_oracle_config(e).ok_or(Error::OracleNotConfigured)?;

        if !config.is_enabled {
            return Err(Error::OracleNotConfigured);
        }

        ReflectorHelper::get_stellar_asset_twap(e, &config.stellar_oracle, asset_address, records)
            .ok_or(Error::OraclePriceUnavailable)
    }

    /// Converts an amount from one asset to another using oracle prices.
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `amount` - Amount to convert
    /// * `from_asset` - Source asset address
    /// * `to_asset` - Target asset address
    ///
    /// # Returns
    /// * `Ok(i128)` - Converted amount
    /// * `Err` - If price fetching fails
    pub fn convert_price(
        e: &Env,
        amount: i128,
        from_asset: &Address,
        to_asset: &Address,
    ) -> Result<i128, Error> {
        let from_price = Self::get_stellar_asset_price(e, from_asset)?;
        let to_price = Self::get_stellar_asset_price(e, to_asset)?;

        if to_price.price == 0 {
            return Err(Error::OraclePriceUnavailable);
        }

        let result = amount
            .checked_mul(from_price.price)
            .ok_or(Error::FeeOverflow)?
            .checked_div(to_price.price)
            .ok_or(Error::FeeOverflow)?;

        Ok(result)
    }

    /// Gets oracle configuration and last update timestamp.
    ///
    /// # Arguments
    /// * `e` - The environment
    ///
    /// # Returns
    /// * `Ok((OracleConfig, u64))` - Config and last update timestamp
    /// * `Err(OracleNotConfigured)` - Oracle not configured
    pub fn get_oracle_info(e: &Env) -> Result<(OracleConfig, u64), Error> {
        let config = get_oracle_config(e).ok_or(Error::OracleNotConfigured)?;
        let last_update = get_last_price_update(e);
        Ok((config, last_update))
    }

    /// Validates that a payment asset is supported by checking if price is available.
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `asset_address` - Address of the payment asset
    ///
    /// # Returns
    /// * `Ok(())` - Asset is supported
    /// * `Err(PaymentAssetNotSupported)` - Asset not tracked by oracle
    pub fn validate_payment_asset(e: &Env, asset_address: &Address) -> Result<(), Error> {
        let config = get_oracle_config(e).ok_or(Error::OracleNotConfigured)?;

        if !config.is_enabled {
            return Ok(());
        }

        ReflectorHelper::get_stellar_asset_price(e, &config.stellar_oracle, asset_address)
            .ok_or(Error::PaymentAssetNotSupported)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_price_stale() {
        assert!(!OracleService::is_price_stale(100, 200, 300));
        assert!(OracleService::is_price_stale(100, 500, 300));
        assert!(!OracleService::is_price_stale(100, 400, 300));
    }

    #[test]
    fn test_detect_manipulation() {
        assert!(!OracleService::detect_manipulation(105, 100, 1000));
        assert!(OracleService::detect_manipulation(115, 100, 1000));
        assert!(!OracleService::detect_manipulation(100, 0, 1000));
    }

    #[test]
    fn test_validate_product_price() {
        assert!(OracleService::validate_product_price(1000, 1000, 2000).is_ok());
        assert!(OracleService::validate_product_price(1000, 1200, 2000).is_ok());
        assert!(OracleService::validate_product_price(1000, 800, 2000).is_ok());
        assert!(OracleService::validate_product_price(1000, 1300, 2000).is_err());
        assert!(OracleService::validate_product_price(1000, 700, 2000).is_err());
        assert!(OracleService::validate_product_price(0, 1000, 2000).is_ok());
    }
}
