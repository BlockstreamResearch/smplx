mod types;

// TODO(Illia): remove #[allow(dead_code)]

use crate::error::ExplorerError;
use crate::esplora::deserializable::TypeConversion;
use simplicityhl::elements::pset::serialize::Deserialize;
use simplicityhl::elements::{BlockHash, Txid};
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

#[allow(dead_code)]
pub struct EsploraConfig {
    url: String,
}

// TODO: Illia add caching as optional parameter
// TODO: Add api backend trait implementation
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
        EsploraClientBuilder::default().build()
    }
}

mod deserializable {
    use crate::error::{CommitmentType, ExplorerError};
    use crate::esplora::types;
    use crate::esplora::types::Stats;
    use bitcoin_hashes::sha256d::Hash;
    use simplicityhl::elements::confidential::{Asset, Nonce, Value};
    use simplicityhl::elements::{Address, AssetId, BlockHash, OutPoint, Script, TxMerkleNode, Txid};
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
        pub discount_vsize: u64,
        pub discount_weight: u64,
    }

    #[allow(dead_code)]
    #[derive(serde::Deserialize)]
    pub struct Vin {
        pub txid: String,
        pub vout: u32,
        pub is_coinbase: bool,
        pub scriptsig: String,
        pub scriptsig_asm: String,
        pub inner_redeemscript_asm: Option<String>,
        pub inner_witnessscript_asm: Option<String>,
        pub sequence: u32,
        #[serde(default)]
        pub witness: Vec<String>,
        pub prevout: Option<Vout>,
    }

    #[derive(serde::Deserialize)]
    pub struct Vout {
        pub scriptpubkey: String,
        pub scriptpubkey_asm: String,
        pub scriptpubkey_type: String,
        pub scriptpubkey_address: Option<String>,
        pub value: Option<u64>,
    }

    #[derive(serde::Deserialize)]
    pub struct TxStatus {
        pub confirmed: bool,
        pub block_height: Option<u64>,
        pub block_hash: Option<String>,
        pub block_time: Option<u64>,
    }

    #[derive(serde::Deserialize)]
    pub struct AddressUtxo {
        pub txid: String,
        pub vout: u32,
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

    #[allow(dead_code)]
    #[derive(serde::Deserialize)]
    pub struct MempoolRecent {
        pub txid: String,
        pub fee: u64,
        pub vsize: u64,
        pub discount_vsize: u64,
    }

    #[derive(serde::Deserialize)]
    pub struct ScripthashInfo {
        pub scripthash: String,
        pub chain_stats: Stats,
        pub mempool_stats: Stats,
    }

    #[derive(serde::Deserialize)]
    pub struct Block {
        pub id: String,
        pub height: u64,
        pub version: u32,
        pub timestamp: u64,
        pub mediantime: u64,
        pub merkle_root: String,
        pub tx_count: u64,
        pub size: u64,
        pub weight: u64,
        pub previousblockhash: String,
        pub ext: Option<BlockExtDataRaw>,
    }

    #[allow(dead_code)]
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    pub enum BlockExtDataRaw {
        Proof {
            challenge: String,
            solution: String,
        },
        Dynafed {
            current: DynafedParamsRaw,
            proposed: DynafedParamsRaw,
            signblock_witness: Vec<Vec<u8>>,
        },
    }

    #[allow(dead_code)]
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    pub enum DynafedParamsRaw {
        Null {},
        Compact {
            signblockscript: String,
            signblock_witness_limit: u32,
            elided_root: String,
        },
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
                        &hex_simd::decode_to_vec(assetcommitment).map_err(ExplorerError::HexSimdDecode)?,
                    )
                    .map_err(|e| ExplorerError::CommitmentDecode {
                        commitment_type: CommitmentType::Asset,
                        error: e,
                    })?,
                    value_comm: Value::from_commitment(
                        &hex_simd::decode_to_vec(valuecommitment).map_err( ExplorerError::HexSimdDecode)?,
                    )
                    .map_err(|e| ExplorerError::CommitmentDecode {
                        commitment_type: CommitmentType::Asset,
                        error: e,
                    })?,
                    nonce_comm: Nonce::from_commitment(
                        &hex_simd::decode_to_vec(noncecommitment).map_err(ExplorerError::HexSimdDecode)?,
                    )
                    .map_err(|e| ExplorerError::CommitmentDecode {
                        commitment_type: CommitmentType::Asset,
                        error: e,
                    })?,
                },
                UtxoInfo::Explicit { asset, value } => types::UtxoInfo::Explicit {
                    value,
                    asset: AssetId::from_str(&asset).map_err(ExplorerError::BitcoinHashesHex)?,
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
                .map(|x| Hash::from_str(&x))
                .collect::<Result<Vec<Hash>, bitcoin_hashes::hex::HexToArrayError>>()?;
            let merkle_proofs = hashes.into_iter().map(TxMerkleNode::from_raw_hash).collect();
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
                discount_vsize: self.discount_vsize,
                discount_weight: self.discount_weight,
            })
        }
    }

    impl TypeConversion<types::Vout> for Vout {
        fn convert(self) -> Result<types::Vout, ExplorerError> {
            Ok(types::Vout {
                scriptpubkey: Script::from_str(&self.scriptpubkey).map_err(ExplorerError::ElementsHex)?,
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

    impl TypeConversion<types::ScripthashInfo> for ScripthashInfo {
        fn convert(self) -> Result<types::ScripthashInfo, ExplorerError> {
            Ok(types::ScripthashInfo {
                scripthash: Script::from_str(&self.scripthash).map_err(ExplorerError::ElementsHex)?,
                chain_stats: self.chain_stats,
                mempool_stats: self.mempool_stats,
            })
        }
    }

    impl TypeConversion<types::Block> for Block {
        fn convert(self) -> Result<types::Block, ExplorerError> {
            let ext = match self.ext {
                None => None,
                Some(val) => Some(val.convert()?),
            };
            Ok(types::Block {
                id: self.id,
                height: self.height,
                version: self.version,
                timestamp: self.timestamp,
                tx_count: self.tx_count,
                size: self.size,
                weight: self.weight,
                merkle_root: TxMerkleNode::from_str(&self.merkle_root)?,
                mediantime: self.mediantime,
                previousblockhash: BlockHash::from_str(&self.previousblockhash)?,
                ext,
            })
        }
    }

    impl TypeConversion<simplicityhl::elements::BlockExtData> for BlockExtDataRaw {
        fn convert(self) -> Result<simplicityhl::elements::BlockExtData, ExplorerError> {
            todo!()
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
        let url = self.join_url(format!("/tx/{txid}"))?;
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
        let url = self.join_url(format!("tx/{txid}/status"))?;
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
        let url = self.join_url(format!("tx/{txid}/hex"))?;
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
        let url = self.join_url(format!("tx/{txid}/raw"))?;
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
        simplicityhl::elements::Transaction::deserialize(&bytes)
            .map_err(|e| ExplorerError::TransactionDecode(e.to_string()))
    }

    pub async fn get_tx_merkle_proof(&self, txid: &str) -> Result<types::MerkleProof, ExplorerError> {
        let url = self.join_url(format!("tx/{txid}/merkle-proof"))?;
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
        let url = self.join_url(format!("tx/{txid}/outspend/{vout}"))?;
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
        let url = self.join_url(format!("tx/{txid}/outspends"))?;
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

    // TODO: add batch execution with 10 elements
    pub async fn broadcast_tx_package(
        &self,
        txs: &[simplicityhl::elements::Transaction],
    ) -> Result<serde_json::Value, ExplorerError> {
        let url = self.join_url("txs/package")?;
        let tx_hexes = txs
            .iter()
            .map(simplicityhl::elements::encode::serialize_hex)
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
        let url = self.join_url(format!("address/{address}"))?;
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
        let url = self.join_url(format!("address/{address}/txs"))?;
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
            self.join_url(format!("address/{address}/txs/chain/{txid}"))?
        } else {
            self.join_url(format!("address/{address}/txs/chain"))?
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
        let url = self.join_url(format!("address/{address}/txs/mempool"))?;
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
        let url = self.join_url(format!("address/{address}/utxo"))?;
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

    pub async fn get_scripthash(&self, hash: &str) -> Result<types::ScripthashInfo, ExplorerError> {
        let url = self.join_url(format!("scripthash/{hash}"))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::ScripthashInfo>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    // TODO: check output
    pub async fn get_scripthash_txs(&self, hash: &str) -> Result<String, ExplorerError> {
        let url = self.join_url(format!("scripthash/{hash}/txs"))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        resp.text().await.map_err(|e| ExplorerError::response_failed(&e))
    }

    // TODO: check output
    pub async fn get_scripthash_txs_chain(
        &self,
        hash: &str,
        last_seen_txid: Option<&str>,
    ) -> Result<String, ExplorerError> {
        let url = if let Some(txid) = last_seen_txid {
            self.join_url(format!("scripthash/{hash}/txs/chain/{txid}"))?
        } else {
            self.join_url(format!("scripthash/{hash}/txs/chain"))?
        };
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        resp.text().await.map_err(|e| ExplorerError::response_failed(&e))
    }

    // TODO: check output
    pub async fn get_scripthash_txs_mempool(&self, hash: &str) -> Result<String, ExplorerError> {
        let url = self.join_url(format!("scripthash/{hash}/txs/mempool"))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        resp.text().await.map_err(|e| ExplorerError::response_failed(&e))
    }

    // TODO: check output
    pub async fn get_scripthash_utxo(&self, hash: &str) -> Result<String, ExplorerError> {
        let url = self.join_url(format!("scripthash/{hash}/utxo"))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        resp.text().await.map_err(|e| ExplorerError::response_failed(&e))
    }

    pub async fn get_block(&self, hash: &str) -> Result<types::Block, ExplorerError> {
        let url = self.join_url(format!("block/{hash}"))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::Block>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    // TODO: decode hex into elements::BlockHeader (no method to do this)
    pub async fn get_block_header(&self, hash: &str) -> Result<String, ExplorerError> {
        let url = self.join_url(format!("block/{hash}/header"))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp.text().await.map_err(|e| ExplorerError::response_failed(&e))?;
        Ok(resp)
    }

    pub async fn get_block_status(&self, hash: &str) -> Result<types::BlockStatus, ExplorerError> {
        let url = self.join_url(format!("block/{hash}/status"))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        resp.json::<types::BlockStatus>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))
    }

    pub async fn get_block_txs(
        &self,
        hash: &str,
        start_index: Option<u32>,
    ) -> Result<Vec<types::EsploraTransaction>, ExplorerError> {
        let url = if let Some(index) = start_index {
            self.join_url(format!("block/{hash}/txs/{index}"))?
        } else {
            self.join_url(format!("block/{hash}/txs"))?
        };
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp
            .json::<Vec<deserializable::EsploraTransaction>>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp.into_iter().map(|val| val.convert()).collect::<Result<_, _>>()?;
        Ok(resp)
    }

    pub async fn get_block_txids(&self, hash: &str) -> Result<Vec<Txid>, ExplorerError> {
        let url = self.join_url(format!("block/{hash}/txids"))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
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

    pub async fn get_block_txid(&self, hash: &str, index: u32) -> Result<Txid, ExplorerError> {
        let url = self.join_url(format!("block/{hash}/txid/{index}"))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp.text().await.map_err(|e| ExplorerError::response_failed(&e))?;

        Ok(Txid::from_str(&resp)?)
    }

    pub async fn get_block_raw(&self, hash: &str) -> Result<Vec<u8>, ExplorerError> {
        let url = self.join_url(format!("block/{hash}/raw"))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| ExplorerError::response_failed(&e))
    }

    pub async fn get_block_height(&self, height: u64) -> Result<BlockHash, ExplorerError> {
        let url = self.join_url(format!("block-height/{height}"))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp.text().await.map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = BlockHash::from_str(&resp)?;
        Ok(resp)
    }

    pub async fn get_blocks(&self, start_height: Option<u64>) -> Result<Vec<types::Block>, ExplorerError> {
        let url = if let Some(height) = start_height {
            self.join_url(format!("blocks/{}", height))?
        } else {
            self.join_url("blocks")?
        };
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::Block>>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = resp.into_iter().map(|val| val.convert()).collect::<Result<_, _>>()?;
        Ok(resp)
    }

    pub async fn get_blocks_tip_height(&self) -> Result<u64, ExplorerError> {
        let url = self.join_url("blocks/tip/height")?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<u64>()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Ok(resp)
    }

    pub async fn get_blocks_tip_hash(&self) -> Result<BlockHash, ExplorerError> {
        let url = self.join_url("blocks/tip/hash")?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp.text().await.map_err(|e| ExplorerError::response_failed(&e))?;
        let resp = BlockHash::from_str(&resp)?;
        Ok(resp)
    }

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
