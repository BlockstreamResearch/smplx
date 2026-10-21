use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::str::FromStr;
use std::time::Duration;

use simplicityhl::elements::hashes::sha256;

use simplicityhl::elements::encode;
use simplicityhl::elements::{Address, AssetId, OutPoint, Script, Transaction, Txid};

use serde::Deserialize;

use crate::provider::SimplicityNetwork;
use crate::transaction::{TxReceipt, UTXO};

use super::core::{DEFAULT_ESPLORA_TIMEOUT_SECS, ProviderTrait};
use super::error::ProviderError;

/// A provider implementation that interacts with the Esplora REST API backend.
#[derive(Debug)]
pub struct EsploraProvider {
    /// The base URL of the Esplora REST API.
    pub esplora_url: String,
    /// The currently configured Simplicity network (e.g. Liquid, Testnet, Regtest).
    pub network: SimplicityNetwork,
    /// Timeout duration used in underlying HTTP requests.
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
    /// Creates a new `EsploraProvider` connected to the provided endpoint targeting the specific network.
    #[must_use]
    pub fn new(url: String, network: SimplicityNetwork) -> Self {
        Self {
            esplora_url: url,
            network,
            timeout: Duration::from_secs(DEFAULT_ESPLORA_TIMEOUT_SECS),
        }
    }

    /// The UTXOs of `utxos`, all locked by `script`. Explicit ones are built from the response alone.
    /// Confidential ones need their transaction for the nonce and range proof.
    fn to_utxos(&self, utxos: &[EsploraUtxo], script: &Script) -> Result<Vec<UTXO>, ProviderError> {
        let mut transactions: HashMap<Txid, Transaction> = HashMap::new();

        utxos
            .iter()
            .map(|utxo| {
                let txid = Txid::from_str(&utxo.txid).map_err(|e| ProviderError::InvalidTxid(e.to_string()))?;
                let outpoint = OutPoint::new(txid, utxo.vout);

                if let (Some(value), Some(asset)) = (utxo.value, &utxo.asset) {
                    let asset = AssetId::from_str(asset).map_err(|e| ProviderError::Deserialize(e.to_string()))?;

                    return Ok(UTXO::explicit(outpoint, script.clone(), value, asset));
                }

                if let Entry::Vacant(entry) = transactions.entry(txid) {
                    entry.insert(self.fetch_transaction(&txid)?);
                }

                let txout = transactions[&txid]
                    .output
                    .get(utxo.vout as usize)
                    .ok_or_else(ProviderError::BadResponse)?;

                Ok(UTXO {
                    outpoint,
                    txout: txout.clone(),
                    secrets: None,
                })
            })
            .collect()
    }
}

impl ProviderTrait for EsploraProvider {
    fn get_network(&self) -> &SimplicityNetwork {
        &self.network
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn broadcast_transaction(&self, tx: &Transaction) -> Result<TxReceipt<'_>, ProviderError> {
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

        Txid::from_str(&body)
            .map_err(|e| ProviderError::InvalidTxid(e.to_string()))
            .map(|tx_id| TxReceipt::new(self, tx_id))
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

    fn fetch_tip_block_hash(&self) -> Result<String, ProviderError> {
        let url = format!("{}/blocks/tip/hash", self.esplora_url);
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

        let tip_block_hash = response
            .as_str()
            .map_err(|e| ProviderError::Deserialize(e.to_string()))?
            .trim();

        Ok(tip_block_hash.into())
    }

    fn fetch_tip_timestamp(&self) -> Result<u64, ProviderError> {
        let tip_hash = self.fetch_tip_block_hash()?;

        let url = format!("{}/block/{}", self.esplora_url, tip_hash);
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

        let block: EsploraBlock = response.json().map_err(|e| ProviderError::Deserialize(e.to_string()))?;

        Ok(block.timestamp)
    }

    fn fetch_block_hash_at_height(&self, block_height: u32) -> Result<String, ProviderError> {
        let url = format!("{}/block-height/{}", self.esplora_url, block_height);
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

        let block_hash = response
            .as_str()
            .map_err(|e| ProviderError::Deserialize(e.to_string()))?
            .trim();

        Ok(block_hash.into())
    }

    fn fetch_block_txids(&self, block_hash: &str) -> Result<Vec<Txid>, ProviderError> {
        let url = format!("{}/block/{}/txids", self.esplora_url, block_hash);
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

        let block_txids: Vec<Txid> = response.json().map_err(|e| ProviderError::Deserialize(e.to_string()))?;

        Ok(block_txids)
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

        self.to_utxos(&utxos, &address.script_pubkey())
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

        self.to_utxos(&utxos, script)
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
