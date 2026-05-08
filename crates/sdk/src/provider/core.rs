use std::collections::HashMap;

use electrsd::bitcoind::bitcoincore_rpc::Auth;

use simplicityhl::elements::{Address, Script, Transaction, Txid};

use crate::provider::SimplicityNetwork;
use crate::transaction::{TxReceipt, UTXO};

use super::error::ProviderError;

/// The fallback default fee rate (in sats/kvb) to use when dynamic estimates fail.
pub const DEFAULT_FEE_RATE: f32 = 100.0;
/// The standard timeout duration (in seconds) applied to Esplora REST API requests.
pub const DEFAULT_ESPLORA_TIMEOUT_SECS: u64 = 10;

/// Contains foundational configuration elements required for initializing a generic blockchain provider.
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    /// URL of the target Esplora REST service.
    pub esplora_url: String,
    /// URL of the target direct `elementsd` or `bitcoind` RPC interface.
    pub elements_url: Option<String>,
    /// Authentication settings (e.g. cookie or username/password) for the RPC backend.
    pub auth: Option<Auth>,
}

/// Baseline traits detailing required interaction methods between the SDK client and the underlying blockchain node or API.
pub trait ProviderTrait {
    /// Retrieves the network configured for this provider.
    fn get_network(&self) -> &SimplicityNetwork;

    /// Attempts to broadcast a fully compiled transaction to the configured backend.
    ///
    /// # Errors
    /// Returns a `ProviderError` if network transmission fails, or if the backend explicitly rejects the transaction.
    fn broadcast_transaction(&self, tx: &Transaction) -> Result<TxReceipt<'_>, ProviderError>;

    /// Blocks and repeatedly polls the network until the specified transaction receives its first confirmation.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the network fails or the designated timeout elapses without confirmation.
    fn wait(&self, txid: &Txid) -> Result<(), ProviderError>;

    /// Retrieves the current block height of the network tip.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the backend request fails or the returned height cannot be parsed.
    fn fetch_tip_height(&self) -> Result<u32, ProviderError>;

    /// Retrieves the block timestamp representing network consensus clock tip.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the hash query, subsequent block query, or block parsing fails.
    fn fetch_tip_timestamp(&self) -> Result<u64, ProviderError>;

    /// Retrieves the serialized transaction payload given its hex transaction ID.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the node fails to locate the transaction or serialization fails.
    fn fetch_transaction(&self, txid: &Txid) -> Result<Transaction, ProviderError>;

    /// Fetches all active unspent transaction outputs correlated to a particular public `Address`.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the backend request fails or if UTXO parsing fails.
    fn fetch_address_utxos(&self, address: &Address) -> Result<Vec<UTXO>, ProviderError>;

    /// Fetches all active unspent transaction outputs correlated to a given custom `Script` mapping.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the backend request fails or if UTXO parsing fails.
    fn fetch_scripthash_utxos(&self, script: &Script) -> Result<Vec<UTXO>, ProviderError>;

    /// Fetches network fee estimation models based on varying target confirmation block delays.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the REST request fails or the resulting mappings fail to parse cleanly.
    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, ProviderError>;

    /// Attempts to extract the specific fee rate (in sats/kvb) necessary for the transaction to be confirmed within `target_blocks`.
    ///
    /// # Errors
    /// Passes along `ProviderError` if `fetch_fee_estimates` fails.
    #[allow(clippy::cast_possible_truncation)]
    fn fetch_fee_rate(&self, target_blocks: u32) -> Result<f32, ProviderError> {
        let estimates = self.fetch_fee_estimates()?;
        let target_str = target_blocks.to_string();

        if let Some(&rate) = estimates.get(&target_str) {
            return Ok((rate * 1000.0) as f32); // Convert sat/vB to sats/kvb
        }

        let fallback_targets = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 144, 504, 1008,
        ];

        for &target in fallback_targets.iter().filter(|&&t| t >= target_blocks) {
            let key = target.to_string();

            if let Some(&rate) = estimates.get(&key) {
                return Ok((rate * 1000.0) as f32);
            }
        }

        for &target in &fallback_targets {
            let key = target.to_string();

            if let Some(&rate) = estimates.get(&key) {
                return Ok((rate * 1000.0) as f32);
            }
        }

        Ok(DEFAULT_FEE_RATE)
    }
}
