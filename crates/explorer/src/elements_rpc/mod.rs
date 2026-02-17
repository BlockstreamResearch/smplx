use crate::elements_rpc::types::AddressType;
use crate::error::ExplorerError;
use bitcoind::bitcoincore_rpc::{Auth, Client, RpcApi, bitcoin};
use electrsd::bitcoind;
use serde_json::Value;
use simplex_core::SimplicityNetwork;
use simplicityhl::elements::{Address, AssetId, BlockHash, Txid};
use std::collections::HashMap;
use std::str::FromStr;

mod types;

pub struct ElementsRpcClient {
    inner: Client,
    #[allow(unused)]
    network: SimplicityNetwork,
    #[allow(unused)]
    auth: Auth,
    #[allow(unused)]
    url: String,
}

impl ElementsRpcClient {
    pub fn new(network: SimplicityNetwork, url: &str, auth: Auth) -> Result<Self, ExplorerError> {
        let inner = Client::new(url, auth.clone())?;
        inner.ping()?;
        Ok(Self {
            inner,
            network,
            auth,
            url: url.to_string(),
        })
    }

    pub fn new_from_credentials(
        network: SimplicityNetwork,
        url: &str,
        user: &str,
        pass: &str,
    ) -> Result<Self, ExplorerError> {
        let auth = Auth::UserPass(user.to_string(), pass.to_string());
        Self::new(network, url, auth)
    }

    pub fn client(&self) -> &Client {
        &self.inner
    }

    pub fn network(&self) -> SimplicityNetwork {
        self.network
    }
}

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

impl ElementsRpcClient {
    pub fn height(client: &Client) -> Result<u64, ExplorerError> {
        const METHOD: &str = "getblockcount";

        client
            .call::<serde_json::Value>(METHOD, &[])?
            .as_u64()
            .ok_or_else(|| ExplorerError::ElementsRpcUnexpectedReturn(METHOD.into()))
    }

    pub fn blockchain_info(client: &Client) -> Result<GetBlockchainInfo, ExplorerError> {
        const METHOD: &str = "getblockchaininfo";

        Ok(client.call::<GetBlockchainInfo>(METHOD, &[])?)
    }

    pub fn sendtoaddress(
        client: &Client,
        address: &Address,
        satoshi: u64,
        asset: Option<AssetId>,
    ) -> Result<Txid, ExplorerError> {
        const METHOD: &str = "sendtoaddress";

        let btc = sat2btc(satoshi);
        let r = match asset {
            Some(asset) => client.call::<Value>(
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
            None => client.call::<Value>(METHOD, &[address.to_string().into(), btc.into()])?,
        };
        Ok(Txid::from_str(r.as_str().unwrap()).unwrap())
    }

    fn rescanblockchain(client: &Client, start: Option<u64>, stop: Option<u64>) -> Result<(), ExplorerError> {
        const METHOD: &str = "rescanblockchain";

        let mut args = Vec::with_capacity(2);
        if start.is_some() {
            args.push(start.into())
        }
        if stop.is_some() {
            args.push(stop.into())
        }
        client.call::<Value>(METHOD, &args)?;
        Ok(())
    }

    fn elementsd_getnewaddress(client: &Client, label: &str, kind: AddressType) -> Result<Address, ExplorerError> {
        const METHOD: &str = "getnewaddress";

        let addr: Value = client.call(METHOD, &[label.into(), kind.to_string().into()])?;
        Ok(Address::from_str(addr.as_str().unwrap()).unwrap())
    }

    fn generate(client: &Client, block_num: u32) -> Result<(), ExplorerError> {
        const METHOD: &str = "generatetoaddress";

        let address = Self::elementsd_getnewaddress(client, "", AddressType::default())?.to_string();
        client.call::<Value>(METHOD, &[block_num.into(), address.into()])?;
        Ok(())
    }

    fn sweep_initialfreecoins(client: &Client) -> Result<(), ExplorerError> {
        const METHOD: &str = "sendtoaddress";

        let address = Self::elementsd_getnewaddress(client, "", AddressType::default())?;
        client.call::<Value>(
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

    pub fn elementsd_issueasset(client: &Client, satoshi: u64) -> Result<AssetId, ExplorerError> {
        const METHOD: &str = "issueasset";

        let btc = sat2btc(satoshi);
        let r = client.call::<Value>(METHOD, &[btc.into(), 0.into()])?;
        let asset = r.get("asset").unwrap().as_str().unwrap().to_string();
        Ok(AssetId::from_str(&asset)?)
    }

    pub fn elementsd_height(client: &Client) -> Result<u64, ExplorerError> {
        const METHOD: &str = "getblockchaininfo";

        let raw: serde_json::Value = client.call(METHOD, &[])?;
        Ok(raw.get("blocks").unwrap().as_u64().unwrap())
    }

    /// Get the genesis block hash from the running elementsd node.
    ///
    /// Could differ from the hardcoded one because parameters like `-initialfreecoins`
    /// change the genesis hash.
    pub fn elementsd_genesis_block_hash(client: &Client) -> Result<BlockHash, ExplorerError> {
        Self::elementsd_block_hash(client, 0)
    }

    pub fn elementsd_block_hash(client: &Client, height: u64) -> Result<BlockHash, ExplorerError> {
        const METHOD: &str = "getblockhash";

        let raw: Value = client.call(METHOD, &[height.into()])?;
        Ok(BlockHash::from_str(raw.as_str().unwrap())?)
    }

    pub fn elementsd_getpeginaddress(client: &Client) -> Result<(bitcoin::Address, String), ExplorerError> {
        #[derive(serde::Deserialize)]
        struct GetpeginaddressResult {
            getpeginaddress: String,
            claim_script: String,
        }

        const METHOD: &str = "getpeginaddress";
        let value: GetpeginaddressResult = client.call(METHOD, &[]).unwrap();

        let mainchain_address = bitcoin::Address::from_str(&value.getpeginaddress)
            .unwrap()
            .assume_checked();

        Ok((mainchain_address, value.claim_script))
    }

    pub fn elementsd_raw_createpsbt(client: &Client, inputs: Value, outputs: Value) -> Result<String, ExplorerError> {
        const METHOD: &str = "createpsbt";

        let psbt: serde_json::Value = client.call(METHOD, &[inputs, outputs, 0.into(), false.into()])?;
        Ok(psbt.as_str().unwrap().to_string())
    }

    pub fn elementsd_expected_next(client: &Client, base64: &str) -> Result<String, ExplorerError> {
        const METHOD: &str = "analyzepsbt";

        let value: serde_json::Value = client.call(METHOD, &[base64.into()])?;
        Ok(value.get("next").unwrap().as_str().unwrap().to_string())
    }

    pub fn elementsd_walletprocesspsbt(client: &Client, psbt: &str) -> Result<String, ExplorerError> {
        const METHOD: &str = "walletprocesspsbt";

        let value: serde_json::Value = client.call(METHOD, &[psbt.into()])?;
        Ok(value.get("psbt").unwrap().as_str().unwrap().to_string())
    }

    pub fn elementsd_finalizepsbt(client: &Client, psbt: &str) -> Result<String, ExplorerError> {
        const METHOD: &str = "finalizepsbt";

        let value: serde_json::Value = client.call(METHOD, &[psbt.into()])?;
        assert!(value.get("complete").unwrap().as_bool().unwrap());
        Ok(value.get("hex").unwrap().as_str().unwrap().to_string())
    }

    pub fn elementsd_sendrawtransaction(client: &Client, tx: &str) -> Result<String, ExplorerError> {
        const METHOD: &str = "sendrawtransaction";

        let value: serde_json::Value = client.call(METHOD, &[tx.into()])?;
        Ok(value.as_str().unwrap().to_string())
    }

    pub fn elementsd_testmempoolaccept(client: &Client, tx: &str) -> Result<bool, ExplorerError> {
        const METHOD: &str = "testmempoolaccept";

        let value: serde_json::Value = client.call(METHOD, &[[tx].into()])?;
        Ok(value.as_array().unwrap()[0].get("allowed").unwrap().as_bool().unwrap())
    }

    pub fn create_wallet(client: &Client, wallet_name: Option<String>) -> Result<WalletMeta, ExplorerError> {
        const METHOD: &str = "createwallet";

        #[derive(serde::Deserialize)]
        pub struct CreatewalletResult {
            name: String,
            warning: String,
        }

        let value: CreatewalletResult = client.call(
            METHOD,
            &[
                wallet_name.unwrap_or("my_wallet_name".to_string()).into(),
                false.into(),
                false.into(),
                "".into(),
                false.into(),
                false.into(),
                true.into(),
                false.into(),
            ],
        )?;
        Ok(WalletMeta { name: value.name })
    }
}

fn sat2btc(sat: u64) -> String {
    let amount = bitcoin::Amount::from_sat(sat);
    amount.to_string_in(bitcoin::amount::Denomination::Bitcoin)
}
