use std::collections::HashMap;

use simplicityhl::elements::{Address, OutPoint, Script, Transaction, TxOut, Txid};

use crate::constants::DEFAULT_FEE_RATE;
use crate::error::SimplexError;

pub trait ProviderTrait {
    fn broadcast_transaction(&self, tx: &Transaction) -> Result<String, SimplexError>;

    fn fetch_transaction(&self, txid: Txid) -> Result<Transaction, SimplexError>;

    fn fetch_address_utxos(&self, address: &Address) -> Result<Vec<(OutPoint, TxOut)>, SimplexError>;

    fn fetch_scripthash_utxos(&self, script: &Script) -> Result<Vec<(OutPoint, TxOut)>, SimplexError>;

    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, SimplexError>;

    fn get_fee_rate(&self, target_blocks: u32) -> Result<f32, SimplexError> {
        if target_blocks == 0 {
            return Ok(DEFAULT_FEE_RATE);
        }

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

        Err(SimplexError::Request("No fee estimates available".to_string()))
    }
}
