use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use bitcoind::bitcoincore_rpc::bitcoin;

use electrsd::bitcoind;
use electrsd::bitcoind::bitcoincore_rpc::jsonrpc::serde_json::Value;
use electrsd::bitcoind::bitcoincore_rpc::{Auth, Client};
use electrsd::bitcoind::{BitcoinD, Conf};

use simplicityhl::elements::Transaction;
use simplicityhl::elements::hex::ToHex;
use simplicityhl::elements::{Address, AssetId, BlockHash, OutPoint, Script, TxOut, Txid};

pub use simplex_provider::elements_rpc::*;

use simplex_sdk::provider::SimplicityNetwork;
use simplex_sdk::provider::{ProviderError, ProviderTrait};

use crate::{ElementsdParams, TestError, common};

pub enum TestClientProvider {
    ConfiguredNode { node: BitcoinD, network: SimplicityNetwork },
    CustomRpc(ElementsRpcClient),
}

pub enum ConfigOption<'a> {
    DefaultRegtest,
    CustomConfRegtest { conf: Conf<'a> },
    CustomRpcUrlRegtest { url: String, auth: Auth },
}

impl TestClientProvider {
    pub fn init(init_option: ConfigOption, elementsd_path: impl AsRef<Path>) -> Result<Self, TestError> {
        let rpc = match init_option {
            ConfigOption::DefaultRegtest => {
                let node = Self::create_default_node(elementsd_path);
                let network = SimplicityNetwork::default_regtest();
                Self::ConfiguredNode { node, network }
            }
            ConfigOption::CustomConfRegtest { conf } => {
                let node = Self::create_node(conf, elementsd_path)?;
                let network = SimplicityNetwork::default_regtest();
                Self::ConfiguredNode { node, network }
            }
            ConfigOption::CustomRpcUrlRegtest { auth, url: rpc_url } => {
                Self::CustomRpc(ElementsRpcClient::new(&rpc_url, auth)?)
            }
        };

        if let Err(e) = ElementsRpcClient::blockchain_info(rpc.as_ref()) {
            return Err(TestError::UnhealthyRpc(e));
        }
        Ok(rpc)
    }

    fn create_default_node(bin_path: impl AsRef<Path>) -> BitcoinD {
        let mut conf = Conf::default();
        let bin_args = common::DefaultElementsdParams {}.get_bin_args();

        conf.args = bin_args.iter().map(|x| x.as_ref()).collect::<Vec<&str>>();
        conf.network = "liquidregtest";
        conf.p2p = bitcoind::P2P::Yes;

        BitcoinD::with_conf(bin_path.as_ref(), &conf).unwrap()
    }

    pub fn create_default_node_with_stdin(bin_path: impl AsRef<Path>) -> BitcoinD {
        let mut conf = Conf::default();
        let bin_args = common::DefaultElementsdParams {}.get_bin_args();

        conf.args = bin_args.iter().map(|x| x.as_ref()).collect::<Vec<&str>>();
        conf.view_stdout = true;
        conf.attempts = 2;
        conf.network = "liquidregtest";
        conf.p2p = bitcoind::P2P::Yes;

        BitcoinD::with_conf(bin_path.as_ref(), &conf).unwrap()
    }

    fn create_node(conf: Conf, bin_path: impl AsRef<Path>) -> Result<BitcoinD, TestError> {
        BitcoinD::with_conf(bin_path.as_ref(), &conf).map_err(|e| TestError::NodeFailedToStart(e.to_string()))
    }

    pub fn client(&self) -> &Client {
        match self {
            TestClientProvider::ConfiguredNode { node, .. } => &node.client,
            TestClientProvider::CustomRpc(x) => x.client(),
        }
    }
}

impl AsRef<Client> for TestClientProvider {
    fn as_ref(&self) -> &Client {
        self.client()
    }
}

pub struct TestRpcProvider {
    provider: TestClientProvider,
}

impl ProviderTrait for TestRpcProvider {
    fn broadcast_transaction(&self, tx: &Transaction) -> Result<Txid, ProviderError> {
        use simplicityhl::simplicity::elements::encode;
        let tx_hex = encode::serialize_hex(tx);
        self.sendrawtransaction(&tx_hex)
            .map_err(|e| ProviderError::Request(e.to_string()))
    }

    fn wait(&self, txid: &Txid) -> Result<(), ProviderError> {
        todo!()
    }

    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, ProviderError> {
        // Todo: search for appropriate endpoint
        let mut map = HashMap::new();
        map.insert("".to_string(), 0.1);
        Ok(map)
    }

    fn fetch_address_utxos(&self, address: &Address) -> Result<Vec<(OutPoint, TxOut)>, ProviderError> {
        todo!()
    }

    fn fetch_scripthash_utxos(&self, script: &Script) -> Result<Vec<(OutPoint, TxOut)>, ProviderError> {
        todo!()
    }

    fn fetch_transaction(&self, txid: &Txid) -> Result<Transaction, ProviderError> {
        self.gettransaction(&txid)
            .map_err(|e| ProviderError::Request(e.to_string()))
    }
}

impl TestRpcProvider {
    pub fn init(init_option: ConfigOption, bin_path: impl AsRef<Path>) -> Result<Self, TestError> {
        Ok(Self {
            provider: TestClientProvider::init(init_option, bin_path)?,
        })
    }

    pub fn gettransaction(&self, txid: &Txid) -> Result<Transaction, TestError> {
        use simplicityhl::elements::encode;

        let client = self.provider.client();
        let res = ElementsRpcClient::getrawtransaction_hex(client, &txid.to_hex())?;
        let tx: Transaction =
            encode::deserialize(res.as_bytes()).map_err(|e| TestError::TransactionDecode(e.to_string()))?;
        Ok(tx)
    }

    pub fn height(&self) -> Result<u64, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::height(client)?)
    }

    pub fn blockchain_info(&self) -> Result<GetBlockchainInfo, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::blockchain_info(client)?)
    }

    pub fn sendtoaddress(&self, address: &Address, satoshi: u64, asset: Option<AssetId>) -> Result<Txid, TestError> {
        Ok(ElementsRpcClient::sendtoaddress(
            self.provider.client(),
            address,
            satoshi,
            asset,
        )?)
    }
    pub fn rescanblockchain(&self, start: Option<u64>, stop: Option<u64>) -> Result<(), TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::rescanblockchain(client, start, stop)?)
    }

    pub fn getnewaddress(&self, label: &str, kind: AddressType) -> Result<Address, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::getnewaddress(client, label, kind)?)
    }

    pub fn generate_blocks(&self, block_num: u32) -> Result<(), TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::generate_blocks(client, block_num)?)
    }

    pub fn sweep_initialfreecoins(&self) -> Result<(), TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::sweep_initialfreecoins(client)?)
    }

    pub fn issueasset(&self, satoshi: u64) -> Result<AssetId, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::issueasset(client, satoshi)?)
    }

    pub fn genesis_block_hash(&self) -> Result<BlockHash, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::genesis_block_hash(client)?)
    }

    pub fn block_hash(&self, height: u64) -> Result<BlockHash, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::block_hash(client, height)?)
    }

    pub fn getpeginaddress(&self) -> Result<(bitcoin::Address, String), TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::getpeginaddress(client)?)
    }

    pub fn raw_createpsbt(&self, inputs: Value, outputs: Value) -> Result<String, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::raw_createpsbt(client, inputs, outputs)?)
    }

    pub fn expected_next(&self, base64: &str) -> Result<String, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::expected_next(client, base64)?)
    }

    pub fn walletprocesspsbt(&self, psbt: &str) -> Result<String, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::walletprocesspsbt(client, psbt)?)
    }

    pub fn finalizepsbt(&self, psbt: &str) -> Result<String, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::finalizepsbt(client, psbt)?)
    }

    pub fn sendrawtransaction(&self, tx: &str) -> Result<Txid, TestError> {
        let client = self.provider.client();
        let res = ElementsRpcClient::sendrawtransaction(client, tx)?;
        Ok(Txid::from_str(&res.txid)?)
    }

    pub fn testmempoolaccept(&self, tx: &str) -> Result<bool, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::testmempoolaccept(client, tx)?)
    }

    pub fn create_wallet(&self, wallet_name: Option<String>) -> Result<WalletMeta, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::create_wallet(client, wallet_name)?)
    }

    pub fn getbalance(&self, conf: Option<u64>) -> Result<GetBalance, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::getbalance(client, conf)?)
    }

    pub fn listunspent(
        &self,
        min_conf: Option<u64>,
        max_conf: Option<u64>,
        addresses: Option<Vec<String>>,
        include_unsafe: Option<bool>,
        query_options: Option<QueryOptions>,
    ) -> Result<Vec<ListUnspent>, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::listunspent(
            client,
            min_conf,
            max_conf,
            addresses,
            include_unsafe,
            query_options,
        )?)
    }
    pub fn importaddress(
        &self,
        address: &str,
        label: Option<&str>,
        rescan: Option<bool>,
        p2sh: Option<bool>,
    ) -> Result<(), TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::importaddress(client, address, label, rescan, p2sh)?)
    }
    pub fn validateaddress(&self, address: &str) -> Result<bool, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::validateaddress(client, address)?)
    }

    pub fn scantxoutset(
        &self,
        action: &str,
        scanobjects: Option<Vec<String>>,
    ) -> Result<ScantxoutsetResult, TestError> {
        let client = self.provider.client();
        Ok(ElementsRpcClient::scantxoutset(client, action, scanobjects)?)
    }
}
