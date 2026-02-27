use serde::ser::Error;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GetBlockchainInfo {
    pub chain: String,
    pub blocks: u64,
    pub headers: u64,
    pub bestblockhash: String,
    // pub difficulty: f64,
    pub time: u64,
    pub mediantime: u64,
    pub verificationprogress: f64,
    pub initialblockdownload: bool,
    // pub chainwork: String,
    pub size_on_disk: u64,
    pub pruned: bool,

    // Elements specific fields
    pub current_params_root: String,
    // pub signblock_asm: String,
    // pub signblock_hex: String,
    pub current_signblock_asm: String,
    pub current_signblock_hex: String,
    pub max_block_witness: u64,
    pub epoch_length: u64,
    pub total_valid_epochs: u64,
    pub epoch_age: u64,

    // Using Value here as the documentation describes it generically as "extension fields"
    pub extension_space: Vec<Value>,

    // Optional pruning fields (only present if pruning is enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pruneheight: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automatic_pruning: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prune_target_size: Option<u64>,

    // Softforks are deprecated but might still be present if configured
    #[serde(skip_serializing_if = "Option::is_none")]
    pub softforks: Option<HashMap<String, Softfork>>,

    pub warnings: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Softfork {
    #[serde(rename = "type")]
    pub fork_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u64>,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bip9: Option<SoftforkBip9>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SoftforkBip9 {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit: Option<u8>,
    pub start_time: u64,
    pub timeout: u64,
    pub min_activation_height: u64,
    pub status: String,
    pub since: u64,
    pub status_next: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<SoftforkStatistics>,
    pub signalling: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SoftforkStatistics {
    pub period: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<u64>,
    pub elapsed: u64,
    pub count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub possible: Option<bool>,
}

pub struct WalletMeta {
    pub name: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GetBalance {
    #[serde(rename = "mine")]
    pub mine: BalanceDetails,
    #[serde(rename = "watchonly")]
    pub watchonly: Option<BalanceDetails>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BalanceDetails {
    pub trusted: f64,
    pub untrusted_pending: f64,
    pub immature: f64,
}

#[derive(Default, Debug, Clone, Copy)]
pub enum AddressType {
    Legacy,
    #[default]
    P2shSegwit,
    Bech32,
    Bech32m,
}

impl std::fmt::Display for AddressType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            AddressType::Legacy => "legacy".to_string(),
            AddressType::P2shSegwit => "p2sh-segwit".to_string(),
            AddressType::Bech32 => "bech32".to_string(),
            AddressType::Bech32m => "bech32m".to_string(),
        };
        write!(f, "{str}")
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ListUnspent {
    pub txid: String,
    pub vout: u32,
    pub address: String,
    pub label: Option<String>,
    #[serde(rename = "scriptPubKey")]
    pub script_pubkey: String,
    pub amount: f64,
    pub amountcommitment: Option<String>,
    pub asset: Option<String>,
    pub assetcommitment: Option<String>,
    pub confirmations: u64,
    pub bcconfirmations: Option<u64>,
    pub spendable: bool,
    pub solvable: bool,
    pub safe: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct QueryOptions {
    #[serde(rename = "minimumAmount")]
    pub minimum_amount: Option<f64>,
    #[serde(rename = "maximumAmount")]
    pub maximum_amount: Option<f64>,
    #[serde(rename = "maximumCount")]
    pub maximum_count: Option<u64>,
    #[serde(rename = "minimumSumAmount")]
    pub minimum_sum_amount: Option<f64>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ScantxoutsetResult {
    Start {
        bestblock: String,
        height: u64,
        success: bool,
        total_unblinded_bitcoin_amount: f64,
        txouts: u32,
        unspents: Vec<ScantxoutsetUtxo>,
    },
    Abort {
        success: bool,
    },
    Status {
        progress: f64,
        searched_items: Option<u64>,
    },
}

impl ScantxoutsetResult {
    /// Parse the RPC response based on the action that was sent
    pub fn from_value(value: serde_json::Value, action: &str) -> Result<Self, serde_json::Error> {
        match action {
            "start" => serde_json::from_value(value).map(|start_data: StartData| ScantxoutsetResult::Start {
                bestblock: start_data.bestblock,
                height: start_data.height,
                success: start_data.success,
                total_unblinded_bitcoin_amount: start_data.total_unblinded_bitcoin_amount,
                txouts: start_data.txouts,
                unspents: start_data.unspents,
            }),
            "abort" => serde_json::from_value(value).map(|abort_data: AbortData| ScantxoutsetResult::Abort {
                success: abort_data.success,
            }),
            "status" => serde_json::from_value(value).map(|status_data: StatusData| ScantxoutsetResult::Status {
                progress: status_data.progress,
                searched_items: status_data.searched_items,
            }),
            _ => Err(serde_json::Error::custom(format!("unknown action: {action}"))),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct StartData {
    bestblock: String,
    height: u64,
    success: bool,
    total_unblinded_bitcoin_amount: f64,
    txouts: u32,
    unspents: Vec<ScantxoutsetUtxo>,
}

#[derive(Debug, serde::Deserialize)]
struct AbortData {
    success: bool,
}

#[derive(Debug, serde::Deserialize)]
struct StatusData {
    progress: f64,
    searched_items: Option<u64>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ScantxoutsetUtxo {
    pub amount: f64,
    pub asset: String,
    pub desc: String,
    pub height: u64,
    #[serde(rename = "scriptPubKey")]
    pub scriptpubkey: String,
    pub txid: String,
    pub vout: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetTransaction {
    pub amount: f64,
    pub fee: Option<f64>,
    pub confirmations: i32,
    pub blockhash: Option<String>,
    pub blockindex: Option<u32>,
    pub blocktime: Option<u64>,
    pub txid: String,
    pub time: u64,
    pub timereceived: u64,
    #[serde(default)]
    pub bip125_replaceable: String,
    pub details: Vec<TransactionDetail>,
    pub hex: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransactionDetail {
    pub involveswatchonly: Option<bool>,
    pub address: Option<String>,
    pub category: String,
    pub amount: f64,
    pub label: Option<String>,
    pub vout: u32,
    #[serde(default)]
    pub fee: Option<f64>,
    pub abandoned: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SendRawTransaction {
    pub txid: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetRawTransaction {
    pub in_active_chain: Option<bool>,
    pub hex: String,
    pub txid: String,
    pub hash: String,
    pub size: u32,
    pub vsize: u32,
    pub weight: u32,
    pub version: u32,
    pub locktime: u32,
    pub vin: Vec<RawTransactionInput>,
    pub vout: Vec<RawTransactionOutput>,
    pub blockhash: Option<String>,
    pub confirmations: Option<u32>,
    pub time: Option<u64>,
    pub blocktime: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RawTransactionInput {
    pub txid: String,
    pub vout: u32,
    #[serde(rename = "scriptSig")]
    pub script_sig: ScriptSig,
    #[serde(default)]
    pub txinwitness: Vec<String>,
    pub sequence: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScriptSig {
    pub asm: String,
    pub hex: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RawTransactionOutput {
    pub value: f64,
    pub n: u32,
    #[serde(rename = "scriptPubKey")]
    pub script_pubkey: ScriptPubKey,
    #[serde(default)]
    pub asset: Option<String>,
    #[serde(default)]
    pub assetcommitment: Option<String>,
    #[serde(default)]
    pub valuecommitment: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScriptPubKey {
    pub asm: String,
    pub hex: String,
    #[serde(rename = "reqSigs")]
    pub req_sigs: Option<u32>,
    #[serde(rename = "type")]
    pub script_type: String,
    pub addresses: Option<Vec<String>>,
}
