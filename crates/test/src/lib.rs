mod common;
mod error;
mod testing;

pub use common::*;
pub use error::*;

use bitcoind::bitcoincore_rpc::{Auth, Client};
use bitcoind::{BitcoinD, Conf};
use electrsd::bitcoind;
use simplex_config::Config;
use simplex_core::SimplicityNetwork;
use simplex_runtime::elements_rpc::ElementsRpcClient;
use simplicityhl::elements::secp256k1_zkp::PublicKey;
use simplicityhl::elements::{Address, AssetId};
use std::path::{Path, PathBuf};

#[derive(Hash, Clone, Debug, Eq, PartialEq)]
pub struct User {
    pubkey: PublicKey,
}

pub enum TestProvider {
    ConfiguredNode { node: BitcoinD, network: SimplicityNetwork },
    CustomRpc(ElementsRpcClient),
}

pub enum ConfigOption<'a> {
    DefaultRegtest,
    CustomConfRegtest { conf: Conf<'a> },
    CustomRpcUrlRegtest { url: String, auth: Auth },
}

impl TestProvider {
    pub fn init(init_option: ConfigOption) -> Result<Self, TestError> {
        let rpc = match init_option {
            ConfigOption::DefaultRegtest => {
                let node = Self::create_default_node();
                let network = SimplicityNetwork::default_regtest();
                Self::ConfiguredNode { node, network }
            }
            ConfigOption::CustomConfRegtest { conf } => {
                let node = Self::create_node(conf, Self::get_bin_path())?;
                let network = SimplicityNetwork::default_regtest();
                Self::ConfiguredNode { node, network }
            }
            ConfigOption::CustomRpcUrlRegtest { auth, url: rpc_url } => {
                let network = SimplicityNetwork::default_regtest();
                Self::CustomRpc(ElementsRpcClient::new(network, &rpc_url, auth)?)
            }
        };

        if let Err(e) = ElementsRpcClient::blockchain_info(rpc.as_ref()) {
            return Err(TestError::UnhealthyRpc(e));
        }
        Ok(rpc)
    }

    // TODO: is it ok?
    pub fn obtain_test_config() -> Config {
        todo!()
    }

    pub fn get_bin_path() -> PathBuf {
        // TODO: change binary into installed one in $HOME dir
        const ELEMENTSD_BIN_PATH: &str = "../../assets/elementsd";
        const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

        Path::new(MANIFEST_DIR).join(ELEMENTSD_BIN_PATH)
    }

    fn create_default_node() -> BitcoinD {
        let mut conf = Conf::default();
        let bin_args = common::DefaultElementsdParams {}.get_bin_args();

        conf.args = bin_args.iter().map(|x| x.as_ref()).collect::<Vec<&str>>();
        conf.network = "liquidregtest";
        conf.p2p = bitcoind::P2P::Yes;

        BitcoinD::with_conf(Self::get_bin_path(), &conf).unwrap()
    }

    pub fn create_default_node_with_stdin() -> BitcoinD {
        let mut conf = Conf::default();
        let bin_args = common::DefaultElementsdParams {}.get_bin_args();

        conf.args = bin_args.iter().map(|x| x.as_ref()).collect::<Vec<&str>>();
        conf.view_stdout = true;
        conf.attempts = 2;
        conf.network = "liquidregtest";
        conf.p2p = bitcoind::P2P::Yes;

        BitcoinD::with_conf(Self::get_bin_path(), &conf).unwrap()
    }

    fn create_node(conf: Conf, bin_path: PathBuf) -> Result<BitcoinD, TestError> {
        BitcoinD::with_conf(bin_path, &conf).map_err(|e| TestError::NodeFailedToStart(e.to_string()))
    }

    pub fn client(&self) -> &Client {
        match self {
            TestProvider::ConfiguredNode { node, .. } => &node.client,
            TestProvider::CustomRpc(x) => x.client(),
        }
    }

    pub fn network(&self) -> SimplicityNetwork {
        match self {
            TestProvider::ConfiguredNode { network, .. } => *network,
            TestProvider::CustomRpc(x) => x.network(),
        }
    }
}

impl TestProvider {
    pub fn fund(satoshi: u64, address: Option<Address>, asset: Option<AssetId>) {
        todo!()
    }

    pub fn get_height() {}

    pub fn get_blockchain_info() {
        todo!()
    }
}

impl AsRef<Client> for TestProvider {
    fn as_ref(&self) -> &Client {
        self.client()
    }
}
