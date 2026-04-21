use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use super::error::RegtestError;

pub const DEFAULT_REGTEST_MNEMONIC: &str = "exist carry drive collect lend cereal occur much tiger just involve mean";
pub const DEFAULT_BITCOINS: u64 = 10_000_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RegtestConfig {
    pub mnemonic: String,
    pub bitcoins: u64,
    pub rpc_port: Option<u16>,
    pub esplora_port: Option<u16>,
    pub rpc_user: Option<String>,
    pub rpc_password: Option<String>,
}

impl RegtestConfig {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, RegtestError> {
        let mut content = String::new();
        let mut file = OpenOptions::new().read(true).open(path)?;

        file.read_to_string(&mut content)?;

        Ok(toml::from_str(&content)?)
    }
}

impl Default for RegtestConfig {
    fn default() -> Self {
        Self {
            mnemonic: DEFAULT_REGTEST_MNEMONIC.to_string(),
            bitcoins: DEFAULT_BITCOINS,
            rpc_port: None,
            esplora_port: None,
            rpc_user: None,
            rpc_password: None,
        }
    }
}
