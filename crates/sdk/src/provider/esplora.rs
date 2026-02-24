use std::collections::HashMap;
use std::collections::HashSet;
use std::str::FromStr;

use simplicityhl::elements::hashes::{Hash, sha256};

use simplicityhl::elements::encode;
use simplicityhl::elements::hex::ToHex;
use simplicityhl::elements::{Address, OutPoint, Script, Transaction, TxOut, Txid};

use serde::Deserialize;

use super::provider::ProviderTrait;

use crate::error::SimplexError;

#[derive(Clone)]
pub struct EsploraProvider {
    esplora_url: String,
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
        Self { esplora_url: url }
    }

    fn esplora_utxo_to_outpoint(&self, utxo: &EsploraUtxo) -> Result<OutPoint, SimplexError> {
        let txid = Txid::from_str(&utxo.txid).map_err(|e| SimplexError::InvalidTxid(e.to_string()))?;

        Ok(OutPoint::new(txid, utxo.vout))
    }

    fn populate_txouts_from_outpoints(
        &self,
        outpoints: &Vec<OutPoint>,
    ) -> Result<Vec<(OutPoint, TxOut)>, SimplexError> {
        let set: HashSet<_> = outpoints.into_iter().collect();
        let mut map = HashMap::new();

        // filter unique transactions
        for point in set {
            let tx = self.fetch_transaction(point.txid)?;
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

    fn fetch_address_utxos(&self, address: &Address) -> Result<Vec<(OutPoint, TxOut)>, SimplexError> {
        let esplora = self.esplora_url.clone();
        let url = format!("{esplora}/address/{address}/utxo");
        let response = minreq::get(&url)
            .send()
            .map_err(|e| SimplexError::Request(e.to_string()))?;

        if response.status_code != 200 {
            return Err(SimplexError::Request(format!(
                "HTTP {}: {}",
                response.status_code, response.reason_phrase
            )));
        }

        let utxos: Vec<EsploraUtxo> = response.json().map_err(|e| SimplexError::Deserialize(e.to_string()))?;
        let outpoints = utxos
            .iter()
            .map(|utxo| Ok(self.esplora_utxo_to_outpoint(&utxo)?))
            .collect::<Result<Vec<OutPoint>, SimplexError>>()?;

        Ok(self.populate_txouts_from_outpoints(&outpoints)?)
    }

    fn fetch_scripthash_utxos(&self, script: &Script) -> Result<Vec<(OutPoint, TxOut)>, SimplexError> {
        let hash = sha256::Hash::hash(script.as_bytes());
        let hash_bytes = hash.to_byte_array();
        let scripthash = hex::encode(hash_bytes);

        let url = self.esplora_url.clone() + "/scripthash/" + scripthash.as_str() + "/utxo";
        let response = minreq::get(&url)
            .send()
            .map_err(|e| SimplexError::Request(e.to_string()))?;

        if response.status_code != 200 {
            return Err(SimplexError::Request(format!(
                "HTTP {}: {}",
                response.status_code, response.reason_phrase
            )));
        }

        let utxos: Vec<EsploraUtxo> = response.json().map_err(|e| SimplexError::Deserialize(e.to_string()))?;
        let outpoints = utxos
            .iter()
            .map(|utxo| Ok(self.esplora_utxo_to_outpoint(&utxo)?))
            .collect::<Result<Vec<OutPoint>, SimplexError>>()?;

        Ok(self.populate_txouts_from_outpoints(&outpoints)?)
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
}
