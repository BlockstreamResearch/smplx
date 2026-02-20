mod esplora;

use std::collections::HashMap;

use simplicityhl::elements::encode;
use simplicityhl::elements::hex::ToHex;
use simplicityhl::elements::{Transaction, Txid};

use serde::Deserialize;

use crate::constants::DEFAULT_FEE_RATE;
use crate::error::SimplexError;

pub trait Provider {
    fn broadcast_transaction(&self, tx: &Transaction) -> Result<String, SimplexError>;

    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, SimplexError>;

    fn fetch_transaction(&self, txid: Txid) -> Result<Transaction, SimplexError>;

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

pub struct EsploraProvider {
    esplora_url: String,
}

#[derive(Clone, Deserialize)]
struct UtxoStatus {
    pub confirmed: bool,
    #[serde(default)]
    pub block_height: Option<u64>,
    #[serde(default)]
    pub block_hash: Option<String>,
}

#[derive(Clone, Deserialize)]
struct EsploraUtxo {
    pub txid: String,
    pub vout: u32,
    #[serde(default)]
    pub value: Option<u64>,
    #[serde(default)]
    pub valuecommitment: Option<String>,
    #[serde(default)]
    pub asset: Option<String>,
    #[serde(default)]
    pub assetcommitment: Option<String>,
    pub status: UtxoStatus,
}

impl EsploraProvider {
    pub fn new(url: String) -> Self {
        Self { esplora_url: url }
    }

    // pub fn fetch_address_utxos(address: &Address) -> Result<Vec<EsploraUtxo>, SimplexError> {
    //     let url = format!("{ESPLORA_URL}/address/{address}/utxo");
    //     let response = minreq::get(&url)
    //         .send()
    //         .map_err(|e| SimplexError::Request(e.to_string()))?;

    //     if response.status_code != 200 {
    //         return Err(SimplexError::Request(format!(
    //             "HTTP {}: {}",
    //             response.status_code, response.reason_phrase
    //         )));
    //     }

    //     let utxos: Vec<EsploraUtxo> = response.json().map_err(|e| SimplexError::Deserialize(e.to_string()))?;

    //     Ok(utxos)
    // }

    // pub fn fetch_scripthash_utxos(script: &Script) -> Result<Vec<EsploraUtxo>, SimplexError> {
    //     let hash = sha256::Hash::hash(script.as_bytes());
    //     let hash_bytes = hash.to_byte_array();
    //     let scripthash = hex::encode(hash_bytes);

    //     let url = format!("{ESPLORA_URL}/scripthash/{scripthash}/utxo");
    //     let response = minreq::get(&url)
    //         .send()
    //         .map_err(|e| SimplexError::Request(e.to_string()))?;

    //     if response.status_code != 200 {
    //         return Err(SimplexError::Request(format!(
    //             "HTTP {}: {}",
    //             response.status_code, response.reason_phrase
    //         )));
    //     }

    //     let utxos: Vec<EsploraUtxo> = response.json().map_err(|e| SimplexError::Deserialize(e.to_string()))?;

    //     Ok(utxos)
    // }
}

impl Provider for EsploraProvider {
    fn broadcast_transaction(&self, tx: &Transaction) -> Result<String, SimplexError> {
        let tx_hex = encode::serialize_hex(tx);
        let url = format!("{}/tx", self.esplora_url);

        let response = minreq::post(&url)
            .with_body(tx_hex)
            .send()
            .map_err(|e| SimplexError::Request(e.to_string()))?;

        let status = response.status_code;
        let body = response.as_str().unwrap_or("").trim().to_owned();

        if !(200..300).contains(&status) {
            return Err(SimplexError::BroadcastRejected {
                status: status as u16,
                url: format!("{}/tx", self.esplora_url),
                message: body,
            });
        }

        Ok(body)
    }

    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, SimplexError> {
        let url = self.esplora_url.clone() + "/fee-estimates";
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

    fn fetch_transaction(&self, txid: Txid) -> Result<Transaction, SimplexError> {
        let url = self.esplora_url.clone() + "/tx/" + txid.to_hex().as_str() + "/raw";
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
