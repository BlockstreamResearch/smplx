// pub mod serde_hex {
//     use bitcoin_hashes::hex::{DisplayHex, FromHex};
//     use serde::de::Error;
//     use serde::{Deserializer, Serializer};
//
//     pub fn serialize<S: Serializer>(b: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
//         s.serialize_str(&b.to_lower_hex_string())
//     }
//
//     pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
//         let hex_str: String = ::serde::Deserialize::deserialize(d)?;
//         Ok(FromHex::from_hex(&hex_str).map_err(D::Error::custom)?)
//     }
//
//     pub mod opt {
//         use serde::de::Error;
//         use serde::{Deserializer, Serializer};
//         use simplicityhl::elements::hex::FromHex;
//         use simplicityhl::simplicity::hex::DisplayHex;
//
//         pub fn serialize<S: Serializer>(b: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
//             match *b {
//                 None => s.serialize_none(),
//                 Some(ref b) => s.serialize_str(&b.to_lower_hex_string()),
//             }
//         }
//
//         pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<u8>>, D::Error> {
//             let hex_str: String = ::serde::Deserialize::deserialize(d)?;
//             Ok(Some(FromHex::from_hex(&hex_str).map_err(D::Error::custom)?))
//         }
//     }
// }

// #[derive(Clone, Debug, serde::Deserialize)]
// pub struct GetBlockchainInfoResult {
//     /// Current network name as defined in BIP70 (main, test, signet, regtest)
//     pub chain: types::Network,
//     /// The current number of blocks processed in the server
//     pub blocks: u64,
//     /// The current number of headers we have validated
//     pub headers: u64,
//     /// The hash of the currently best block
//     #[serde(rename = "bestblockhash")]
//     pub best_block_hash: bitcoin::BlockHash,
//     /// The current difficulty
//     pub difficulty: f64,
//     /// Median time for the current best block
//     #[serde(rename = "mediantime")]
//     pub median_time: u64,
//     /// Estimate of verification progress [0..1]
//     #[serde(rename = "verificationprogress")]
//     pub verification_progress: f64,
//     /// Estimate of whether this node is in Initial Block Download mode
//     #[serde(rename = "initialblockdownload")]
//     pub initial_block_download: bool,
//     /// Total amount of work in active chain, in hexadecimal
//     #[serde(rename = "chainwork", with = "serde_hex")]
//     pub chain_work: Vec<u8>,
//     /// The estimated size of the block and undo files on disk
//     pub size_on_disk: u64,
//     /// If the blocks are subject to pruning
//     pub pruned: bool,
//     /// Lowest-height complete block stored (only present if pruning is enabled)
//     #[serde(rename = "pruneheight")]
//     pub prune_height: Option<u64>,
//     /// Whether automatic pruning is enabled (only present if pruning is enabled)
//     pub automatic_pruning: Option<bool>,
//     /// The target size used by pruning (only present if automatic pruning is enabled)
//     pub prune_target_size: Option<u64>,
//     /// Status of softforks in progress
//     #[serde(default)]
//     pub softforks: HashMap<String, Softfork>,
//     /// Any network and blockchain warnings. In later versions of bitcoind, it's an array of strings.
//     pub warnings: StringOrStringArray,
// }