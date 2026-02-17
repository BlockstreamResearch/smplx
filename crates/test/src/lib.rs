mod common;
mod error;

pub use common::*;
pub use error::*;

use corepc_node::client::client_sync::Auth;
use corepc_node::{ Conf, Node};
use simplex_explorer::ElementsRpcClient;
use simplicityhl::elements::bitcoin::secp256k1;
use simplicityhl::elements::schnorr::Keypair;
use simplicityhl::elements::secp256k1_zkp::rand::thread_rng;
use simplicityhl::elements::secp256k1_zkp::PublicKey;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Hash, Clone, Debug, Eq, PartialEq)]
pub struct User {
    pubkey: PublicKey,
}

pub struct TestArgs {
    _test_path: String,
    user_map: HashMap<User, Keypair>,
    elements_rpc: ElementsRpc,
}

pub struct ElementsRpc {
    elementsd: Node,
}

pub enum ConfigOption {
    Config,
    Custom {
        url: String,
        simplicity_network: simplex_explorer::Network,
        auth: Auth,
    },
}

impl TestArgs {
    pub fn new(test_path: impl AsRef<str>, rpc_info: ElementsRpc) -> Self {
        Self {
            _test_path: test_path.as_ref().to_string(),
            user_map: Default::default(),
            elements_rpc: rpc_info,
        }
    }

    pub fn from_rpc(rpc_info: ElementsRpc) -> Self {
        Self {
            _test_path: "".to_string(),
            user_map: Default::default(),
            elements_rpc: rpc_info,
        }
    }

    pub fn create_user(&mut self) -> User {
        let keypair = Keypair::new(secp256k1::SECP256K1, &mut thread_rng());
        let user = User {
            pubkey: keypair.public_key(),
        };
        self.user_map.insert(user.clone(), keypair);
        user
    }

    pub fn get_rpc(&self) -> &ElementsRpc {
        &self.elements_rpc
    }
}

impl ElementsRpc {
    pub fn init_rpc(init_option: ConfigOption) -> Result<ElementsRpcClient, TestError> {
        let client = match init_option {
            ConfigOption::Config => {
                todo!()
            }
            ConfigOption::Custom {
                auth,
                simplicity_network: network,
                url: rpc_url,
            } => {
                println!(" == Rpc url: {}", rpc_url);

                ElementsRpcClient::new(network, &rpc_url, auth)?
            }
        };
        if let Err(e) = client.blockchain_info() {
            return Err(TestError::UnhealthyRpc(e));
        }
        Ok(client)
    }

    pub fn get_bin_path() -> PathBuf {
        const ELEMENTSD_BIN_PATH: &str = "../../assets/elementsd";
        const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

        Path::new(MANIFEST_DIR).join(ELEMENTSD_BIN_PATH)
    }

    pub fn create_default_node() -> Node {
        let mut conf = Conf::default();
        let bin_args = common::DefaultElementsdParams{}.get_bin_args();

        conf.args = bin_args.iter().map(|x| x.as_ref()).collect::<Vec<&str>>();
        conf.wallet = None;

        Node::with_conf(Self::get_bin_path(), &conf).unwrap()
    }

     fn create_node(conf: Conf, bin_path: PathBuf) -> Result<Node, TestError> {
        Node::with_conf(bin_path, &conf).map_err(|e| TestError::NodeFailedToStart(e.to_string()))
    }

    pub fn spawn_default() -> Result<Self, TestError> {
        let node = Self::create_default_node();
        Self::spawn_elements_rpc(node)
    }

    pub fn spawn(conf: Conf) -> Result<Self, TestError> {
        let node = Self::create_node(conf, Self::get_bin_path())?;
        Ok(Self { elementsd: node })
    }

    fn spawn_elements_rpc(node: Node) -> Result<Self, TestError> {
        Ok(ElementsRpc { elementsd: node })
    }


    pub fn node(&self) -> &Node {
        &self.elementsd
    }
}
