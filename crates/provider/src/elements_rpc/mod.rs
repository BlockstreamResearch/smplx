mod types;

pub use types::*;

use crate::error::ExplorerError;
use bitcoind::bitcoincore_rpc::{Auth, Client, RpcApi, bitcoin};
use electrsd::bitcoind;
use serde_json::Value;
use simplicityhl::elements::{Address, AssetId, BlockHash, Txid};
use std::str::FromStr;

pub struct ElementsRpcClient {
    inner: Client,
    #[allow(unused)]
    auth: Auth,
    #[allow(unused)]
    url: String,
}

impl ElementsRpcClient {
    pub fn new(url: &str, auth: Auth) -> Result<Self, ExplorerError> {
        let inner = Client::new(url, auth.clone())?;
        inner.ping()?;
        Ok(Self {
            inner,
            auth,
            url: url.to_string(),
        })
    }

    pub fn new_from_credentials(url: &str, user: &str, pass: &str) -> Result<Self, ExplorerError> {
        let auth = Auth::UserPass(user.to_string(), pass.to_string());
        Self::new(url, auth)
    }

    pub fn client(&self) -> &Client {
        &self.inner
    }
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

    pub fn rescanblockchain(client: &Client, start: Option<u64>, stop: Option<u64>) -> Result<(), ExplorerError> {
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

    pub fn getnewaddress(client: &Client, label: &str, kind: AddressType) -> Result<Address, ExplorerError> {
        const METHOD: &str = "getnewaddress";

        let addr: Value = client.call(METHOD, &[label.into(), kind.to_string().into()])?;
        Ok(Address::from_str(addr.as_str().unwrap()).unwrap())
    }

    pub fn generate_blocks(client: &Client, block_num: u32) -> Result<(), ExplorerError> {
        const METHOD: &str = "generatetoaddress";

        let address = Self::getnewaddress(client, "", AddressType::default())?.to_string();
        client.call::<Value>(METHOD, &[block_num.into(), address.into()])?;
        Ok(())
    }

    pub fn sweep_initialfreecoins(client: &Client) -> Result<(), ExplorerError> {
        const METHOD: &str = "sendtoaddress";

        let address = Self::getnewaddress(client, "", AddressType::default())?;
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

    pub fn issueasset(client: &Client, satoshi: u64) -> Result<AssetId, ExplorerError> {
        const METHOD: &str = "issueasset";

        let btc = sat2btc(satoshi);
        let r = client.call::<Value>(METHOD, &[btc.into(), 0.into()])?;
        let asset = r.get("asset").unwrap().as_str().unwrap().to_string();
        Ok(AssetId::from_str(&asset)?)
    }

    /// Get the genesis block hash from the running elementsd node.
    ///
    /// Could differ from the hardcoded one because parameters like `-initialfreecoins`
    /// change the genesis hash.
    pub fn genesis_block_hash(client: &Client) -> Result<BlockHash, ExplorerError> {
        Self::block_hash(client, 0)
    }

    pub fn block_hash(client: &Client, height: u64) -> Result<BlockHash, ExplorerError> {
        const METHOD: &str = "getblockhash";

        let raw: Value = client.call(METHOD, &[height.into()])?;
        Ok(BlockHash::from_str(raw.as_str().unwrap())?)
    }

    pub fn getpeginaddress(client: &Client) -> Result<(bitcoin::Address, String), ExplorerError> {
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

    pub fn raw_createpsbt(client: &Client, inputs: Value, outputs: Value) -> Result<String, ExplorerError> {
        const METHOD: &str = "createpsbt";

        let psbt: serde_json::Value = client.call(METHOD, &[inputs, outputs, 0.into(), false.into()])?;
        Ok(psbt.as_str().unwrap().to_string())
    }

    pub fn expected_next(client: &Client, base64: &str) -> Result<String, ExplorerError> {
        const METHOD: &str = "analyzepsbt";

        let value: serde_json::Value = client.call(METHOD, &[base64.into()])?;
        Ok(value.get("next").unwrap().as_str().unwrap().to_string())
    }

    pub fn walletprocesspsbt(client: &Client, psbt: &str) -> Result<String, ExplorerError> {
        const METHOD: &str = "walletprocesspsbt";

        let value: serde_json::Value = client.call(METHOD, &[psbt.into()])?;
        Ok(value.get("psbt").unwrap().as_str().unwrap().to_string())
    }

    pub fn finalizepsbt(client: &Client, psbt: &str) -> Result<String, ExplorerError> {
        const METHOD: &str = "finalizepsbt";

        let value: serde_json::Value = client.call(METHOD, &[psbt.into()])?;
        assert!(value.get("complete").unwrap().as_bool().unwrap());
        Ok(value.get("hex").unwrap().as_str().unwrap().to_string())
    }

    pub fn sendrawtransaction(client: &Client, tx: &str) -> Result<String, ExplorerError> {
        const METHOD: &str = "sendrawtransaction";

        let value: serde_json::Value = client.call(METHOD, &[tx.into()])?;
        Ok(value.as_str().unwrap().to_string())
    }

    pub fn testmempoolaccept(client: &Client, tx: &str) -> Result<bool, ExplorerError> {
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

    pub fn getbalance(client: &Client, conf: Option<u64>) -> Result<GetBalance, ExplorerError> {
        const METHOD: &str = "getbalance";

        Ok(client.call::<GetBalance>(METHOD, &["*".into(), conf.unwrap_or_default().into()])?)
    }

    pub fn listunspent(
        client: &Client,
        min_conf: Option<u64>,
        max_conf: Option<u64>,
        addresses: Option<Vec<String>>,
        include_unsafe: Option<bool>,
        query_options: Option<QueryOptions>,
    ) -> Result<Vec<ListUnspent>, ExplorerError> {
        const METHOD: &str = "listunspent";

        let mut args = Vec::new();
        args.push(min_conf.unwrap_or(1).into());
        args.push(max_conf.unwrap_or(9999999).into());

        if let Some(addrs) = addresses {
            args.push(addrs.into());
        } else {
            args.push(serde_json::to_value(Vec::<String>::new()).unwrap());
        }

        if include_unsafe.is_some() || query_options.is_some() {
            args.push(include_unsafe.unwrap_or(true).into());
        }

        if let Some(opts) = query_options {
            args.push(serde_json::to_value(opts).unwrap());
        }

        Ok(client.call::<Vec<ListUnspent>>(METHOD, &args)?)
    }

    pub fn importaddress(
        client: &Client,
        address: &str,
        label: Option<&str>,
        rescan: Option<bool>,
        p2sh: Option<bool>,
    ) -> Result<(), ExplorerError> {
        const METHOD: &str = "importaddress";

        let mut args = vec![address.into()];

        if let Some(lbl) = label {
            args.push(lbl.into());
        } else {
            args.push("".into());
        }

        if rescan.is_some() || p2sh.is_some() {
            args.push(rescan.unwrap_or(true).into());
        }

        if let Some(p2sh_val) = p2sh {
            args.push(p2sh_val.into());
        }

        client.call::<serde_json::Value>(METHOD, &args)?;
        Ok(())
    }

    pub fn validateaddress(client: &Client, address: &str) -> Result<bool, ExplorerError> {
        const METHOD: &str = "validateaddress";

        let value: serde_json::Value = client.call(METHOD, &[address.into()])?;
        Ok(value
            .get("isvalid")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| ExplorerError::ElementsRpcUnexpectedReturn(METHOD.into()))?)
    }

    pub fn scantxoutset(
        client: &Client,
        action: &str,
        scanobjects: Option<Vec<String>>,
    ) -> Result<ScantxoutsetResult, ExplorerError> {
        const METHOD: &str = "scantxoutset";

        let mut args = vec![action.into()];

        match action {
            "start" => {
                if let Some(objects) = scanobjects {
                    args.push(serde_json::to_value(objects).unwrap());
                } else {
                    return Err(ExplorerError::InvalidInput(
                        "scantxoutset 'start' action requires scanobjects".to_string(),
                    ));
                }
            }
            "abort" | "status" => {
                if scanobjects.is_some() {
                    return Err(ExplorerError::InvalidInput(format!(
                        "scantxoutset '{}' action does not accept scanobjects",
                        action
                    )));
                }
            }
            _ => {
                return Err(ExplorerError::InvalidInput(format!(
                    "unknown scantxoutset action: {}",
                    action
                )));
            }
        }

        let response = client.call::<serde_json::Value>(METHOD, &args)?;
        dbg!("response: {}", response.to_string());
        ScantxoutsetResult::from_value(response, action)
            .map_err(|e| ExplorerError::ElementsRpcUnexpectedReturn(e.to_string()))
    }
}

fn sat2btc(sat: u64) -> String {
    let amount = bitcoin::Amount::from_sat(sat);
    amount.to_string_in(bitcoin::amount::Denomination::Bitcoin)
}
