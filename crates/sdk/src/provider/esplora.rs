use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::str::FromStr;
use std::time::Duration;

use simplicityhl::elements::encode;
use simplicityhl::elements::{Address, AssetId, OutPoint, Script, Transaction, Txid};

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::provider::SimplicityNetwork;
use crate::transaction::{TxReceipt, UTXO};
use crate::utils::hash_script;

use super::core::{DEFAULT_CONFIRMATION_ATTEMPTS, DEFAULT_ESPLORA_TIMEOUT_SECS, ProviderTrait};
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
    /// How many times `wait` polls for a confirmation before giving up.
    pub confirmation_attempts: u32,
}

#[derive(Deserialize)]
struct TxStatus {
    confirmed: bool,
}

#[derive(Deserialize)]
struct EsploraBlock {
    timestamp: u64,
}

#[derive(Deserialize)]
struct EsploraUtxo {
    txid: String,
    vout: u32,
    value: Option<u64>,
    asset: Option<String>,
}

impl EsploraProvider {
    /// Creates a new `EsploraProvider` connected to the provided endpoint targeting the specific network.
    #[must_use]
    pub fn new(url: String, network: SimplicityNetwork) -> Self {
        Self {
            esplora_url: url,
            network,
            timeout: Duration::from_secs(DEFAULT_ESPLORA_TIMEOUT_SECS),
            confirmation_attempts: DEFAULT_CONFIRMATION_ATTEMPTS,
        }
    }

    /// Sends a GET to `path` under the Esplora URL, whatever the status it answers with.
    fn request(&self, path: &str) -> Result<minreq::Response, ProviderError> {
        minreq::get(format!("{}{path}", self.esplora_url))
            .with_timeout(self.timeout.as_secs())
            .send()
            .map_err(|e| ProviderError::Request(e.to_string()))
    }

    /// Sends a GET to `path`, failing unless it answers 200.
    fn get(&self, path: &str) -> Result<minreq::Response, ProviderError> {
        let response = self.request(path)?;

        if response.status_code != 200 {
            return Err(ProviderError::Request(format!(
                "HTTP {}: {}",
                response.status_code, response.reason_phrase
            )));
        }

        Ok(response)
    }

    fn get_text(&self, path: &str) -> Result<String, ProviderError> {
        let response = self.get(path)?;
        let text = response
            .as_str()
            .map_err(|e| ProviderError::Deserialize(e.to_string()))?;

        Ok(text.trim().to_owned())
    }

    fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, ProviderError> {
        self.get(path)?
            .json()
            .map_err(|e| ProviderError::Deserialize(e.to_string()))
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
                    .ok_or(ProviderError::BadResponse)?;

                if txout.script_pubkey != *script {
                    return Err(ProviderError::UnexpectedScript(outpoint));
                }

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
                url,
                message: body,
            });
        }

        let returned = Txid::from_str(&body).map_err(|e| ProviderError::InvalidTxid(e.to_string()))?;

        if returned != tx.txid() {
            return Err(ProviderError::UnexpectedTxid {
                expected: tx.txid(),
                returned,
            });
        }

        Ok(TxReceipt::new(self, returned))
    }

    fn wait(&self, txid: &Txid) -> Result<(), ProviderError> {
        let path = format!("/tx/{txid}/status");

        let confirmation_poll = match self.network {
            SimplicityNetwork::ElementsRegtest { .. } => Duration::from_millis(100),
            _ => Duration::from_secs(10),
        };

        for _ in 0..self.confirmation_attempts {
            let response = self.request(&path)?;

            // an unknown transaction answers 404 until Esplora indexes it
            if response.status_code == 200 {
                let status: TxStatus = response.json().map_err(|e| ProviderError::Deserialize(e.to_string()))?;

                if status.confirmed {
                    return Ok(());
                }
            }

            std::thread::sleep(confirmation_poll);
        }

        Err(ProviderError::Confirmation)
    }

    fn fetch_tip_height(&self) -> Result<u32, ProviderError> {
        self.get_text("/blocks/tip/height")?
            .parse()
            .map_err(|e: std::num::ParseIntError| ProviderError::Deserialize(e.to_string()))
    }

    fn fetch_tip_block_hash(&self) -> Result<String, ProviderError> {
        self.get_text("/blocks/tip/hash")
    }

    fn fetch_tip_timestamp(&self) -> Result<u64, ProviderError> {
        let tip_hash = self.fetch_tip_block_hash()?;
        let block: EsploraBlock = self.get_json(&format!("/block/{tip_hash}"))?;

        Ok(block.timestamp)
    }

    fn fetch_block_hash_at_height(&self, block_height: u32) -> Result<String, ProviderError> {
        self.get_text(&format!("/block-height/{block_height}"))
    }

    fn fetch_block_txids(&self, block_hash: &str) -> Result<Vec<Txid>, ProviderError> {
        self.get_json(&format!("/block/{block_hash}/txids"))
    }

    fn fetch_transaction(&self, txid: &Txid) -> Result<Transaction, ProviderError> {
        let response = self.get(&format!("/tx/{txid}/raw"))?;
        let transaction: Transaction =
            encode::deserialize(response.as_bytes()).map_err(|e| ProviderError::Deserialize(e.to_string()))?;

        if transaction.txid() != *txid {
            return Err(ProviderError::UnexpectedTxid {
                expected: *txid,
                returned: transaction.txid(),
            });
        }

        Ok(transaction)
    }

    fn fetch_address_utxos(&self, address: &Address) -> Result<Vec<UTXO>, ProviderError> {
        let utxos: Vec<EsploraUtxo> = self.get_json(&format!("/address/{address}/utxo"))?;

        self.to_utxos(&utxos, &address.script_pubkey())
    }

    fn fetch_scripthash_utxos(&self, script: &Script) -> Result<Vec<UTXO>, ProviderError> {
        let scripthash = hex::encode(hash_script(script));
        let utxos: Vec<EsploraUtxo> = self.get_json(&format!("/scripthash/{scripthash}/utxo"))?;

        self.to_utxos(&utxos, script)
    }

    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, ProviderError> {
        self.get_json("/fee-estimates")
    }
}
