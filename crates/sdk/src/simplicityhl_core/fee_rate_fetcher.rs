use std::collections::HashMap;

/// Fee estimates response from Esplora.
/// Key: confirmation target (in blocks as string), Value: fee rate (sat/vB).
pub type FeeEstimates = HashMap<String, f64>;

/// Default Target blocks value for using `DEFAULT_FEE_RATE` later
pub const DEFAULT_TARGET_BLOCKS: u32 = 0;

/// Default fallback fee rate in sats/kvb (0.10 sat/vB).
/// Higher than LWK default to meet Liquid minimum relay fee requirements.
pub const DEFAULT_FEE_RATE: f32 = 100.0;

/// Error type for Esplora sync operations.
#[derive(thiserror::Error, Debug)]
pub enum FeeFetcherError {
    #[error("HTTP request failed: {0}")]
    Request(String),

    #[error("Failed to deserialize response: {0}")]
    Deserialize(String),

    #[error("Invalid txid format: {0}")]
    InvalidTxid(String),
}

pub trait SyncFeeFetcher {
    /// Fetch fee estimates for various confirmation targets.
    ///
    /// # Errors
    ///
    /// Returns error if the HTTP request fails or response body cannot be parsed.
    fn fetch_fee_estimates() -> Result<FeeEstimates, FeeFetcherError>;

    /// Get fee rate for a specific confirmation target.
    ///
    /// Fetches fee estimates from Esplora and returns the rate for the given target.
    /// If the exact target is not available, falls back to higher targets.
    ///
    /// # Arguments
    ///
    /// * `target_blocks` - Desired confirmation target in blocks (1-25, 144, 504, 1008)
    ///
    /// # Returns
    ///
    /// Fee rate in sats/kvb (satoshis per 1000 virtual bytes).
    /// Multiply Esplora's sat/vB value by 1000.
    ///
    /// # Errors
    ///
    /// Returns an error if the `fetch_fee_estimates()` fails or no suitable fee rate is found.
    #[allow(clippy::cast_possible_truncation)]
    fn get_fee_rate(target_blocks: u32) -> Result<f32, FeeFetcherError> {
        if target_blocks == 0 {
            return Ok(DEFAULT_FEE_RATE);
        }

        let estimates = Self::fetch_fee_estimates()?;

        let target_str = target_blocks.to_string();
        if let Some(&rate) = estimates.get(&target_str) {
            return Ok((rate * 1000.0) as f32); // Convert sat/vB to sats/kvb
        }

        // Fall back to higher targets (lower fee rates)
        // Available targets: 1-25, 144, 504, 1008
        let fallback_targets = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 144, 504, 1008,
        ];

        for &target in fallback_targets.iter().filter(|&&t| t >= target_blocks) {
            let key = target.to_string();
            if let Some(&rate) = estimates.get(&key) {
                return Ok((rate * 1000.0) as f32);
            }
        }

        // If no higher target found, try any available rate (use lowest target = highest rate)
        for &target in &fallback_targets {
            let key = target.to_string();
            if let Some(&rate) = estimates.get(&key) {
                return Ok((rate * 1000.0) as f32);
            }
        }

        Err(FeeFetcherError::Request(
            "No fee estimates available".to_string(),
        ))
    }
}

pub struct EsploraFeeFetcher;

impl SyncFeeFetcher for EsploraFeeFetcher {
    /// Fetch fee estimates for various confirmation targets.
    ///
    /// Uses the `GET /fee-estimates` endpoint.
    /// Note: Liquid testnet typically returns empty results, so callers should
    /// use a fallback rate (see `config.fee.fallback_rate`).
    ///
    /// Returns a map where key is confirmation target (blocks) and value is fee rate (sat/vB).
    ///
    /// Example response: `{ "1": 87.882, "2": 87.882, ..., "144": 1.027, "1008": 1.027 }`
    fn fetch_fee_estimates() -> Result<FeeEstimates, FeeFetcherError> {
        const ESPLORA_URL: &str = "https://blockstream.info/liquidtestnet/api";

        let url = format!("{ESPLORA_URL}/fee-estimates");
        let response = minreq::get(&url)
            .send()
            .map_err(|e| FeeFetcherError::Request(e.to_string()))?;

        if response.status_code != 200 {
            return Err(FeeFetcherError::Request(format!(
                "HTTP {}: {}",
                response.status_code, response.reason_phrase
            )));
        }

        let estimates: FeeEstimates = response
            .json()
            .map_err(|e| FeeFetcherError::Deserialize(e.to_string()))?;

        Ok(estimates)
    }
}
