mod esplora;

use simplicityhl::elements::encode;
use simplicityhl::elements::hex::ToHex;
use simplicityhl::elements::{Transaction, Txid};
use std::collections::HashMap;
use std::str::FromStr;

use crate::constants::DEFAULT_FEE_RATE;
use crate::error::SimplexError;

pub use simplex_provider::esplora::*;

pub trait ProviderSync {
    fn broadcast_transaction(&self, tx: &Transaction) -> Result<Txid, SimplexError>;

    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, SimplexError>;

    fn fetch_transaction(&self, txid: &Txid) -> Result<Transaction, SimplexError>;

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

#[async_trait::async_trait]
pub trait ProviderAsync {
    async fn broadcast_transaction(&self, tx: &Transaction) -> Result<Txid, SimplexError>;

    async fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, SimplexError>;

    async fn fetch_transaction(&self, txid: &Txid) -> Result<Transaction, SimplexError>;

    async fn get_fee_rate(&self, target_blocks: u32) -> Result<f32, SimplexError> {
        if target_blocks == 0 {
            return Ok(DEFAULT_FEE_RATE);
        }

        let estimates = self.fetch_fee_estimates().await?;

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

pub struct EsploraProvider {
    esplora_url: String,
}

impl EsploraProvider {
    pub fn new(url: String) -> Self {
        Self { esplora_url: url }
    }
}

impl ProviderSync for EsploraProvider {
    fn broadcast_transaction(&self, tx: &Transaction) -> Result<Txid, SimplexError> {
        let tx_hex = encode::serialize_hex(tx);
        let url = format!("{}/tx", self.esplora_url);

        let response = minreq::post(&url)
            .with_body(tx_hex)
            .send()
            .map_err(|e| SimplexError::Request(e.to_string()))?;

        let status = response.status_code;
        let body = response.as_str().unwrap_or("").trim().to_owned();

        if !(200..300).contains(&status) {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            return Err(SimplexError::BroadcastRejected {
                status: status as u16,
                url: format!("{}/tx", self.esplora_url),
                message: body,
            });
        }
        Ok(Txid::from_str(&body)?)
    }

    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, SimplexError> {
        let url = format!("{}/fee-estimates", self.esplora_url.clone());
        let response = minreq::get(&url)
            .send()
            .map_err(|e| SimplexError::Request(e.to_string()))?;

        if response.status_code != 200 {
            return Err(SimplexError::Request(format!(
                "HTTP {}: {}",
                response.status_code, response.reason_phrase
            )));
        }

        let estimates: HashMap<String, f64> = response.json().map_err(|e| SimplexError::Deserialize(e.to_string()))?;

        Ok(estimates)
    }

    fn fetch_transaction(&self, txid: &Txid) -> Result<Transaction, SimplexError> {
        let url = format!("{}/tx/{}/raw", self.esplora_url.clone(), txid.to_hex().as_str());
        let response = minreq::get(&url)
            .send()
            .map_err(|e| SimplexError::Request(e.to_string()))?;

        if response.status_code != 200 {
            return Err(SimplexError::Request(format!(
                "HTTP {}: {}",
                response.status_code, response.reason_phrase
            )));
        }

        let bytes = response.as_bytes();
        let tx: Transaction = encode::deserialize(bytes).map_err(|e| SimplexError::Deserialize(e.to_string()))?;

        Ok(tx)
    }
}
