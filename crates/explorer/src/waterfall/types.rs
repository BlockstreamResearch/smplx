use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterfallResponse {
    pub txs_seen: HashMap<String, Vec<TxSeen>>,
    pub page: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterfallResponseV4 {
    pub txs_seen: HashMap<String, Vec<TxSeen>>,
    pub page: u32,
    pub tip_meta: TipMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxSeen {
    pub txid: String,
    pub height: u64,
    pub block_hash: String,
    pub block_timestamp: u64,
    pub v: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TipMeta {
    pub b: String, // block hash
    pub t: u64,    // timestamp
    pub h: u64,    // height
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastUsedIndex {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildInfo {
    pub version: String,
    pub git_commit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressTxs {
    pub txid: String,
    pub status: AddressTxStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressTxStatus {
    pub block_height: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<String>,
}
