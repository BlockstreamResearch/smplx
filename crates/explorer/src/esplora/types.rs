use serde::Deserialize;
use simplicityhl::elements::{AssetId, BlockHash, OutPoint, Script, TxMerkleNode, Txid};
use std::collections::HashMap;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct EsploraTransaction {
    pub txid: Txid,
    pub version: u32,
    pub locktime: u32,
    pub size: u64,
    pub weight: u64,
    pub fee: u64,
    pub vin: Vec<Vin>,
    pub vout: Vec<Vout>,
    pub status: TxStatus,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Vin {
    pub out_point: OutPoint,
    pub is_coinbase: bool,
    pub scriptsig: String,
    pub scriptsig_asm: String,
    pub inner_redeemscript_asm: Option<String>,
    pub inner_witnessscript_asm: Option<String>,
    pub sequence: u32,
    pub witness: Vec<String>,
    pub prevout: Option<Vout>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Vout {
    pub scriptpubkey: Script,
    pub scriptpubkey_asm: String,
    pub scriptpubkey_type: String,
    pub scriptpubkey_address: Option<String>,
    pub value: Option<u64>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct TxStatus {
    pub confirmed: bool,
    pub block_height: Option<u64>,
    pub block_hash: Option<simplicityhl::elements::BlockHash>,
    pub block_time: Option<u64>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Outspend {
    pub spent: bool,
    pub txid: Option<Txid>,
    pub vin: Option<u32>,
    pub status: Option<TxStatus>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct AddressInfo {
    pub address: simplicityhl::elements::Address,
    pub chain_stats: ChainStats,
    pub mempool_stats: MempoolStats,
}

#[derive(Debug, Clone, Deserialize, Hash, Eq, PartialEq)]
pub struct ScripthashInfo {
    pub scripthash: String,
    pub chain_stats: Stats,
    pub mempool_stats: Stats,
}

pub type MempoolStats = ChainStats;

#[derive(Debug, Clone, Deserialize, Hash, Eq, PartialEq)]
pub struct ChainStats {
    pub funded_txo_count: u64,
    pub spent_txo_count: u64,
    pub tx_count: u64,
}

#[derive(Debug, Clone, Deserialize, Hash, Eq, PartialEq)]
pub struct Stats {
    pub tx_count: u64,
    pub funded_txo_count: u64,
    pub funded_txo_sum: u64,
    pub spent_txo_count: u64,
    pub spent_txo_sum: u64,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Utxo {
    pub outpoint: OutPoint,
    pub value: u64,
    pub status: TxStatus,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct AddressUtxo {
    pub outpoint: OutPoint,
    pub status: TxStatus,
    pub utxo_info: UtxoInfo,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum UtxoInfo {
    Confidential {
        value_comm: simplicityhl::elements::confidential::Value,
        asset_comm: simplicityhl::elements::confidential::Asset,
        nonce_comm: simplicityhl::elements::confidential::Nonce,
    },
    Explicit {
        value: u64,
        asset: AssetId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub id: String,
    pub height: u64,
    pub version: u32,
    pub timestamp: u64,
    pub mediantime: u64,
    pub bits: u32,
    pub nonce: u32,
    pub merkle_root: TxMerkleNode,
    pub tx_count: u64,
    pub size: u64,
    pub weight: u64,
    pub previousblockhash: BlockHash,
    pub difficulty: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Hash, Eq, PartialEq)]
pub struct BlockStatus {
    pub in_best_chain: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_best: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MempoolInfo {
    pub count: u64,
    pub vsize: u64,
    pub total_fee: u64,
    pub fee_histogram: Vec<(f64, u64)>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct MempoolRecent {
    pub txid: Txid,
    pub fee: u64,
    pub vsize: u64,
    pub discount_vsize: u64,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct MerkleProof {
    pub block_height: u64,
    pub merkle: Vec<TxMerkleNode>,
    pub pos: u64,
}

pub type FeeEstimates = HashMap<String, f64>;
