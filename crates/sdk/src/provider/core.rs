use std::collections::HashMap;

#[cfg(feature = "provider")]
use bitcoincore_rpc::Auth;

use simplicityhl::elements::{Address, Script, Transaction, Txid};

use crate::provider::SimplicityNetwork;
use crate::transaction::{TxReceipt, UTXO};

use super::error::ProviderError;

/// The fee rate, in sats/kvb, that [`ProviderTrait::fetch_fee_rate`] returns when the provider
/// doesn't report any estimate.
pub const DEFAULT_FEE_RATE: f32 = 100.0;
/// The default timeout, in seconds, of each request an `EsploraProvider` makes.
pub const DEFAULT_ESPLORA_TIMEOUT_SECS: u64 = 10;
/// How many times `EsploraProvider::wait` polls for a confirmation by default.
pub const DEFAULT_CONFIRMATION_ATTEMPTS: u32 = 9;

/// The endpoints of a provider: an Esplora URL, and for a regtest node, its RPC URL and
/// credentials. Neither the SDK nor the test framework reads it.
#[cfg(feature = "provider")]
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    /// URL of the target Esplora REST service.
    pub esplora_url: String,
    /// URL of the `elementsd` JSON-RPC interface.
    pub elements_url: Option<String>,
    /// Authentication settings (e.g. cookie or username/password) for the RPC backend.
    pub auth: Option<Auth>,
}

/// Access to a Liquid or Elements network: broadcasting, confirmation, chain queries and UTXO
/// lookups.
///
/// All methods block. `EsploraProvider` and `SimplexProvider` implement it with the `provider`
/// feature. The trait itself doesn't need the feature, so a host without it can implement it.
///
/// UTXOs come back as the chain has them: a confidential output has `secrets: None`. A
/// [`Signer`](crate::signer::Signer) unblinds its own outputs.
pub trait ProviderTrait: Send + Sync {
    /// The network this provider talks to.
    fn get_network(&self) -> &SimplicityNetwork;

    /// Broadcasts `tx` and returns a receipt for it.
    ///
    /// # Errors
    /// Returns `ProviderError::BroadcastRejected` if the provider answers with an error status,
    /// as it does when the node refuses the transaction, and `ProviderError::Request` if the
    /// request fails.
    fn broadcast_transaction(&self, tx: &Transaction) -> Result<TxReceipt<'_>, ProviderError>;

    /// Blocks until transaction `txid` has one confirmation.
    ///
    /// # Errors
    /// Returns `ProviderError::Confirmation` if the transaction doesn't confirm before the
    /// provider stops waiting, and another `ProviderError` if a request fails.
    fn wait(&self, txid: &Txid) -> Result<(), ProviderError>;

    /// Retrieves the current block height of the network tip.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the backend request fails or the returned height cannot be parsed.
    fn fetch_tip_height(&self) -> Result<u32, ProviderError>;

    /// The hash of the tip block, as a hex string.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the backend request fails or the returned block hash cannot be parsed.
    fn fetch_tip_block_hash(&self) -> Result<String, ProviderError>;

    /// The timestamp of the tip block, in seconds since the Unix epoch.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the hash query, subsequent block query, or block parsing fails.
    fn fetch_tip_timestamp(&self) -> Result<u64, ProviderError>;

    /// The hash of the block at `block_height`, as a hex string.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the passed block height does not exist or the returned hash cannot be parsed.
    fn fetch_block_hash_at_height(&self, block_height: u32) -> Result<String, ProviderError>;

    /// The IDs of the transactions in the block with hex hash `block_hash`.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the passed block hash does not exist or the returned data cannot be parsed.
    fn fetch_block_txids(&self, block_hash: &str) -> Result<Vec<Txid>, ProviderError>;

    /// The transaction with ID `txid`.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the node fails to locate the transaction or serialization fails.
    fn fetch_transaction(&self, txid: &Txid) -> Result<Transaction, ProviderError>;

    /// The unspent outputs paying to `address`'s script, confidential ones not unblinded.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the backend request fails or if UTXO parsing fails.
    fn fetch_address_utxos(&self, address: &Address) -> Result<Vec<UTXO>, ProviderError>;

    /// The unspent outputs locked by `script`, confidential ones not unblinded.
    ///
    /// This finds the outputs a program controls, given its
    /// [`get_script_pubkey`](crate::program::Program::get_script_pubkey), which nobody holds a
    /// key for.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the backend request fails or if UTXO parsing fails.
    fn fetch_scripthash_utxos(&self, script: &Script) -> Result<Vec<UTXO>, ProviderError>;

    /// The provider's fee estimates: keys are confirmation targets in blocks, as strings, and
    /// values are fee rates in sat/vB.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the REST request fails or the resulting mappings fail to parse cleanly.
    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, ProviderError>;

    /// The fee rate, in sats/kvb, for confirmation within `target_blocks`.
    ///
    /// Uses the estimate for `target_blocks` if there is one. Otherwise it looks only at the
    /// targets 1 to 25, 144, 504 and 1008: the smallest of them that is at least `target_blocks`
    /// and has an estimate, else the smallest that has one. Without any, it returns
    /// [`DEFAULT_FEE_RATE`].
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
