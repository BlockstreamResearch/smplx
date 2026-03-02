use std::collections::HashMap;
use std::collections::HashSet;
use std::str::FromStr;
use std::thread::sleep;
use std::time::Duration;

use simplicityhl::elements::hashes::{Hash, sha256};

use simplicityhl::elements::encode;
use simplicityhl::elements::{Address, OutPoint, Script, Transaction, TxOut, Txid};

use serde::Deserialize;

pub use simplex_provider::esplora::*;

use super::error::ProviderError;
use super::provider::{DEFAULT_TIMEOUT_SECS, ProviderTrait};

#[derive(Clone)]
pub struct EsploraProvider {
    esplora_url: String,
    timeout: Duration,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct TxStatus {
    confirmed: bool,
    #[serde(default)]
    block_height: Option<u32>,
}

#[derive(Clone, Deserialize)]
#[allow(dead_code)]
struct UtxoStatus {
    pub confirmed: bool,
    #[serde(default)]
    pub block_height: Option<u64>,
    #[serde(default)]
    pub block_hash: Option<String>,
    #[serde(default)]
    pub block_time: Option<u64>,
}

#[derive(Clone, Deserialize)]
#[allow(dead_code)]
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
        Self {
            esplora_url: url,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        }
    }

    fn esplora_utxo_to_outpoint(&self, utxo: &EsploraUtxo) -> Result<OutPoint, ProviderError> {
        let txid = Txid::from_str(&utxo.txid).map_err(|e| ProviderError::InvalidTxid(e.to_string()))?;

        Ok(OutPoint::new(txid, utxo.vout))
    }

    fn populate_txouts_from_outpoints(
        &self,
        outpoints: &Vec<OutPoint>,
    ) -> Result<Vec<(OutPoint, TxOut)>, ProviderError> {
        let set: HashSet<_> = outpoints.into_iter().collect();
        let mut map = HashMap::new();

        // filter unique transactions
        for point in set {
            let tx = self.fetch_transaction(&point.txid)?;
            map.insert(point.txid, tx);
        }

        // populate TxOuts
        Ok(outpoints
            .iter()
            .map(|point| {
                (
                    *point,
                    map.get(&point.txid).unwrap().output[point.vout as usize].clone(),
                )
            })
            .collect())
    }
}

impl ProviderTrait for EsploraProvider {
    fn broadcast_transaction(&self, tx: &Transaction) -> Result<Txid, ProviderError> {
        let tx_hex = encode::serialize_hex(tx);
        let url = format!("{}/tx", self.esplora_url);
        let timeout_secs = self.timeout.as_secs();

        let response = minreq::post(&url)
            .with_timeout(timeout_secs)
            .with_body(tx_hex)
            .send()
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        let status = response.status_code;
        let body = response.as_str().unwrap_or("").trim().to_owned();

        if !(200..300).contains(&status) {
            return Err(ProviderError::BroadcastRejected {
                status: status as u16,
                url: format!("{}/tx", self.esplora_url),
                message: body,
            });
        }

        Ok(Txid::from_str(&body).map_err(|e| ProviderError::InvalidTxid(e.to_string()))?)
    }

    fn wait(&self, txid: &Txid) -> Result<(), ProviderError> {
        let url = format!("{}/tx/{}/status", self.esplora_url, txid);
        let timeout_secs = self.timeout.as_secs();

        for _ in 1..10 {
            let response = minreq::get(&url)
                .with_timeout(timeout_secs)
                .send()
                .map_err(|e| ProviderError::Request(e.to_string()))?;

            if response.status_code != 200 {
                sleep(Duration::from_secs(5));
                continue;
            }

            let status: TxStatus = response.json().map_err(|e| ProviderError::Deserialize(e.to_string()))?;

            if status.confirmed {
                return Ok(());
            }

            sleep(Duration::from_secs(10));
        }

        Err(ProviderError::Confirmation())
    }

    fn fetch_transaction(&self, txid: &Txid) -> Result<Transaction, ProviderError> {
        let url = format!("{}/tx/{}/raw", self.esplora_url, txid);
        let timeout_secs = self.timeout.as_secs();

        let response = minreq::get(&url)
            .with_timeout(timeout_secs)
            .send()
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        if response.status_code != 200 {
            return Err(ProviderError::Request(format!(
                "HTTP {}: {}",
                response.status_code, response.reason_phrase
            )));
        }

        let bytes = response.as_bytes();
        let tx: Transaction = encode::deserialize(bytes).map_err(|e| ProviderError::Deserialize(e.to_string()))?;

        Ok(tx)
    }

    fn fetch_address_utxos(&self, address: &Address) -> Result<Vec<(OutPoint, TxOut)>, ProviderError> {
        let url = format!("{}/address/{}/utxo", self.esplora_url, address);
        let timeout_secs = self.timeout.as_secs();

        let response = minreq::get(&url)
            .with_timeout(timeout_secs)
            .send()
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        if response.status_code != 200 {
            return Err(ProviderError::Request(format!(
                "HTTP {}: {}",
                response.status_code, response.reason_phrase
            )));
        }

        let utxos: Vec<EsploraUtxo> = response.json().map_err(|e| ProviderError::Deserialize(e.to_string()))?;
        let outpoints = utxos
            .iter()
            .map(|utxo| Ok(self.esplora_utxo_to_outpoint(&utxo)?))
            .collect::<Result<Vec<OutPoint>, ProviderError>>()?;

        Ok(self.populate_txouts_from_outpoints(&outpoints)?)
    }

    fn fetch_scripthash_utxos(&self, script: &Script) -> Result<Vec<(OutPoint, TxOut)>, ProviderError> {
        let hash = sha256::Hash::hash(script.as_bytes());
        let hash_bytes = hash.to_byte_array();
        let scripthash = hex::encode(hash_bytes);

        let url = format!("{}/scripthash/{}/utxo", self.esplora_url, scripthash);
        let timeout_secs = self.timeout.as_secs();

        let response = minreq::get(&url)
            .with_timeout(timeout_secs)
            .send()
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        if response.status_code != 200 {
            return Err(ProviderError::Request(format!(
                "HTTP {}: {}",
                response.status_code, response.reason_phrase
            )));
        }

        let utxos: Vec<EsploraUtxo> = response.json().map_err(|e| ProviderError::Deserialize(e.to_string()))?;
        let outpoints = utxos
            .iter()
            .map(|utxo| Ok(self.esplora_utxo_to_outpoint(&utxo)?))
            .collect::<Result<Vec<OutPoint>, ProviderError>>()?;

        Ok(self.populate_txouts_from_outpoints(&outpoints)?)
    }

    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, ProviderError> {
        let url = format!("{}/fee-estimates", self.esplora_url);
        let timeout_secs = self.timeout.as_secs();

        let response = minreq::get(&url)
            .with_timeout(timeout_secs)
            .send()
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        if response.status_code != 200 {
            return Err(ProviderError::Request(format!(
                "HTTP {}: {}",
                response.status_code, response.reason_phrase
            )));
        }

        let estimates: HashMap<String, f64> = response.json().map_err(|e| ProviderError::Deserialize(e.to_string()))?;

        Ok(estimates)
    }
}
