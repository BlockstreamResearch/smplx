mod types;

// TODO(Illia): remove #[allow(dead_code)]

use crate::error::ExplorerError;
use crate::esplora::deserializable::TypeConversion;
use simplicityhl::elements::pset::serialize::Deserialize;
use simplicityhl::elements::{BlockHash, Txid};
use std::str::FromStr;

const ESPLORA_LIQUID_TESTNET: &str = "https://blockstream.info/liquidtestnet/api";
const ESPLORA_LIQUID: &str = "https://blockstream.info/liquid/api";

pub struct EsploraClientAsync {
    url_builder: UrlBuilder,
    client: reqwest::Client,
}

pub struct EsploraClientSync {
    url_builder: UrlBuilder,
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

    #[must_use]
    pub fn liquid_testnet() -> Self {
        Self {
            url: Some(ESPLORA_LIQUID_TESTNET.to_string()),
        }
    }

    #[must_use]
    pub fn liquid_mainnet() -> Self {
        Self {
            url: Some(ESPLORA_LIQUID.to_string()),
        }
    }

    pub fn custom(url: impl Into<String>) -> Self {
        // todo: remove trailling slash
        EsploraClientBuilder { url: Some(url.into()) }
    }

    #[must_use]
    pub fn build_async(self) -> EsploraClientAsync {
        EsploraClientAsync {
            url_builder: UrlBuilder {
                base_url: self.url.unwrap_or(Self::default_url()),
            },
            client: reqwest::Client::new(),
        }
    }

    #[must_use]
    pub fn build_sync(self) -> EsploraClientSync {
        EsploraClientSync {
            url_builder: UrlBuilder {
                base_url: self.url.unwrap_or(Self::default_url()),
            },
        }
    }
}

impl Default for EsploraClientBuilder {
    fn default() -> Self {
        EsploraClientBuilder::liquid_testnet()
    }
}

impl Default for EsploraClientAsync {
    fn default() -> Self {
        EsploraClientBuilder::default().build_async()
    }
}

impl Default for EsploraClientSync {
    fn default() -> Self {
        EsploraClientBuilder::default().build_sync()
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
                        &hex_simd::decode_to_vec(valuecommitment).map_err(ExplorerError::HexSimdDecode)?,
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
            let vin = self
                .vin
                .into_iter()
                .map(TypeConversion::convert)
                .collect::<Result<_, _>>()?;
            let vout = self
                .vout
                .into_iter()
                .map(TypeConversion::convert)
                .collect::<Result<_, _>>()?;

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
                out_point: OutPoint::default(),
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

impl EsploraClientAsync {
    #[inline]
    fn filter_resp(resp: &reqwest::Response) -> Result<(), ExplorerError> {
        if is_resp_ok(i32::from(resp.status().as_u16())) {
            return Err(ExplorerError::erroneous_response_reqwest(resp));
        }
        Ok(())
    }

    /// Retrieves transaction details by transaction ID.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub async fn get_tx(&self, txid: &str) -> Result<types::EsploraTransaction, ExplorerError> {
        let url = self.url_builder.get_tx_url(txid)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::EsploraTransaction>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = resp.convert()?;

        Ok(resp)
    }

    /// Retrieves transaction status by transaction ID.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if block hash parsing fails
    pub async fn get_tx_status(&self, txid: &str) -> Result<types::TxStatus, ExplorerError> {
        let url = self.url_builder.get_tx_status_url(txid)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::TxStatus>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    /// Retrieves transaction hex by transaction ID.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if response text extraction fails
    pub async fn get_tx_hex(&self, txid: &str) -> Result<String, ExplorerError> {
        let url = self.url_builder.get_tx_hex_url(txid)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        resp.text().await.map_err(|e| ExplorerError::deserialize_reqwest(&e))
    }

    /// Retrieves raw transaction bytes by transaction ID.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if response bytes extraction fails
    pub async fn get_tx_raw(&self, txid: &str) -> Result<Vec<u8>, ExplorerError> {
        let url = self.url_builder.get_tx_raw_url(txid)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))
    }

    /// Retrieves and deserializes a transaction as an Elements transaction.
    ///
    /// # Errors
    /// - Returns all errors from `get_tx_raw`
    /// - Returns `ExplorerError::TransactionDecode` if transaction deserialization fails
    pub async fn get_tx_elements(&self, txid: &str) -> Result<simplicityhl::elements::Transaction, ExplorerError> {
        let bytes = self.get_tx_raw(txid).await?;
        simplicityhl::elements::Transaction::deserialize(&bytes)
            .map_err(|e| ExplorerError::TransactionDecode(e.to_string()))
    }

    /// Retrieves merkle proof for a transaction.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if merkle hash parsing fails
    pub async fn get_tx_merkle_proof(&self, txid: &str) -> Result<types::MerkleProof, ExplorerError> {
        let url = self.url_builder.get_tx_merkle_proof_url(txid)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::MerkleProof>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    /// Retrieves outspend information for a specific output.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub async fn get_tx_outspend(&self, txid: &str, vout: u32) -> Result<types::Outspend, ExplorerError> {
        let url = self.url_builder.get_tx_outspend_url(txid, vout)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::Outspend>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    /// Retrieves outspend information for all outputs of a transaction.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub async fn get_tx_outspends(&self, txid: &str) -> Result<Vec<types::Outspend>, ExplorerError> {
        let url = self.url_builder.get_tx_outspends_url(txid)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::Outspend>>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        resp.into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<Vec<_>, _>>()
    }

    /// Broadcasts a transaction to the network.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if response text extraction fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub async fn broadcast_tx(&self, tx: &simplicityhl::elements::Transaction) -> Result<Txid, ExplorerError> {
        let tx_hex = simplicityhl::elements::encode::serialize_hex(tx);
        let url = self.url_builder.get_broadcast_tx_url()?;
        let resp = self
            .client
            .post(url)
            .body(tx_hex)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp.text().await.map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        Ok(Txid::from_str(&resp)?)
    }

    /// Broadcasts a package of transactions.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    pub async fn broadcast_tx_package(
        &self,
        txs: &[simplicityhl::elements::Transaction],
    ) -> Result<serde_json::Value, ExplorerError> {
        let url = self.url_builder.get_broadcast_tx_package_url()?;
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
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        resp.json().await.map_err(|e| ExplorerError::deserialize_reqwest(&e))
    }

    /// Retrieves address information.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::AddressConversion` if address parsing fails
    pub async fn get_address(&self, address: &str) -> Result<types::AddressInfo, ExplorerError> {
        let url = self.url_builder.get_address_url(address)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::AddressInfo>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = resp.convert()?;

        Ok(resp)
    }

    /// Retrieves all transactions for an address.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID/block hash parsing fails
    pub async fn get_address_txs(&self, address: &str) -> Result<Vec<types::EsploraTransaction>, ExplorerError> {
        let url = self.url_builder.get_address_txs_url(address)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;
        let resp = resp
            .json::<Vec<deserializable::EsploraTransaction>>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = resp.into_iter().map(|x| x.convert()).collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves confirmed transactions for an address with pagination.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID/block hash parsing fails
    pub async fn get_address_txs_chain(
        &self,
        address: &str,
        last_seen_txid: Option<&str>,
    ) -> Result<Vec<types::EsploraTransaction>, ExplorerError> {
        let url = self.url_builder.get_address_txs_chain_url(address, last_seen_txid)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::EsploraTransaction>>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = resp
            .into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves mempool transactions for an address.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub async fn get_address_txs_mempool(
        &self,
        address: &str,
    ) -> Result<Vec<types::EsploraTransaction>, ExplorerError> {
        let url = self.url_builder.get_address_txs_mempool_url(address)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::EsploraTransaction>>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = resp
            .into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves UTXOs for an address.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::HexSimdDecode` if hex decoding fails
    /// - Returns `ExplorerError::CommitmentDecode` if commitment parsing fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID/block hash parsing fails
    pub async fn get_address_utxo(&self, address: &str) -> Result<Vec<types::AddressUtxo>, ExplorerError> {
        let url = self.url_builder.get_address_utxo_url(address)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::AddressUtxo>>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        resp.into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<Vec<_>, _>>()
    }

    /// Retrieves scripthash information.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::ElementsHex` if script parsing fails
    pub async fn get_scripthash(&self, hash: &str) -> Result<types::ScripthashInfo, ExplorerError> {
        let url = self.url_builder.get_scripthash_url(hash)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::ScripthashInfo>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    /// Retrieves transactions for a scripthash.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if response text extraction fails
    pub async fn get_scripthash_txs(&self, hash: &str) -> Result<String, ExplorerError> {
        let url = self.url_builder.get_scripthash_txs_url(hash)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        resp.text().await.map_err(|e| ExplorerError::deserialize_reqwest(&e))
    }

    /// Retrieves confirmed transactions for a scripthash with pagination.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if response text extraction fails
    pub async fn get_scripthash_txs_chain(
        &self,
        hash: &str,
        last_seen_txid: Option<&str>,
    ) -> Result<String, ExplorerError> {
        let url = self.url_builder.get_scripthash_txs_chain_url(hash, last_seen_txid)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        resp.text().await.map_err(|e| ExplorerError::deserialize_reqwest(&e))
    }

    /// Retrieves mempool transactions for a scripthash.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if response text extraction fails
    pub async fn get_scripthash_txs_mempool(&self, hash: &str) -> Result<String, ExplorerError> {
        let url = self.url_builder.get_scripthash_txs_mempool_url(hash)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        resp.text().await.map_err(|e| ExplorerError::deserialize_reqwest(&e))
    }

    /// Retrieves UTXOs for a scripthash.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if response text extraction fails
    pub async fn get_scripthash_utxo(&self, hash: &str) -> Result<String, ExplorerError> {
        let url = self.url_builder.get_scripthash_utxo_url(hash)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        resp.text().await.map_err(|e| ExplorerError::deserialize_reqwest(&e))
    }

    /// Retrieves block information by block hash.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if hash/merkle node parsing fails
    pub async fn get_block(&self, hash: &str) -> Result<types::Block, ExplorerError> {
        let url = self.url_builder.get_block_url(hash)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::Block>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    /// Retrieves block header as hex string.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if response text extraction fails
    pub async fn get_block_header(&self, hash: &str) -> Result<String, ExplorerError> {
        let url = self.url_builder.get_block_header_url(hash)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp.text().await.map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        Ok(resp)
    }

    /// Retrieves block status information.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    pub async fn get_block_status(&self, hash: &str) -> Result<types::BlockStatus, ExplorerError> {
        let url = self.url_builder.get_block_status_url(hash)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        resp.json::<types::BlockStatus>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))
    }

    /// Retrieves transactions in a block with optional pagination.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID/block hash parsing fails
    pub async fn get_block_txs(
        &self,
        hash: &str,
        start_index: Option<u32>,
    ) -> Result<Vec<types::EsploraTransaction>, ExplorerError> {
        let url = self.url_builder.get_block_txs_url(hash, start_index)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::EsploraTransaction>>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = resp
            .into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves transaction IDs in a block.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub async fn get_block_txids(&self, hash: &str) -> Result<Vec<Txid>, ExplorerError> {
        let url = self.url_builder.get_block_txids_url(hash)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<String>>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;

        let resp = resp
            .into_iter()
            .map(|val| Txid::from_str(&val))
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves a specific transaction ID from a block.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if response text extraction fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub async fn get_block_txid(&self, hash: &str, index: u32) -> Result<Txid, ExplorerError> {
        let url = self.url_builder.get_block_txid_url(hash, index)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp.text().await.map_err(|e| ExplorerError::deserialize_reqwest(&e))?;

        Ok(Txid::from_str(&resp)?)
    }

    /// Retrieves raw block bytes.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if response bytes extraction fails
    pub async fn get_block_raw(&self, hash: &str) -> Result<Vec<u8>, ExplorerError> {
        let url = self.url_builder.get_block_raw_url(hash)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))
    }

    /// Retrieves block hash by block height.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if response text extraction fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if block hash parsing fails
    pub async fn get_block_height(&self, height: u64) -> Result<BlockHash, ExplorerError> {
        let url = self.url_builder.get_block_height_url(height)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp.text().await.map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = BlockHash::from_str(&resp)?;
        Ok(resp)
    }

    /// Retrieves blocks starting from a given height.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if hash/merkle node parsing fails
    pub async fn get_blocks(&self, start_height: Option<u64>) -> Result<Vec<types::Block>, ExplorerError> {
        let url = self.url_builder.get_blocks_url(start_height)?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::Block>>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = resp
            .into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves the height of the blockchain tip.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    pub async fn get_blocks_tip_height(&self) -> Result<u64, ExplorerError> {
        let url = self.url_builder.get_blocks_tip_height_url()?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<u64>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        Ok(resp)
    }

    /// Retrieves the hash of the blockchain tip.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if response text extraction fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if block hash parsing fails
    pub async fn get_blocks_tip_hash(&self) -> Result<BlockHash, ExplorerError> {
        let url = self.url_builder.get_blocks_tip_hash_url()?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp.text().await.map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = BlockHash::from_str(&resp)?;
        Ok(resp)
    }

    /// Retrieves mempool information.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    pub async fn get_mempool(&self) -> Result<types::MempoolInfo, ExplorerError> {
        let url = self.url_builder.get_mempool_url()?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        resp.json().await.map_err(|e| ExplorerError::deserialize_reqwest(&e))
    }

    /// Retrieves all transaction IDs in the mempool.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub async fn get_mempool_txids(&self) -> Result<Vec<Txid>, ExplorerError> {
        let url = self.url_builder.get_mempool_txids_url()?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<String>>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = resp
            .into_iter()
            .map(|val| Txid::from_str(&val))
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves recent mempool transactions.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub async fn get_mempool_recent(&self) -> Result<Vec<types::MempoolRecent>, ExplorerError> {
        let url = self.url_builder.get_mempool_recent_url()?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::MempoolRecent>>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))?;
        let resp = resp
            .into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves fee estimates.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_reqwest` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_reqwest` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_reqwest` if JSON deserialization fails
    pub async fn get_fee_estimates(&self) -> Result<types::FeeEstimates, ExplorerError> {
        let url = self.url_builder.get_fee_estimates_url()?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ExplorerError::response_failed_reqwest(&e))?;
        Self::filter_resp(&resp)?;

        resp.json::<types::FeeEstimates>()
            .await
            .map_err(|e| ExplorerError::deserialize_reqwest(&e))
    }
}

impl EsploraClientSync {
    #[inline]
    fn filter_resp(resp: &minreq::Response) -> Result<(), ExplorerError> {
        if is_resp_ok(resp.status_code) {
            return Err(ExplorerError::erroneous_response_minreq(resp));
        }
        Ok(())
    }

    /// Retrieves transaction details by transaction ID.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub fn get_tx(&self, txid: &str) -> Result<types::EsploraTransaction, ExplorerError> {
        let url: String = self.url_builder.get_tx_url(txid)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::EsploraTransaction>()
            .map_err(ExplorerError::deserialize_minreq)?;
        let resp = resp.convert()?;

        Ok(resp)
    }

    /// Retrieves transaction status by transaction ID.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if block hash parsing fails
    pub fn get_tx_status(&self, txid: &str) -> Result<types::TxStatus, ExplorerError> {
        let url: String = self.url_builder.get_tx_status_url(txid)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::TxStatus>()
            .map_err(ExplorerError::deserialize_minreq)?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    /// Retrieves transaction hex by transaction ID.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if response text extraction fails
    pub fn get_tx_hex(&self, txid: &str) -> Result<String, ExplorerError> {
        let url: String = self.url_builder.get_tx_hex_url(txid)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        Ok(resp.as_str().map_err(ExplorerError::deserialize_minreq)?.to_string())
    }

    /// Retrieves raw transaction bytes by transaction ID.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    pub fn get_tx_raw(&self, txid: &str) -> Result<Vec<u8>, ExplorerError> {
        let url: String = self.url_builder.get_tx_raw_url(txid)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        Ok(resp.as_bytes().to_vec())
    }

    /// Retrieves and deserializes a transaction as an Elements transaction.
    ///
    /// # Errors
    /// - Returns all errors from `get_tx_raw`
    /// - Returns `ExplorerError::TransactionDecode` if transaction deserialization fails
    pub fn get_tx_elements(&self, txid: &str) -> Result<simplicityhl::elements::Transaction, ExplorerError> {
        let bytes = self.get_tx_raw(txid)?;
        simplicityhl::elements::Transaction::deserialize(&bytes)
            .map_err(|e| ExplorerError::TransactionDecode(e.to_string()))
    }

    /// Retrieves merkle proof for a transaction.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if merkle hash parsing fails
    pub fn get_tx_merkle_proof(&self, txid: &str) -> Result<types::MerkleProof, ExplorerError> {
        let url: String = self.url_builder.get_tx_merkle_proof_url(txid)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::MerkleProof>()
            .map_err(ExplorerError::response_failed_minreq)?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    /// Retrieves outspend information for a specific output.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub fn get_tx_outspend(&self, txid: &str, vout: u32) -> Result<types::Outspend, ExplorerError> {
        let url: String = self.url_builder.get_tx_outspend_url(txid, vout)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::Outspend>()
            .map_err(ExplorerError::response_failed_minreq)?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    /// Retrieves outspend information for all outputs of a transaction.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub fn get_tx_outspends(&self, txid: &str) -> Result<Vec<types::Outspend>, ExplorerError> {
        let url: String = self.url_builder.get_tx_outspends_url(txid)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::Outspend>>()
            .map_err(ExplorerError::response_failed_minreq)?;
        resp.into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<Vec<_>, _>>()
    }

    /// Broadcasts a transaction to the network.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON serialization or response text extraction fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub fn broadcast_tx(&self, tx: &simplicityhl::elements::Transaction) -> Result<Txid, ExplorerError> {
        let tx_hex = simplicityhl::elements::encode::serialize_hex(tx);
        let url: String = self.url_builder.get_broadcast_tx_url()?;
        let resp = minreq::post(url)
            .with_json(&tx_hex)
            .map_err(ExplorerError::deserialize_minreq)?
            .send()
            .map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .as_str()
            .map_err(ExplorerError::response_failed_minreq)?
            .to_string();
        Ok(Txid::from_str(&resp)?)
    }

    /// Broadcasts a package of transactions.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code or JSON parsing fails
    /// - Returns `ExplorerError::deserialize_minreq` if JSON serialization fails
    pub fn broadcast_tx_package(
        &self,
        txs: &[simplicityhl::elements::Transaction],
    ) -> Result<serde_json::Value, ExplorerError> {
        let url: String = self.url_builder.get_broadcast_tx_package_url()?;
        let tx_hexes = txs
            .iter()
            .map(simplicityhl::elements::encode::serialize_hex)
            .collect::<Vec<_>>();

        let resp = minreq::post(url)
            .with_json(&tx_hexes)
            .map_err(ExplorerError::deserialize_minreq)?
            .send()
            .map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        resp.json().map_err(ExplorerError::response_failed_minreq)
    }

    /// Retrieves address information.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::AddressConversion` if address parsing fails
    pub fn get_address(&self, address: &str) -> Result<types::AddressInfo, ExplorerError> {
        let url: String = self.url_builder.get_address_url(address)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::AddressInfo>()
            .map_err(ExplorerError::response_failed_minreq)?;
        let resp = resp.convert()?;

        Ok(resp)
    }

    /// Retrieves all transactions for an address.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID/block hash parsing fails
    pub fn get_address_txs(&self, address: &str) -> Result<Vec<types::EsploraTransaction>, ExplorerError> {
        let url: String = self.url_builder.get_address_txs_url(address)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;
        let resp = resp
            .json::<Vec<deserializable::EsploraTransaction>>()
            .map_err(ExplorerError::response_failed_minreq)?;
        let resp = resp
            .into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves confirmed transactions for an address with pagination.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID/block hash parsing fails
    pub fn get_address_txs_chain(
        &self,
        address: &str,
        last_seen_txid: Option<&str>,
    ) -> Result<Vec<types::EsploraTransaction>, ExplorerError> {
        let url: String = self.url_builder.get_address_txs_chain_url(address, last_seen_txid)?;

        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::EsploraTransaction>>()
            .map_err(ExplorerError::response_failed_minreq)?;
        let resp = resp
            .into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves mempool transactions for an address.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub fn get_address_txs_mempool(&self, address: &str) -> Result<Vec<types::EsploraTransaction>, ExplorerError> {
        let url: String = self.url_builder.get_address_txs_mempool_url(address)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::EsploraTransaction>>()
            .map_err(ExplorerError::response_failed_minreq)?;
        let resp = resp
            .into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves UTXOs for an address.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::HexSimdDecode` if hex decoding fails
    /// - Returns `ExplorerError::CommitmentDecode` if commitment parsing fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID/block hash parsing fails
    pub fn get_address_utxo(&self, address: &str) -> Result<Vec<types::AddressUtxo>, ExplorerError> {
        let url: String = self.url_builder.get_address_utxo_url(address)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::AddressUtxo>>()
            .map_err(ExplorerError::response_failed_minreq)?;
        resp.into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<Vec<_>, _>>()
    }

    /// Retrieves scripthash information.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::ElementsHex` if script parsing fails
    pub fn get_scripthash(&self, hash: &str) -> Result<types::ScripthashInfo, ExplorerError> {
        let url: String = self.url_builder.get_scripthash_url(hash)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::ScripthashInfo>()
            .map_err(ExplorerError::response_failed_minreq)?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    /// Retrieves transactions for a scripthash.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if response text extraction fails
    pub fn get_scripthash_txs(&self, hash: &str) -> Result<String, ExplorerError> {
        let url: String = self.url_builder.get_scripthash_txs_url(hash)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        Ok(resp
            .as_str()
            .map_err(ExplorerError::response_failed_minreq)?
            .to_string())
    }

    /// Retrieves confirmed transactions for a scripthash with pagination.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if response text extraction fails
    pub fn get_scripthash_txs_chain(&self, hash: &str, last_seen_txid: Option<&str>) -> Result<String, ExplorerError> {
        let url: String = self.url_builder.get_scripthash_txs_chain_url(hash, last_seen_txid)?;

        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        Ok(resp
            .as_str()
            .map_err(ExplorerError::response_failed_minreq)?
            .to_string())
    }

    /// Retrieves mempool transactions for a scripthash.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if response text extraction fails
    pub fn get_scripthash_txs_mempool(&self, hash: &str) -> Result<String, ExplorerError> {
        let url: String = self.url_builder.get_scripthash_txs_mempool_url(hash)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        Ok(resp
            .as_str()
            .map_err(ExplorerError::response_failed_minreq)?
            .to_string())
    }

    /// Retrieves UTXOs for a scripthash.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if response text extraction fails
    pub fn get_scripthash_utxo(&self, hash: &str) -> Result<String, ExplorerError> {
        let url: String = self.url_builder.get_scripthash_utxo_url(hash)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        Ok(resp
            .as_str()
            .map_err(ExplorerError::response_failed_minreq)?
            .to_string())
    }

    /// Retrieves block information by block hash.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if hash/merkle node parsing fails
    pub fn get_block(&self, hash: &str) -> Result<types::Block, ExplorerError> {
        let url: String = self.url_builder.get_block_url(hash)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<deserializable::Block>()
            .map_err(ExplorerError::response_failed_minreq)?;
        let resp = resp.convert()?;
        Ok(resp)
    }

    /// Retrieves block header as hex string.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if response text extraction fails
    pub fn get_block_header(&self, hash: &str) -> Result<String, ExplorerError> {
        let url: String = self.url_builder.get_block_header_url(hash)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .as_str()
            .map_err(ExplorerError::response_failed_minreq)?
            .to_string();
        Ok(resp)
    }

    /// Retrieves block status information.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    pub fn get_block_status(&self, hash: &str) -> Result<types::BlockStatus, ExplorerError> {
        let url: String = self.url_builder.get_block_status_url(hash)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        resp.json::<types::BlockStatus>()
            .map_err(ExplorerError::response_failed_minreq)
    }

    /// Retrieves transactions in a block with optional pagination.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID/block hash parsing fails
    pub fn get_block_txs(
        &self,
        hash: &str,
        start_index: Option<u32>,
    ) -> Result<Vec<types::EsploraTransaction>, ExplorerError> {
        let url: String = self.url_builder.get_block_txs_url(hash, start_index)?;

        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::EsploraTransaction>>()
            .map_err(ExplorerError::response_failed_minreq)?;
        let resp = resp
            .into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves transaction IDs in a block.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub fn get_block_txids(&self, hash: &str) -> Result<Vec<Txid>, ExplorerError> {
        let url: String = self.url_builder.get_block_txids_url(hash)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<String>>()
            .map_err(ExplorerError::response_failed_minreq)?;

        let resp = resp
            .into_iter()
            .map(|val| Txid::from_str(&val))
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves a specific transaction ID from a block.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if response text extraction fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub fn get_block_txid(&self, hash: &str, index: u32) -> Result<Txid, ExplorerError> {
        let url: String = self.url_builder.get_block_txid_url(hash, index)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp.as_str().map_err(ExplorerError::response_failed_minreq)?;

        Ok(Txid::from_str(resp)?)
    }

    /// Retrieves raw block bytes.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    pub fn get_block_raw(&self, hash: &str) -> Result<Vec<u8>, ExplorerError> {
        let url: String = self.url_builder.get_block_raw_url(hash)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        Ok(resp.as_bytes().to_vec())
    }

    /// Retrieves block hash by block height.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if response text extraction fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if block hash parsing fails
    pub fn get_block_height(&self, height: u64) -> Result<BlockHash, ExplorerError> {
        let url: String = self.url_builder.get_block_height_url(height)?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp.as_str().map_err(ExplorerError::response_failed_minreq)?;

        let resp = BlockHash::from_str(resp)?;
        Ok(resp)
    }

    /// Retrieves blocks starting from a given height.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if hash/merkle node parsing fails
    pub fn get_blocks(&self, start_height: Option<u64>) -> Result<Vec<types::Block>, ExplorerError> {
        let url = self.url_builder.get_blocks_url(start_height)?;

        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::Block>>()
            .map_err(ExplorerError::response_failed_minreq)?;
        let resp = resp
            .into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves the height of the blockchain tip.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    pub fn get_blocks_tip_height(&self) -> Result<u64, ExplorerError> {
        let url: String = self.url_builder.get_blocks_tip_height_url()?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp.json::<u64>().map_err(ExplorerError::response_failed_minreq)?;
        Ok(resp)
    }

    /// Retrieves the hash of the blockchain tip.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if response text extraction fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if block hash parsing fails
    pub fn get_blocks_tip_hash(&self) -> Result<BlockHash, ExplorerError> {
        let url: String = self.url_builder.get_blocks_tip_hash_url()?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp.as_str().map_err(ExplorerError::response_failed_minreq)?;
        let resp = BlockHash::from_str(resp)?;
        Ok(resp)
    }

    /// Retrieves mempool information.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    pub fn get_mempool(&self) -> Result<types::MempoolInfo, ExplorerError> {
        let url: String = self.url_builder.get_mempool_url()?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        resp.json().map_err(ExplorerError::response_failed_minreq)
    }

    /// Retrieves all transaction IDs in the mempool.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub fn get_mempool_txids(&self) -> Result<Vec<Txid>, ExplorerError> {
        let url: String = self.url_builder.get_mempool_txids_url()?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<String>>()
            .map_err(ExplorerError::response_failed_minreq)?;
        let resp = resp
            .into_iter()
            .map(|val| Txid::from_str(&val))
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves recent mempool transactions.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    /// - Returns `ExplorerError::BitcoinHashesHex` if TXID parsing fails
    pub fn get_mempool_recent(&self) -> Result<Vec<types::MempoolRecent>, ExplorerError> {
        let url: String = self.url_builder.get_mempool_recent_url()?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        let resp = resp
            .json::<Vec<deserializable::MempoolRecent>>()
            .map_err(ExplorerError::response_failed_minreq)?;
        let resp = resp
            .into_iter()
            .map(deserializable::TypeConversion::convert)
            .collect::<Result<_, _>>()?;
        Ok(resp)
    }

    /// Retrieves fee estimates.
    ///
    /// # Errors
    /// - Returns `ExplorerError::response_failed_minreq` if the HTTP request fails
    /// - Returns `ExplorerError::erroneous_response_minreq` if the API returns an error status code
    /// - Returns `ExplorerError::deserialize_minreq` if JSON deserialization fails
    pub fn get_fee_estimates(&self) -> Result<types::FeeEstimates, ExplorerError> {
        let url: String = self.url_builder.get_fee_estimates_url()?;
        let resp = minreq::get(url).send().map_err(ExplorerError::response_failed_minreq)?;
        Self::filter_resp(&resp)?;

        resp.json::<types::FeeEstimates>()
            .map_err(ExplorerError::response_failed_minreq)
    }
}

struct UrlBuilder {
    base_url: String,
}

impl UrlBuilder {
    fn get_tx_url(&self, txid: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("/tx/{txid}"))
    }

    fn get_tx_status_url(&self, txid: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("tx/{txid}/status"))
    }

    fn get_tx_hex_url(&self, txid: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("tx/{txid}/hex"))
    }

    fn get_tx_raw_url(&self, txid: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("tx/{txid}/raw"))
    }

    fn get_tx_merkle_proof_url(&self, txid: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("tx/{txid}/merkle-proof"))
    }

    fn get_tx_outspend_url(&self, txid: &str, vout: u32) -> Result<String, ExplorerError> {
        self.join_url(format!("tx/{txid}/outspend/{vout}"))
    }

    fn get_tx_outspends_url(&self, txid: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("tx/{txid}/outspends"))
    }

    fn get_broadcast_tx_url(&self) -> Result<String, ExplorerError> {
        self.join_url("tx")
    }

    fn get_broadcast_tx_package_url(&self) -> Result<String, ExplorerError> {
        self.join_url("txs/package")
    }

    fn get_address_url(&self, address: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("address/{address}"))
    }

    fn get_address_txs_url(&self, address: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("address/{address}/txs"))
    }

    fn get_address_txs_chain_url(&self, address: &str, last_seen_txid: Option<&str>) -> Result<String, ExplorerError> {
        if let Some(txid) = last_seen_txid {
            self.join_url(format!("address/{address}/txs/chain/{txid}"))
        } else {
            self.join_url(format!("address/{address}/txs/chain"))
        }
    }

    fn get_address_txs_mempool_url(&self, address: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("address/{address}/txs/mempool"))
    }

    fn get_address_utxo_url(&self, address: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("address/{address}/utxo"))
    }

    fn get_scripthash_url(&self, hash: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("scripthash/{hash}"))
    }

    fn get_scripthash_txs_url(&self, hash: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("scripthash/{hash}/txs"))
    }

    fn get_scripthash_txs_chain_url(&self, hash: &str, last_seen_txid: Option<&str>) -> Result<String, ExplorerError> {
        if let Some(txid) = last_seen_txid {
            self.join_url(format!("scripthash/{hash}/txs/chain/{txid}"))
        } else {
            self.join_url(format!("scripthash/{hash}/txs/chain"))
        }
    }

    fn get_scripthash_txs_mempool_url(&self, hash: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("scripthash/{hash}/txs/mempool"))
    }

    fn get_scripthash_utxo_url(&self, hash: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("scripthash/{hash}/utxo"))
    }

    fn get_block_url(&self, hash: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("block/{hash}"))
    }

    fn get_block_header_url(&self, hash: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("block/{hash}/header"))
    }

    fn get_block_status_url(&self, hash: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("block/{hash}/status"))
    }

    fn get_block_txs_url(&self, hash: &str, start_index: Option<u32>) -> Result<String, ExplorerError> {
        if let Some(index) = start_index {
            self.join_url(format!("block/{hash}/txs/{index}"))
        } else {
            self.join_url(format!("block/{hash}/txs"))
        }
    }

    fn get_block_txids_url(&self, hash: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("block/{hash}/txids"))
    }

    fn get_block_txid_url(&self, hash: &str, index: u32) -> Result<String, ExplorerError> {
        self.join_url(format!("block/{hash}/txid/{index}"))
    }

    fn get_block_raw_url(&self, hash: &str) -> Result<String, ExplorerError> {
        self.join_url(format!("block/{hash}/raw"))
    }

    fn get_block_height_url(&self, height: u64) -> Result<String, ExplorerError> {
        self.join_url(format!("block-height/{height}"))
    }

    fn get_blocks_url(&self, start_height: Option<u64>) -> Result<String, ExplorerError> {
        if let Some(height) = start_height {
            self.join_url(format!("blocks/{height}"))
        } else {
            self.join_url("blocks")
        }
    }

    fn get_blocks_tip_height_url(&self) -> Result<String, ExplorerError> {
        self.join_url("blocks/tip/height")
    }

    fn get_blocks_tip_hash_url(&self) -> Result<String, ExplorerError> {
        self.join_url("blocks/tip/hash")
    }

    fn get_mempool_url(&self) -> Result<String, ExplorerError> {
        self.join_url("mempool")
    }

    fn get_mempool_txids_url(&self) -> Result<String, ExplorerError> {
        self.join_url("mempool/txids")
    }

    fn get_mempool_recent_url(&self) -> Result<String, ExplorerError> {
        self.join_url("mempool/recent")
    }

    fn get_fee_estimates_url(&self) -> Result<String, ExplorerError> {
        self.join_url("fee-estimates")
    }
}

trait BaseUrlGetter {
    fn get_base_url(&self) -> &str;
}

trait UrlAppender {
    fn join_url(&self, str: impl AsRef<str>) -> Result<String, ExplorerError>;
}

impl<T: BaseUrlGetter> UrlAppender for T {
    #[inline]
    fn join_url(&self, str: impl AsRef<str>) -> Result<String, ExplorerError> {
        Ok(format!("{}/{}", self.get_base_url(), str.as_ref()))
    }
}

impl BaseUrlGetter for UrlBuilder {
    fn get_base_url(&self) -> &str {
        self.base_url.as_str()
    }
}

impl BaseUrlGetter for EsploraClientAsync {
    fn get_base_url(&self) -> &str {
        self.url_builder.get_base_url()
    }
}

impl BaseUrlGetter for EsploraClientSync {
    fn get_base_url(&self) -> &str {
        self.url_builder.get_base_url()
    }
}

fn is_resp_ok(code: i32) -> bool {
    !(200..300).contains(&code)
}
