use std::collections::BTreeMap;
use std::str::FromStr;

use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use simplicityhl::elements::{Address, AssetId, Txid};
use simplicityhl::simplicity::bitcoin;

use super::error::RpcError;

use crate::utils::sat2btc;

/// A lightweight wrapper around the standard `bitcoincore_rpc` `Client` providing Elements-specific functionality.
#[derive(Debug)]
pub struct ElementsRpc {
    /// The underlying JSON-RPC client connected to the Elements node.
    pub inner: Client,
    /// The authentication credentials used.
    pub auth: Auth,
    /// The URL endpoint of the node.
    pub url: String,
}

// TODO: insert corepc deps
/// Result of JSON-RPC method `getblockchaininfo`.
///
/// > getblockchaininfo
/// >
/// > Returns an object containing various state info regarding blockchain processing.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct GetBlockchainInfo {
    /// Current network name (main, test, signet, regtest, liquidv1, liquidv1test, liquidtestnet).
    pub chain: String,
    /// The height of the most-work fully-validated chain. The genesis block has height 0.
    pub blocks: i64,
    /// The current number of headers we have validated.
    pub headers: i64,
    /// The hash of the currently best block.
    #[serde(rename = "bestblockhash")]
    pub best_block_hash: String,
    /// The current difficulty.
    pub difficulty: Option<i64>,
    /// The block time expressed in UNIX epoch time.
    pub time: i64,
    /// The median block time expressed in UNIX epoch time.
    #[serde(rename = "mediantime")]
    pub median_time: i64,
    /// Estimate of verification progress [0..1].
    #[serde(rename = "verificationprogress")]
    pub verification_progress: f64,
    /// (Debug information) estimate of whether this node is in Initial Block Download mode.
    #[serde(rename = "initialblockdownload")]
    pub initial_block_download: bool,
    /// Total amount of work in active chain, in hexadecimal.
    #[serde(rename = "chainwork")]
    pub chain_work: Option<String>,
    /// The estimated size of the block and undo files on disk.
    pub size_on_disk: i64,
    /// If the blocks are subject to pruning.
    pub pruned: bool,
    /// Whether header trimming is enabled (-trim-headers).
    pub trim_headers: bool,
    /// The root of the currently active dynafed params.
    pub current_params_root: Option<String>,
    /// The fedpeg program enforced on the next block.
    pub current_fedpeg_program: Option<String>,
    /// The fedpeg script enforced on the next block.
    pub current_fedpeg_script: Option<String>,
    /// ASM of sign block challenge data enforced on the next block.
    #[serde(rename = "current_signblock_asm")]
    pub current_sign_block_asm: Option<String>,
    /// Hex of sign block challenge data enforced on the next block.
    #[serde(rename = "current_signblock_hex")]
    pub current_sign_block_hex: Option<String>,
    /// Maximum sized block witness serialized size for the next block.
    pub max_block_witness: Option<i64>,
    /// Length of dynamic federations epoch, or signaling period.
    pub epoch_length: Option<i64>,
    /// Number of epochs a given fedpscript is valid for, defined per chain.
    pub total_valid_epochs: Option<i64>,
    /// Number of blocks into a dynamic federation epoch chain tip is. This number is between 0 to epoch_length-1.
    pub epoch_age: Option<i64>,
    /// Array of extension fields in dynamic blockheader.
    pub extension_space: Option<Vec<String>>,
    /// Lowest-height complete block stored (only present if pruning is enabled).
    #[serde(rename = "pruneheight")]
    pub prune_height: Option<i64>,
    /// Whether automatic pruning is enabled (only present if pruning is enabled).
    pub automatic_pruning: Option<bool>,
    /// The target size used by pruning (only present if automatic pruning is enabled).
    pub prune_target_size: Option<i64>,
    /// Status of softforks in progress, maps softfork name -> [`Softfork`].
    pub softforks: Option<BTreeMap<String, Softfork>>,
    /// Any network and blockchain warnings.
    pub warnings: String,
}

/// Softfork status. Part of `getblockchaininfo`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Softfork {
    /// The [`corepc_elements_types::v23_3_1::SoftforkType`]: one of "buried", "bip9".
    #[serde(rename = "type")]
    pub type_: SoftforkType,
    ///  Height of the first block which the rules are or will be enforced (only for "buried" type, or "bip9" type with "active" status).
    pub height: Option<i64>,
    /// `true` if the rules are enforced for the mempool and the next block.
    pub active: bool,
    /// The status of bip9 softforks (only for "bip9" type).
    pub bip9: Option<Bip9SoftforkInfo>,
}

/// The softfork type. Part of `getblockchaininfo`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SoftforkType {
    /// Softfork is "buried" (as defined in [BIP-90]).
    ///
    /// [BIP-90] <https://github.com/bitcoin/bips/blob/master/bip-0090.mediawiki>
    Buried,
    /// Softfork is "bip9" (see [BIP-9]).
    ///
    /// [BIP-9] <https://github.com/bitcoin/bips/blob/master/bip-0009.mediawiki>
    Bip9,
}

/// BIP-9 softfork info. Part of `getblockchaininfo`.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize, Serialize)]
pub struct Bip9SoftforkInfo {
    /// One of "defined", "started", "`locked_in`", "active", "failed".
    pub status: Bip9SoftforkStatus,
    /// Status of deployment at the next block.
    pub status_next: Bip9SoftforkStatus,
    /// The bit (0-28) in the block version field used to signal this softfork (only for "started" status).
    pub bit: Option<u8>,
    /// The minimum median time past of a block at which the bit gains its meaning.
    pub start_time: i64,
    /// The median time past of a block at which the deployment is considered failed if not yet locked in.
    pub timeout: i64,
    /// Height of the first block to which the status applies.
    pub since: i64,
    /// Numeric statistics about BIP-9 signalling for a softfork (only for "started" status).
    pub statistics: Option<Bip9SoftforkStatistics>,
    /// Minimum height of blocks for which the rules may be enforced.
    pub min_activation_height: Option<i64>,
}

/// BIP-9 softfork status. Part of `getblockchaininfo`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Bip9SoftforkStatus {
    /// BIP-9 softfork status "defined".
    Defined,
    /// BIP-9 softfork status "started".
    Started,
    /// BIP-9 softfork status "`locked_in`".
    LockedIn,
    /// BIP-9 softfork status "active".
    Active,
    /// BIP-9 softfork status "failed".
    Failed,
}

/// BIP-9 softfork statistics. Part of `getblockchaininfo`.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize, Serialize)]
pub struct Bip9SoftforkStatistics {
    /// The length in blocks of the BIP9 signalling period.
    pub period: i64,
    /// The number of blocks with the version bit set required to activate the feature.
    pub threshold: Option<i64>,
    /// The number of blocks elapsed since the beginning of the current period.
    pub elapsed: i64,
    /// The number of blocks with the version bit set in the current period.
    pub count: i64,
    /// `false` if there are not enough blocks left in this period to pass activation threshold.
    pub possible: Option<bool>,
}

impl ElementsRpc {
    /// Creates a new `ElementsRpc` client.
    ///
    /// # Errors
    /// Returns an `RpcError` if it fails to initialize the connection or if a liveness `ping` fails.
    pub fn new(url: String, auth: Auth) -> Result<Self, RpcError> {
        let inner = Client::new(url.as_str(), auth.clone())?;
        inner.ping()?;

        Ok(Self { inner, auth, url })
    }

    /// Requests a new wallet address from the node, mapped to the provided label.
    ///
    /// # Errors
    /// Returns an `RpcError` if the node fails to generate an address or yields an invalid string payload.
    ///
    /// # Panics
    /// Panics if the returned JSON value is not a string, or if the string cannot be parsed into a valid `Address`.
    pub fn get_new_address(&self, label: &str) -> Result<Address, RpcError> {
        const METHOD: &str = "getnewaddress";

        let addr: Value = self.inner.call(METHOD, &[label.into(), "bech32".to_string().into()])?;

        Ok(Address::from_str(addr.as_str().unwrap()).unwrap())
    }

    /// Instructs the node to transfer funds directly to the target address.
    ///
    /// # Errors
    /// Returns an `RpcError` if the node returns an error or returns an invalid `Txid` payload.
    ///
    /// # Panics
    /// Panics if the satoshi amount cannot be converted to a valid BTC amount, if the returned
    ///  JSON value is not a string, or if the string cannot be parsed into a valid `Txid`.
    pub fn send_to_address(&self, address: &Address, satoshi: u64, asset: Option<AssetId>) -> Result<Txid, RpcError> {
        const METHOD: &str = "sendtoaddress";

        let btc = sat2btc(satoshi);
        let btc = bitcoin::amount::Amount::from_btc(btc)
            .unwrap()
            .to_string_in(bitcoin::amount::Denomination::Bitcoin);

        let r = match asset {
            Some(asset) => self.inner.call::<Value>(
                METHOD,
                &[
                    address.to_string().into(),
                    btc.into(),
                    "".into(),
                    "".into(),
                    false.into(),
                    false.into(),
                    1.into(),
                    "UNSET".into(),
                    false.into(),
                    asset.to_string().into(),
                ],
            )?,
            None => self
                .inner
                .call::<Value>(METHOD, &[address.to_string().into(), btc.into()])?,
        };

        Ok(Txid::from_str(r.as_str().unwrap()).unwrap())
    }

    /// Instructs the node to rescan the block chain for missed wallet transactions.
    ///
    /// # Errors
    /// Returns an `RpcError` if the node fails the RPC call.
    pub fn rescan_blockchain(&self, start: Option<u64>, stop: Option<u64>) -> Result<(), RpcError> {
        const METHOD: &str = "rescanblockchain";

        let mut args = Vec::with_capacity(2);

        if start.is_some() {
            args.push(start.into());
        }

        if stop.is_some() {
            args.push(stop.into());
        }

        self.inner.call::<Value>(METHOD, &args)?;

        Ok(())
    }

    /// Mines a specified number of new blocks mapping generated rewards to a newly generated wallet address.
    ///
    /// # Errors
    /// Returns an `RpcError` if generating the address or calling the `generatetoaddress` RPC command fails.
    pub fn generate_blocks(&self, block_num: u64) -> Result<(), RpcError> {
        const METHOD: &str = "generatetoaddress";

        let address = self.get_new_address("")?.to_string();
        self.inner.call::<Value>(METHOD, &[block_num.into(), address.into()])?;

        Ok(())
    }

    /// Instructs the node to sweep the `initialfreecoins` balance generated by standard regtest genesis blocks.
    ///
    /// # Errors
    /// Returns an `RpcError` if the `getnewaddress` or `sendtoaddress` RPC commands fail.
    pub fn sweep_initialfreecoins(&self) -> Result<(), RpcError> {
        const METHOD: &str = "sendtoaddress";

        let address = self.get_new_address("")?;
        self.inner.call::<Value>(
            METHOD,
            &[
                address.to_string().into(),
                "21".into(),
                "".into(),
                "".into(),
                true.into(),
            ],
        )?;

        Ok(())
    }

    /// Retrieves the current block chain tip height.
    ///
    /// # Errors
    /// Returns an `RpcError` if the node call fails or yields a JSON structure that does not map successfully to a `u64`.
    pub fn height(&self) -> Result<u64, RpcError> {
        const METHOD: &str = "getblockcount";

        self.inner
            .call::<serde_json::Value>(METHOD, &[])?
            .as_u64()
            .ok_or_else(|| RpcError::ElementsRpcUnexpectedReturn(METHOD.into()))
    }

    /// Retrieves state about the current block chain.
    ///
    /// # Errors
    /// Returns an [`RpcError`] if the node call fails or its response cannot be deserialized.
    pub fn get_blockchain_info(&self) -> Result<GetBlockchainInfo, RpcError> {
        const METHOD: &str = "getblockchaininfo";

        Ok(self.inner.call::<GetBlockchainInfo>(METHOD, &[])?)
    }

    /// Overrides the node's clock with a UNIX timestamp. A value of zero restores the system clock.
    ///
    /// # Errors
    /// Returns an [`RpcError`] if the node rejects the regtest-only `setmocktime` RPC call.
    #[cfg(feature = "regtest-utils")]
    pub fn set_mock_time(&self, unix_timestamp: u64) -> Result<(), RpcError> {
        const METHOD: &str = "setmocktime";

        Ok(self.inner.call(METHOD, &[unix_timestamp.into()])?)
    }
}
