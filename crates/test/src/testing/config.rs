use crate::TestError;
use electrsd::bitcoind::bitcoincore_rpc::jsonrpc::serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;

pub const TEST_ENV_NAME: &str = "SIMPLEX_TEST_ENV";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub rpc_creds: RpcCreds,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum RpcCreds {
    Auth {
        url: String,
        username: String,
        password: String,
    },
    #[default]
    None,
}

impl TestConfig {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, TestError> {
        let mut content = String::new();
        let mut file = OpenOptions::new().read(true).open(path)?;
        file.read_to_string(&mut content)?;
        Ok(toml::from_str(&content)?)
    }
}
