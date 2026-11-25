use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::str::FromStr;
use std::time::Duration;

use simplicityhl::elements::encode;
use simplicityhl::elements::{Address, AssetId, Block, BlockHash, OutPoint, Script, Transaction, Txid};

use serde::Deserialize;
use serde::de::DeserializeOwned;

use ureq::http::{Response, StatusCode};
use ureq::{Agent, Body};

use crate::provider::SimplicityNetwork;
use crate::transaction::{TxReceipt, UTXO};
use crate::utils::hash_script;

use super::core::{DEFAULT_CONFIRMATION_ATTEMPTS, DEFAULT_ESPLORA_TIMEOUT_SECS, ProviderTrait};
use super::error::ProviderError;

/// A provider that talks to an Esplora REST API, such as
/// `https://blockstream.info/liquidtestnet/api`.
///
/// Each request times out after `timeout`. The provider doesn't retry a failed request.
///
/// Requests go over pooled keep-alive connections, so consecutive requests to the same Esplora
/// skip the TCP and TLS handshakes.
#[derive(Debug)]
pub struct EsploraProvider {
    /// The base URL of the Esplora REST API, without a trailing slash.
    pub esplora_url: String,
    /// The network the API serves.
    pub network: SimplicityNetwork,
    /// The timeout of each request. Default: [`DEFAULT_ESPLORA_TIMEOUT_SECS`].
    pub timeout: Duration,
    /// How many times `wait` polls for a confirmation before giving up. Default:
    /// [`DEFAULT_CONFIRMATION_ATTEMPTS`].
    ///
    /// `wait` polls every 100 ms on [`SimplicityNetwork::ElementsRegtest`] and every 10 seconds on
    /// other networks, so by default it gives up after about 1 second or about 90 seconds.
    pub confirmation_attempts: u32,
    /// The HTTP client, which keeps the connection pool.
    agent: Agent,
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
    /// Creates a provider for the Esplora API at `url`, which serves `network`, with the default
    /// timeout and confirmation attempts.
    #[must_use]
    pub fn new(url: String, network: SimplicityNetwork) -> Self {
        Self {
            esplora_url: url,
            network,
            timeout: Duration::from_secs(DEFAULT_ESPLORA_TIMEOUT_SECS),
            confirmation_attempts: DEFAULT_CONFIRMATION_ATTEMPTS,
            agent: Agent::config_builder()
                // Esplora's error statuses carry information: 404 while a transaction isn't
                // indexed yet, and the node's reason with a rejected broadcast.
                .http_status_as_error(false)
                // ureq would otherwise route requests through `HTTP_PROXY` and the like, even to a
                // local regtest Esplora unless `NO_PROXY` lists it.
                .proxy(None)
                .build()
                .new_agent(),
        }
    }

    /// Sends a GET to `path` under the Esplora URL, whatever the response status.
    fn request(&self, path: &str) -> Result<Response<Body>, ProviderError> {
        self.agent
            .get(format!("{}{path}", self.esplora_url))
            .config()
            .timeout_global(Some(self.timeout))
            .build()
            .call()
            .map_err(|e| ProviderError::Request(e.to_string()))
    }

    /// Sends a GET to `path`, failing unless the response status is 200.
    fn get(&self, path: &str) -> Result<Response<Body>, ProviderError> {
        let response = self.request(path)?;
        let status = response.status();

        if status != StatusCode::OK {
            return Err(ProviderError::Request(format!(
                "HTTP {}: {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or_default()
            )));
        }

        Ok(response)
    }

    fn get_text(&self, path: &str) -> Result<String, ProviderError> {
        let text = self
            .get(path)?
            .body_mut()
            .read_to_string()
            .map_err(|e| ProviderError::Deserialize(e.to_string()))?;

        Ok(text.trim().to_owned())
    }

    fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, ProviderError> {
        self.get(path)?
            .body_mut()
            .read_json()
            .map_err(|e| ProviderError::Deserialize(e.to_string()))
    }

    /// The body of a 200 response to a GET of `path`, up to 10 MiB, which holds any Liquid block.
    fn get_bytes(&self, path: &str) -> Result<Vec<u8>, ProviderError> {
        self.get(path)?
            .body_mut()
            .read_to_vec()
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

    fn broadcast_transaction(&self, tx: &Transaction) -> Result<TxReceipt<'_>, ProviderError> {
        let tx_hex = encode::serialize_hex(tx);
        let url = format!("{}/tx", self.esplora_url);

        let mut response = self
            .agent
            .post(&url)
            .config()
            .timeout_global(Some(self.timeout))
            .build()
            .send(tx_hex)
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        let status = response.status();
        let body = response
            .body_mut()
            .read_to_string()
            .unwrap_or_default()
            .trim()
            .to_owned();

        if !status.is_success() {
            return Err(ProviderError::BroadcastRejected {
                status: status.as_u16(),
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
            let mut response = self.request(&path)?;

            // Esplora answers 404 for a transaction it hasn't indexed yet
            if response.status() == StatusCode::OK {
                let status: TxStatus = response
                    .body_mut()
                    .read_json()
                    .map_err(|e| ProviderError::Deserialize(e.to_string()))?;

                if status.confirmed {
                    return Ok(());
                }
            } else {
                // Read the error body to the end, so that the connection goes back to the pool.
                let _ = response.body_mut().read_to_vec();
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

    fn fetch_block(&self, block_hash: &str) -> Result<Block, ProviderError> {
        let expected = BlockHash::from_str(block_hash).map_err(|e| ProviderError::InvalidBlockHash(e.to_string()))?;

        let raw = self.get_bytes(&format!("/block/{expected}/raw"))?;
        let block: Block = encode::deserialize(&raw).map_err(|e| ProviderError::Deserialize(e.to_string()))?;

        if block.block_hash() != expected {
            return Err(ProviderError::UnexpectedBlockHash {
                expected,
                returned: block.block_hash(),
            });
        }

        Ok(block)
    }

    fn fetch_transaction(&self, txid: &Txid) -> Result<Transaction, ProviderError> {
        let raw = self.get_bytes(&format!("/tx/{txid}/raw"))?;
        let transaction: Transaction =
            encode::deserialize(&raw).map_err(|e| ProviderError::Deserialize(e.to_string()))?;

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
