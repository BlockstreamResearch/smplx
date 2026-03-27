use std::collections::HashMap;

use simplicityhl::elements::{Address, Script, Transaction, Txid};

use crate::provider::SimplicityNetwork;
use crate::transaction::UTXO;

use super::error::ProviderError;

pub const DEFAULT_FEE_RATE: f32 = 100.0;
pub const DEFAULT_ESPLORA_TIMEOUT_SECS: u64 = 10;

pub trait ProviderTrait {
    fn get_network(&self) -> &SimplicityNetwork;

    fn broadcast_transaction(&self, tx: &Transaction) -> Result<Txid, ProviderError>;

    fn wait(&self, txid: &Txid) -> Result<(), ProviderError>;

    fn fetch_tip_height(&self) -> Result<u32, ProviderError>;

    fn fetch_tip_timestamp(&self) -> Result<u64, ProviderError>;

    fn fetch_transaction(&self, txid: &Txid) -> Result<Transaction, ProviderError>;

    fn fetch_address_utxos(&self, address: &Address) -> Result<Vec<UTXO>, ProviderError>;

    fn fetch_scripthash_utxos(&self, script: &Script) -> Result<Vec<UTXO>, ProviderError>;

    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, ProviderError>;

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
