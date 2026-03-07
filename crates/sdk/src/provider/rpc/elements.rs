use std::str::FromStr;

use electrsd::bitcoind::bitcoincore_rpc::{Auth, Client, RpcApi};

use serde_json::Value;

use simplicityhl::elements::{Address, AssetId, BlockHash, Txid};

use super::error::RpcError;

use crate::utils::sat2btc;

pub struct ElementsRpc {
    pub inner: Client,
    pub auth: Auth,
    pub url: String,
}

impl ElementsRpc {
    pub fn new(url: String, auth: Auth) -> Result<Self, RpcError> {
        let inner = Client::new(url.as_str(), auth.clone())?;
        inner.ping()?;

        Ok(Self {
            inner: inner,
            auth: auth,
            url: url,
        })
    }

    pub fn height(&self) -> Result<u64, RpcError> {
        const METHOD: &str = "getblockcount";

        self.inner
            .call::<serde_json::Value>(METHOD, &[])?
            .as_u64()
            .ok_or_else(|| RpcError::ElementsRpcUnexpectedReturn(METHOD.into()))
    }

    pub fn block_hash(&self, height: u64) -> Result<BlockHash, RpcError> {
        const METHOD: &str = "getblockhash";

        let raw: Value = self.inner.call(METHOD, &[height.into()])?;

        Ok(BlockHash::from_str(raw.as_str().unwrap())?)
    }

    pub fn sendtoaddress(&self, address: &Address, satoshi: u64, asset: Option<AssetId>) -> Result<Txid, RpcError> {
        const METHOD: &str = "sendtoaddress";

        let btc = sat2btc(satoshi);
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

    pub fn rescanblockchain(&self, start: Option<u64>, stop: Option<u64>) -> Result<(), RpcError> {
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

    pub fn getnewaddress(&self, label: &str) -> Result<Address, RpcError> {
        const METHOD: &str = "getnewaddress";

        let addr: Value = self.inner.call(METHOD, &[label.into(), "bech32".to_string().into()])?;

        Ok(Address::from_str(addr.as_str().unwrap()).unwrap())
    }

    pub fn generate_blocks(&self, block_num: u32) -> Result<(), RpcError> {
        const METHOD: &str = "generatetoaddress";

        let address = self.getnewaddress("")?.to_string();
        self.inner.call::<Value>(METHOD, &[block_num.into(), address.into()])?;

        Ok(())
    }

    pub fn sweep_initialfreecoins(&self) -> Result<(), RpcError> {
        const METHOD: &str = "sendtoaddress";

        let address = self.getnewaddress("")?;
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
}
