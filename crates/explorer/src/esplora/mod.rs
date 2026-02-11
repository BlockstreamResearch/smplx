mod types;

use crate::error::ExplorerError;
use crate::esplora::deserializable::TypeConversion;
use simplicityhl::elements::Txid;
use simplicityhl::elements::pset::serialize::Deserialize;
use std::str::FromStr;

const ESPLORA_LIQUID_TESTNET: &str = "https://blockstream.info/liquidtestnet/api";
const ESPLORA_LIQUID: &str = "https://blockstream.info/liquid/api";

pub struct EsploraClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct EsploraClientBuilder {
    url: Option<String>,
}

struct EsploraConfig {
    url: String,
}

// TODO: Illia add caching as optional parameter
impl EsploraClientBuilder {
    fn default_url() -> String {
        ESPLORA_LIQUID_TESTNET.to_string()
    }

    pub fn liquid_testnet() -> Self {
        Self {
            url: Some(ESPLORA_LIQUID_TESTNET.to_string()),
        }
    }

    pub fn liquid_mainnet() -> Self {
        Self {
            url: Some(ESPLORA_LIQUID.to_string()),
        }
    }

    pub fn custom(url: impl Into<String>) -> Self {
        // todo: remove trailling slash
        EsploraClientBuilder { url: Some(url.into()) }
    }

    pub fn build(self) -> EsploraClient {
        EsploraClient {
            base_url: self.url.unwrap_or(Self::default_url()),
            client: reqwest::Client::new(),
        }
    }
}

impl Default for EsploraClientBuilder {
    fn default() -> Self {
        EsploraClientBuilder::liquid_testnet()
    }
}

impl Default for EsploraClient {
    fn default() -> Self {
        crate::esplora::EsploraClientBuilder::default().build()
    }
}

// TODO: add batching with 25 requests

#[cfg(test)]
mod tests {
    use crate::esplora::EsploraClientBuilder;

    #[tokio::test]
    async fn test1() {
        let client = EsploraClientBuilder::liquid_testnet().build();
        println!(
            "== get_tx: {:?}",
            client
                .get_tx("785c3bddbaf2bda8469415c911a407ae8f7b2c1d48883fa7a3d05934ed5454f3")
                .await
        );
        println!(
            "== get_tx_status: {:?}",
            client
                .get_tx_status("785c3bddbaf2bda8469415c911a407ae8f7b2c1d48883fa7a3d05934ed5454f3")
                .await
        );
        println!(
            "== get_tx_raw: {:?}",
            client
                .get_tx_raw("785c3bddbaf2bda8469415c911a407ae8f7b2c1d48883fa7a3d05934ed5454f3")
                .await
        );
        println!(
            "== get_tx_hex: {:?}",
            client
                .get_tx_hex("785c3bddbaf2bda8469415c911a407ae8f7b2c1d48883fa7a3d05934ed5454f3")
                .await
        );
        println!(
            "== get_tx_elements: {:?}",
            client
                .get_tx_elements("785c3bddbaf2bda8469415c911a407ae8f7b2c1d48883fa7a3d05934ed5454f3")
                .await
        );
        println!(
            "== get_tx_elements: {:?}",
            client
                .get_tx_elements("785c3bddbaf2bda8469415c911a407ae8f7b2c1d48883fa7a3d05934ed5454f3")
                .await
        );
        println!(
            "== get_tx_merkleblock_proof: {:?}",
            client
                .get_tx_merkleblock_proof("785c3bddbaf2bda8469415c911a407ae8f7b2c1d48883fa7a3d05934ed5454f3")
                .await
        );
        println!(
            "== get_tx_merkle_proof: {:?}",
            client
                .get_tx_merkle_proof("785c3bddbaf2bda8469415c911a407ae8f7b2c1d48883fa7a3d05934ed5454f3")
                .await
        );
        println!(
            "== get_tx_merkle_proof: {:?}",
            client
                .get_tx_merkle_proof("3d2a1d39ca6b82b215bd2ad5a7594f8a10850db435ec2e8b801c10a74388ccd3")
                .await
        );
        // println!(
        //     "== get_tx_outspend: {:?}",
        //     client
        //         .get_tx_outspend("3d2a1d39ca6b82b215bd2ad5a7594f8a10850db435ec2e8b801c10a74388ccd3", 0)
        //         .await
        // );
        // println!(
        //     "== get_tx_outspends: {:?}",
        //     client
        //         .get_tx_outspends("3d2a1d39ca6b82b215bd2ad5a7594f8a10850db435ec2e8b801c10a74388ccd3")
        //         .await
        // );
        println!(
            "== get_address_txs_mempool: {:?}",
            client
                .get_address_txs_mempool("tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm")
                .await
        );
        println!(
            "== get_address: {:?}",
            client
                .get_address("tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm")
                .await
        );
        println!(
            "== get_address_txs_chain: {:?}",
            client
                .get_address_txs_chain("tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm", None)
                .await
        );
        println!(
            "== get_address_utxo: {:?}",
            client
                .get_address_utxo("tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm")
                .await
        );
        println!(
            "== search_address_prefix: {:?}",
            client
                .search_address_prefix("tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv")
                .await
        );
        println!("== get_mempool: {:?}", client.get_mempool().await);
        println!("== get_mempool_txids: {:?}", client.get_mempool_txids().await);
        println!("== get_mempool_recent: {:?}", client.get_mempool_recent().await);
        println!("== get_fee_estimates: {:?}", client.get_fee_estimates().await);
    }
}

mod deserializable {
    use crate::error::{CommitmentType, ExplorerError};
    use crate::esplora::types;
    use bitcoin_hashes::sha256d::Hash;
    use serde::Deserialize;
    use simplicityhl::elements::confidential::{Asset, Nonce, Value};
    use simplicityhl::elements::{Address, AssetId, BlockHash, OutPoint, Script, Txid};
    use std::str::FromStr;

    pub(crate) trait TypeConversion<T> {
        fn convert(self) -> Result<T, ExplorerError>;
    }

    #[derive(serde::Deserialize)]
    pub struct EsploraTransaction {
        pub txid: String,
        pub version: u32,
        pub locktime: u32,
        pub size: u64,
        pub weight: u64,
        pub fee: u64,
        pub vin: Vec<Vin>,
        pub vout: Vec<Vout>,
        pub status: TxStatus,
    }

    #[derive(serde::Deserialize)]
    pub struct Vin {
        pub txid: String,
        pub vout: u32,
        pub is_coinbase: bool,
        pub scriptsig: String,
        pub scriptsig_asm: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub inner_redeemscript_asm: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub inner_witnessscript_asm: Option<String>,
        pub sequence: u32,
        #[serde(default)]
        pub witness: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub prevout: Option<Vout>,
    }

    #[derive(serde::Deserialize)]
    pub struct Vout {
        pub scriptpubkey: String,
        pub scriptpubkey_asm: String,
        pub scriptpubkey_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub scriptpubkey_address: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub value: Option<u64>,
    }

    #[derive(serde::Deserialize)]
    pub struct TxStatus {
        pub confirmed: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub block_height: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub block_hash: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub block_time: Option<u64>,
    }

    #[derive(serde::Deserialize)]
    pub struct AddressUtxo {
        pub txid: String,
        pub vout: u32,
        pub value: u64,
        pub status: TxStatus,
        #[serde(flatten)]
        pub utxo_info: UtxoInfo,
    }

    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    pub enum UtxoInfo {
        Confidential {
            valuecommitment: String,
            assetcommitment: String,
            noncecommitment: String,
        },
        Explicit {
            value: u64,
            asset: String,
        },
    }

    #[derive(serde::Deserialize)]
    pub struct AddressInfo {
        pub address: String,
        pub chain_stats: types::ChainStats,
        pub mempool_stats: types::MempoolStats,
    }

    #[derive(serde::Deserialize)]
    pub struct MerkleProof {
        pub block_height: u64,
        pub merkle: Vec<String>,
        pub pos: u64,
    }

    #[derive(serde::Deserialize)]
    pub struct Outspend {
        pub spent: bool,
        pub txid: Option<String>,
        pub vin: Option<u32>,
        pub status: Option<TxStatus>,
    }

    #[derive(Debug, Clone, Deserialize, Hash, Eq, PartialEq)]
    pub struct MempoolRecent {
        pub txid: String,
        pub fee: u64,
        pub vsize: u64,
        pub discount_vsize: u64,
    }

    impl TypeConversion<types::TxStatus> for TxStatus {
        fn convert(self) -> Result<types::TxStatus, ExplorerError> {
            let block_hash = match self.block_hash {
                None => None,
                Some(val) => match BlockHash::from_str(&val) {
                    Ok(x) => Some(x),
                    Err(e) => return Err(ExplorerError::BitcoinHashesHex(e)),
                },
            };
            Ok(types::TxStatus {
                confirmed: self.confirmed,
                block_height: self.block_height,
                block_hash,
                block_time: self.block_time,
            })
        }
    }

    impl TypeConversion<types::AddressUtxo> for AddressUtxo {
        fn convert(self) -> Result<types::AddressUtxo, ExplorerError> {
            let block_hash = self.status.block_hash.map(|hash| BlockHash::from_str(&hash));
            let block_hash = match block_hash {
                None => None,
                Some(Err(err)) => return Err(ExplorerError::BitcoinHashesHex(err)),
                Some(Ok(x)) => Some(x),
            };
            let utxo_info = match self.utxo_info {
                UtxoInfo::Confidential {
                    assetcommitment,
                    noncecommitment,
                    valuecommitment,
                } => types::UtxoInfo::Confidential {
                    asset_comm: Asset::from_commitment(
                        &hex_simd::decode_to_vec(assetcommitment)
                            .map_err(|e| ExplorerError::HexSimdDecode(e.to_string()))?,
                    )
                    .map_err(|e| ExplorerError::CommitmentDecode {
                        commitment_type: CommitmentType::Asset,
                        error: e.to_string(),
                    })?,
                    value_comm: Value::from_commitment(
                        &hex_simd::decode_to_vec(valuecommitment)
                            .map_err(|e| ExplorerError::HexSimdDecode(e.to_string()))?,
                    )
                    .map_err(|e| ExplorerError::CommitmentDecode {
                        commitment_type: CommitmentType::Asset,
                        error: e.to_string(),
                    })?,
                    nonce_comm: Nonce::from_commitment(
                        &hex_simd::decode_to_vec(noncecommitment)
                            .map_err(|e| ExplorerError::HexSimdDecode(e.to_string()))?,
                    )
                    .map_err(|e| ExplorerError::CommitmentDecode {
                        commitment_type: CommitmentType::Asset,
                        error: e.to_string(),
                    })?,
                },
                UtxoInfo::Explicit { asset, value } => types::UtxoInfo::Explicit {
                    value,
                    asset: AssetId::from_str(&asset).map_err(|err| ExplorerError::BitcoinHashesHex(err))?,
                },
            };

            Ok(types::AddressUtxo {
                outpoint: OutPoint::new(Txid::from_str(&self.txid)?, self.vout),
                status: types::TxStatus {
                    confirmed: self.status.confirmed,
                    block_height: self.status.block_height,
                    block_hash,
                    block_time: self.status.block_time,
                },
                utxo_info,
            })
        }
    }

    impl TypeConversion<types::MerkleProof> for MerkleProof {
        fn convert(self) -> Result<types::MerkleProof, ExplorerError> {
            let hashes = self
                .merkle
                .into_iter()
                .map(|x| Ok(Hash::from_str(&x)?))
                .collect::<Result<Vec<Hash>, bitcoin_hashes::hex::HexToArrayError>>()?;
            let merkle_proofs = hashes
                .into_iter()
                .map(|hash| simplicityhl::elements::TxMerkleNode::from_raw_hash(hash))
                .collect();
            Ok(types::MerkleProof {
                block_height: self.block_height,
                merkle: merkle_proofs,
                pos: self.pos,
            })
        }
    }

    impl TypeConversion<types::AddressInfo> for AddressInfo {
        fn convert(self) -> Result<types::AddressInfo, ExplorerError> {
            Ok(types::AddressInfo {
                address: Address::from_str(&self.address)
                    .map_err(|e| ExplorerError::AddressConversion(e.to_string()))?,
                chain_stats: self.chain_stats,
                mempool_stats: self.mempool_stats,
            })
        }
    }

    impl TypeConversion<types::EsploraTransaction> for EsploraTransaction {
        fn convert(self) -> Result<types::EsploraTransaction, ExplorerError> {
            let status = self.status.convert()?;
            let vin = self.vin.into_iter().map(|x| x.convert()).collect::<Result<_, _>>()?;
            let vout = self.vout.into_iter().map(|x| x.convert()).collect::<Result<_, _>>()?;

            Ok(types::EsploraTransaction {
                txid: Txid::from_str(&self.txid)?,
                version: self.version,
                locktime: self.locktime,
                size: self.size,
                weight: self.weight,
                fee: self.fee,
                vin,
                vout,
                status,
            })
        }
    }

    impl TypeConversion<types::Vout> for Vout {
        fn convert(self) -> Result<types::Vout, ExplorerError> {
            Ok(types::Vout {
                scriptpubkey: Script::from_str(&self.scriptpubkey).map_err(|e| ExplorerError::ElementsHex(e))?,
                scriptpubkey_asm: self.scriptpubkey_asm,
                scriptpubkey_type: self.scriptpubkey_type,
                scriptpubkey_address: self.scriptpubkey_address,
                value: self.value,
            })
        }
    }
    impl TypeConversion<types::Vin> for Vin {
        fn convert(self) -> Result<types::Vin, ExplorerError> {
            let prevout = match self.prevout {
                None => None,
                Some(val) => Some(val.convert()?),
            };

            Ok(types::Vin {
                out_point: Default::default(),
                is_coinbase: self.is_coinbase,
                scriptsig: self.scriptsig,
                scriptsig_asm: self.scriptsig_asm,
                inner_redeemscript_asm: self.inner_redeemscript_asm,
                inner_witnessscript_asm: self.inner_witnessscript_asm,
                sequence: self.sequence,
                witness: self.witness,
                prevout,
            })
        }
    }

    impl TypeConversion<types::Outspend> for Outspend {
        fn convert(self) -> Result<types::Outspend, ExplorerError> {
            let status = match self.status {
                None => None,
                Some(val) => Some(val.convert()?),
            };
            let txid = match self.txid {
                None => None,
                Some(val) => Some(Txid::from_str(&val)?),
            };

            Ok(types::Outspend {
                spent: self.spent,
                txid,
                vin: self.vin,
                status,
            })
        }
    }

    impl TypeConversion<types::MempoolRecent> for MempoolRecent {
        fn convert(self) -> Result<types::MempoolRecent, ExplorerError> {
            Ok(types::MempoolRecent {
                txid: Txid::from_str(&self.txid)?,
                fee: 0,
                vsize: 0,
                discount_vsize: 0,
            })
        }
    }
}

impl EsploraClient {
    #[inline]
    fn join_url(&self, str: impl AsRef<str>) -> Result<String, ExplorerError> {
        Ok(format!("{}/{}", self.base_url, str.as_ref()))
    }

    #[inline]
    fn filter_resp(resp: &reqwest::Response) -> Result<(), ExplorerError> {
        if !(200..300).contains(&resp.status().as_u16()) {
            return Err(ExplorerError::erroneous_response(resp));
        }
        Ok(())
    }

    pub async fn get_tx(&self, txid: &str) -> Result<types::EsploraTransaction, ExplorerError> {
        let url = self.join_url(&format!("/tx/{txid}"))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::EsploraTransaction>()
            .await
            .map_err(|e| ExplorerError::deserialize(&e))?;
        let resp = resp.convert()?;

        Ok(resp)
    }

    pub async fn get_tx_status(&self, txid: &str) -> Result<types::TxStatus, ExplorerError> {
        let url = self.join_url(&format!("tx/{txid}/status"))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::TxStatus>()
            .await
            .map_err(|e| ExplorerError::deserialize(&e))?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    pub async fn get_tx_hex(&self, txid: &str) -> Result<String, ExplorerError> {
        let url = self.join_url(&format!("tx/{}/hex", txid))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        resp.text().await.map_err(|e| ExplorerError::deserialize(&e))
    }

    pub async fn get_tx_raw(&self, txid: &str) -> Result<Vec<u8>, ExplorerError> {
        let url = self.join_url(&format!("tx/{}/raw", txid))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| ExplorerError::deserialize(&e))
    }

    pub async fn get_tx_elements(&self, txid: &str) -> Result<simplicityhl::elements::Transaction, ExplorerError> {
        let bytes = self.get_tx_raw(txid).await?;
        Ok(simplicityhl::elements::Transaction::deserialize(&bytes).unwrap())
    }

    // TODO: erroneous
    pub async fn get_tx_merkleblock_proof(&self, txid: &str) -> Result<String, ExplorerError> {
        let url = self.join_url(&format!("tx/{}/merkleblock-proof", txid))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        resp.text().await.map_err(|e| ExplorerError::deserialize(&e))
    }

    pub async fn get_tx_merkle_proof(&self, txid: &str) -> Result<types::MerkleProof, ExplorerError> {
        let url = self.join_url(&format!("tx/{}/merkle-proof", txid))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::MerkleProof>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    pub async fn get_tx_outspend(&self, txid: &str, vout: u32) -> Result<types::Outspend, ExplorerError> {
        let url = self.join_url(&format!("tx/{}/outspend/{}", txid, vout))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::Outspend>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    pub async fn get_tx_outspends(&self, txid: &str) -> Result<Vec<types::Outspend>, ExplorerError> {
        let url = self.join_url(&format!("tx/{}/outspends", txid))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::Outspend>>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        resp.into_iter().map(|x| x.convert()).collect::<Result<Vec<_>, _>>()
    }

    pub async fn broadcast_tx(&self, tx: &simplicityhl::elements::Transaction) -> Result<String, ExplorerError> {
        let tx_hex = simplicityhl::elements::encode::serialize_hex(tx);
        let url = self.join_url("tx")?;
        let resp = self
            .client
            .post(url)
            .body(tx_hex)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        resp.text().await.map_err(|e| ExplorerError::response_failed(&e))
    }

    // TODO: add batch execution with 25 elements
    pub async fn broadcast_tx_package(
        &self,
        txs: &[simplicityhl::elements::Transaction],
    ) -> Result<serde_json::Value, ExplorerError> {
        let url = self.join_url("txs/package")?;
        let tx_hexes = txs
            .iter()
            .map(|tx| simplicityhl::elements::encode::serialize_hex(tx))
            .collect::<Vec<_>>();

        let resp = self
            .client
            .post(url)
            .json(&tx_hexes)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        resp.json().await.map_err(|e| ExplorerError::response_failed(&e))
    }

    pub async fn get_address(&self, address: &str) -> Result<types::AddressInfo, ExplorerError> {
        let url = self.join_url(&format!("address/{}", address))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::AddressInfo>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp.convert()?;

        Ok(resp)
    }

    pub async fn get_address_txs(&self, address: &str) -> Result<Vec<types::EsploraTransaction>, ExplorerError> {
        let url = self.join_url(&format!("address/{}/txs", address))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;
        let resp = resp
            .json::<Vec<deserializable::EsploraTransaction>>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp.into_iter().map(|x| x.convert()).collect::<Result<_, _>>()?;
        Ok(resp)
    }

    pub async fn get_address_txs_chain(
        &self,
        address: &str,
        last_seen_txid: Option<&str>,
    ) -> Result<Vec<types::EsploraTransaction>, ExplorerError> {
        let url = if let Some(txid) = last_seen_txid {
            self.join_url(&format!("address/{}/txs/chain/{}", address, txid))?
        } else {
            self.join_url(&format!("address/{}/txs/chain", address))?
        };
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::EsploraTransaction>>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp.into_iter().map(|x| x.convert()).collect::<Result<_, _>>()?;
        Ok(resp)
    }

    pub async fn get_address_txs_mempool(
        &self,
        address: &str,
    ) -> Result<Vec<types::EsploraTransaction>, ExplorerError> {
        let url = self.join_url(&format!("address/{}/txs/mempool", address))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::EsploraTransaction>>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp.into_iter().map(|x| x.convert()).collect::<Result<_, _>>()?;
        Ok(resp)
    }

    pub async fn get_address_utxo(&self, address: &str) -> Result<Vec<types::AddressUtxo>, ExplorerError> {
        let url = self.join_url(&format!("address/{}/utxo", address))?;
        println!("{url}");
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::AddressUtxo>>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        resp.into_iter().map(|x| x.convert()).collect::<Result<Vec<_>, _>>()
    }

    pub async fn search_address_prefix(&self, prefix: &str) -> Result<Vec<String>, ExplorerError> {
        let url = self.join_url(&format!("address-prefix/{}", prefix))?;
        println!("{url}");
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        resp.json().await.map_err(|e| ExplorerError::response_failed(&e))
    }

    // pub async fn get_scripthash(&self, hash: &str) -> Result<ScripthashInfo, ExplorerError> {
    //     let url = self.join_url(&format!("scripthash/{}", hash))?;
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.json().await.map_err(ExplorerError::from_reqwest)
    // }
    //
    // pub async fn get_scripthash_txs(&self, hash: &str) -> Result<Vec<Transaction>, ExplorerError> {
    //     let url = self.join_url(&format!("scripthash/{}/txs", hash))?;
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.json().await.map_err(ExplorerError::from_reqwest)
    // }
    //
    // pub async fn get_scripthash_txs_chain(
    //     &self,
    //     hash: &str,
    //     last_seen_txid: Option<&str>,
    // ) -> Result<Vec<Transaction>, ExplorerError> {
    //     let url = if let Some(txid) = last_seen_txid {
    //         self.join_url(&format!("scripthash/{}/txs/chain/{}", hash, txid))?
    //     } else {
    //         self.join_url(&format!("scripthash/{}/txs/chain", hash))?
    //     };
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.json().await.map_err(ExplorerError::from_reqwest)
    // }
    //
    // pub async fn get_scripthash_txs_mempool(&self, hash: &str) -> Result<Vec<Transaction>, ExplorerError> {
    //     let url = self.join_url(&format!("scripthash/{}/txs/mempool", hash))?;
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.json().await.map_err(ExplorerError::from_reqwest)
    // }
    //
    // pub async fn get_scripthash_utxo(&self, hash: &str) -> Result<Vec<Utxo>, ExplorerError> {
    //     let url = self.join_url(&format!("scripthash/{}/utxo", hash))?;
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.json().await.map_err(ExplorerError::from_reqwest)
    // }
    //
    // // Block endpoints
    // pub async fn get_block(&self, hash: &str) -> Result<Block, ExplorerError> {
    // #[derive(serde::Deserialize)]
    // pub struct Block {
    //     pub id: String,
    //     pub height: u64,
    //     pub version: u32,
    //     pub timestamp: u64,
    //     pub mediantime: u64,
    //     pub bits: u32,
    //     pub nonce: u32,
    //     pub merkle_root: String,
    //     pub tx_count: u64,
    //     pub size: u64,
    //     pub weight: u64,
    //     pub previousblockhash: String,
    //     #[serde(skip_serializing_if = "Option::is_none")]
    //     pub difficulty: Option<f64>,
    // }
    //     let url = self.join_url(&format!("block/{}", hash))?;
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.json().await.map_err(ExplorerError::from_reqwest)
    // }
    //
    // pub async fn get_block_header(&self, hash: &str) -> Result<String, ExplorerError> {
    //     let url = self.join_url(&format!("block/{}/header", hash))?;
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.text().await.map_err(ExplorerError::from_reqwest)
    // }
    //
    // pub async fn get_block_status(&self, hash: &str) -> Result<BlockStatus, ExplorerError> {
    //     let url = self.join_url(&format!("block/{}/status", hash))?;
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.json().await.map_err(ExplorerError::from_reqwest)
    // }
    //
    // pub async fn get_block_txs(&self, hash: &str, start_index: Option<u32>) -> Result<Vec<Transaction>, ExplorerError> {
    //     let url = if let Some(index) = start_index {
    //         self.join_url(&format!("block/{}/txs/{}", hash, index))?
    //     } else {
    //         self.join_url(&format!("block/{}/txs", hash))?
    //     };
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.json().await.map_err(ExplorerError::from_reqwest)
    // }
    //
    // pub async fn get_block_txids(&self, hash: &str) -> Result<Vec<String>, ExplorerError> {
    //     let url = self.join_url(&format!("block/{}/txids", hash))?;
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.json().await.map_err(ExplorerError::from_reqwest)
    // }
    //
    // pub async fn get_block_txid(&self, hash: &str, index: u32) -> Result<String, ExplorerError> {
    //     let url = self.join_url(&format!("block/{}/txid/{}", hash, index))?;
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.text().await.map_err(ExplorerError::from_reqwest)
    // }
    //
    // pub async fn get_block_raw(&self, hash: &str) -> Result<Vec<u8>, ExplorerError> {
    //     let url = self.join_url(&format!("block/{}/raw", hash))?;
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.bytes().await.map(|b| b.to_vec()).map_err(ExplorerError::from_reqwest)
    // }
    //
    // pub async fn get_block_height(&self, height: u64) -> Result<String, ExplorerError> {
    //     let url = self.join_url(&format!("block-height/{}", height))?;
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.text().await.map_err(ExplorerError::from_reqwest)
    // }
    //
    // pub async fn get_blocks(&self, start_height: Option<u64>) -> Result<Vec<Block>, ExplorerError> {
    //     let url = if let Some(height) = start_height {
    //         self.join_url(&format!("blocks/{}", height))?
    //     } else {
    //         self.join_url("blocks")?
    //     };
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.json().await.map_err(ExplorerError::from_reqwest)
    // }
    //
    // pub async fn get_blocks_tip_height(&self) -> Result<u64, ExplorerError> {
    //     let url = self.join_url("blocks/tip/height")?;
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     let text = resp.text().await.map_err(ExplorerError::from_reqwest)?;
    //     text.parse().map_err(|_| {
    //         ExplorerError::from_reqwest(reqwest::Error::from(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid height")))
    //     })
    // }
    //
    // pub async fn get_blocks_tip_hash(&self) -> Result<String, ExplorerError> {
    //     let url = self.join_url("blocks/tip/hash")?;
    //     let resp = self.client.get(url).send().await.map_err(ExplorerError::from_reqwest)?;
    //     resp.text().await.map_err(ExplorerError::from_reqwest)
    // }
    //
    pub async fn get_mempool(&self) -> Result<types::MempoolInfo, ExplorerError> {
        let url = self.join_url("mempool")?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        resp.json().await.map_err(|e| ExplorerError::response_failed(&e))
    }

    pub async fn get_mempool_txids(&self) -> Result<Vec<Txid>, ExplorerError> {
        let url = self.join_url("mempool/txids")?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<String>>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp
            .into_iter()
            .map(|val| Txid::from_str(&val))
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    pub async fn get_mempool_recent(&self) -> Result<Vec<types::MempoolRecent>, ExplorerError> {
        let url = self.join_url("mempool/recent")?;
        println!("{url}");
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::MempoolRecent>>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp.into_iter().map(|x| x.convert()).collect::<Result<_, _>>()?;
        Ok(resp)
    }

    pub async fn get_fee_estimates(&self) -> Result<types::FeeEstimates, ExplorerError> {
        let url = self.join_url("fee-estimates")?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        resp.json::<types::FeeEstimates>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))
    }
}
