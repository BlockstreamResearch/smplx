use std::collections::HashMap;
use std::str::FromStr;

use simplicityhl::elements::confidential::{Asset, Nonce, Value};
use simplicityhl::elements::hashes::{Hash, sha256};

use simplicityhl::elements::hex::ToHex;
use simplicityhl::elements::{Address, OutPoint, Script, Transaction, TxOut, TxOutWitness, Txid};
use simplicityhl::elements::{AssetId, encode};

use serde::Deserialize;

use super::provider::ProviderTrait;

use crate::error::SimplexError;

#[derive(Clone)]
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

    fn esplora_utxo_to_outpoint(&self, utxo: &EsploraUtxo) -> Result<OutPoint, SimplexError> {
        let txid = Txid::from_str(&utxo.txid).map_err(|e| SimplexError::InvalidTxid(e.to_string()))?;

        Ok(OutPoint::new(txid, utxo.vout))
    }

    fn esplora_utxo_to_txout(&self, utxo: &EsploraUtxo) -> Result<TxOut, SimplexError> {
        let asset = match &utxo.assetcommitment {
            Some(commitment) => {
                Asset::from_commitment(commitment.as_bytes()).map_err(|e| SimplexError::Deserialize(e.to_string()))?
            }
            None => Asset::Explicit(
                AssetId::from_slice(utxo.asset.clone().unwrap().as_bytes())
                    .map_err(|e| SimplexError::Deserialize(e.to_string()))?,
            ),
        };

        let value = match &utxo.valuecommitment {
            Some(commitment) => {
                Value::from_commitment(commitment.as_bytes()).map_err(|e| SimplexError::Deserialize(e.to_string()))?
            }
            None => Value::Explicit(utxo.value.unwrap()),
        };

        Ok(TxOut {
            asset: asset,
            value: value,
            nonce: Nonce::Null,
            script_pubkey: Script::new(),
            witness: TxOutWitness::empty(),
        })
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

        Ok(utxos
            .iter()
            .map(|utxo| {
                Ok((
                    self.esplora_utxo_to_outpoint(&utxo)?,
                    self.esplora_utxo_to_txout(&utxo)?,
                ))
            })
            .collect::<Result<Vec<(OutPoint, TxOut)>, SimplexError>>()?)
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

        Ok(utxos
            .iter()
            .map(|utxo| {
                Ok((
                    self.esplora_utxo_to_outpoint(&utxo)?,
                    self.esplora_utxo_to_txout(&utxo)?,
                ))
            })
            .collect::<Result<Vec<(OutPoint, TxOut)>, SimplexError>>()?)
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
