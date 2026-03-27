use std::collections::HashMap;
use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;

use simplicityhl::elements::hashes::{Hash, sha256};

use simplicityhl::elements::encode;
use simplicityhl::elements::{Address, OutPoint, Script, Transaction, TxOut, Txid};

use serde::Deserialize;

use crate::provider::SimplicityNetwork;
use crate::transaction::UTXO;

use super::core::{DEFAULT_ESPLORA_TIMEOUT_SECS, ProviderTrait};
use super::error::ProviderError;

pub struct EsploraProvider {
    pub esplora_url: String,
    pub network: SimplicityNetwork,
    pub timeout: Duration,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct TxStatus {
    confirmed: bool,
    block_height: Option<u32>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct EsploraBlock {
    id: String,
    height: u32,
    timestamp: u64,
    tx_count: u32,
}

#[derive(Clone, Deserialize)]
#[allow(dead_code)]
struct UtxoStatus {
    pub confirmed: bool,
    pub block_height: Option<u64>,
    pub block_hash: Option<String>,
    pub block_time: Option<u64>,
}

#[derive(Clone, Deserialize)]
#[allow(dead_code)]
struct EsploraUtxo {
    pub txid: String,
    pub vout: u32,
    pub value: Option<u64>,
    pub valuecommitment: Option<String>,
    pub asset: Option<String>,
    pub assetcommitment: Option<String>,
    pub status: UtxoStatus,
}

impl EsploraProvider {
    pub fn new(url: String, network: SimplicityNetwork) -> Self {
        Self {
            esplora_url: url,
            network,
            timeout: Duration::from_secs(DEFAULT_ESPLORA_TIMEOUT_SECS),
        }
    }

    fn esplora_utxo_to_outpoint(&self, utxo: &EsploraUtxo) -> Result<OutPoint, ProviderError> {
        let txid = Txid::from_str(&utxo.txid).map_err(|e| ProviderError::InvalidTxid(e.to_string()))?;

        Ok(OutPoint::new(txid, utxo.vout))
    }

    fn populate_txouts_from_outpoints(&self, outpoints: &[OutPoint]) -> Result<Vec<UTXO>, ProviderError> {
        let set: HashSet<_> = outpoints.iter().collect();
        let mut map = HashMap::new();

        // filter unique transactions
        for point in set {
            let tx = self.fetch_transaction(&point.txid)?;
            map.insert(point.txid, tx);
        }

        // populate TxOuts
        Ok(outpoints
            .iter()
            .map(|point| UTXO {
                outpoint: *point,
                txout: map.get(&point.txid).unwrap().output[point.vout as usize].clone(),
            })
            .collect())
    }
}

impl ProviderTrait for EsploraProvider {
    fn get_network(&self) -> &SimplicityNetwork {
        &self.network
    }

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

        Txid::from_str(&body).map_err(|e| ProviderError::InvalidTxid(e.to_string()))
    }

    fn wait(&self, txid: &Txid) -> Result<(), ProviderError> {
        let url = format!("{}/tx/{}/status", self.esplora_url, txid);
        let timeout_secs = self.timeout.as_secs();

        let confirmation_poll = match self.network {
            SimplicityNetwork::ElementsRegtest { .. } => Duration::from_millis(100),
            _ => Duration::from_secs(10),
        };

        // polling needs to be > 1 min on mainnet/testnet
        for _ in 1..10 {
            let response = minreq::get(&url)
                .with_timeout(timeout_secs)
                .send()
                .map_err(|e| ProviderError::Request(e.to_string()))?;

            if response.status_code != 200 {
                std::thread::sleep(confirmation_poll);
                continue;
            }

            let status: TxStatus = response.json().map_err(|e| ProviderError::Deserialize(e.to_string()))?;

            if status.confirmed {
                return Ok(());
            }

            std::thread::sleep(confirmation_poll);
        }

        Err(ProviderError::Confirmation())
    }

    fn fetch_tip_height(&self) -> Result<u32, ProviderError> {
        let url = format!("{}/blocks/tip/height", self.esplora_url);
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

        let body_str = response
            .as_str()
            .map_err(|e| ProviderError::Deserialize(e.to_string()))?;

        let height: u32 = body_str
            .trim()
            .parse()
            .map_err(|e: std::num::ParseIntError| ProviderError::Deserialize(e.to_string()))?;

        Ok(height)
    }

    fn fetch_tip_timestamp(&self) -> Result<u64, ProviderError> {
        let timeout_secs = self.timeout.as_secs();

        let hash_url = format!("{}/blocks/tip/hash", self.esplora_url);
        let hash_response = minreq::get(&hash_url)
            .with_timeout(timeout_secs)
            .send()
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        if hash_response.status_code != 200 {
            return Err(ProviderError::Request(format!(
                "HTTP {}: {}",
                hash_response.status_code, hash_response.reason_phrase
            )));
        }

        let tip_hash = hash_response
            .as_str()
            .map_err(|e| ProviderError::Deserialize(e.to_string()))?
            .trim();

        let block_url = format!("{}/block/{}", self.esplora_url, tip_hash);
        let block_response = minreq::get(&block_url)
            .with_timeout(timeout_secs)
            .send()
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        if block_response.status_code != 200 {
            return Err(ProviderError::Request(format!(
                "HTTP {}: {}",
                block_response.status_code, block_response.reason_phrase
            )));
        }

        let block: EsploraBlock = block_response
            .json()
            .map_err(|e| ProviderError::Deserialize(e.to_string()))?;

        Ok(block.timestamp)
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

    fn fetch_address_utxos(&self, address: &Address) -> Result<Vec<UTXO>, ProviderError> {
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
            .map(|utxo| self.esplora_utxo_to_outpoint(utxo))
            .collect::<Result<Vec<OutPoint>, ProviderError>>()?;

        self.populate_txouts_from_outpoints(&outpoints)
    }

    fn fetch_scripthash_utxos(&self, script: &Script) -> Result<Vec<UTXO>, ProviderError> {
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
            .map(|utxo| self.esplora_utxo_to_outpoint(utxo))
            .collect::<Result<Vec<OutPoint>, ProviderError>>()?;

        self.populate_txouts_from_outpoints(&outpoints)
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
