use std::fs;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use smplx_regtest::RegtestConfig;

use super::error::TestError;

pub const TEST_ENV_NAME: &str = "SIMPLEX_TEST_ENV";
pub const DEFAULT_TEST_MNEMONIC: &str = "exist carry drive collect lend cereal occur much tiger just involve mean";
pub const DEFAULT_BITCOINS: u64 = 10_000_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TestConfig {
    pub mnemonic: String,
    pub bitcoins: u64,
    pub esplora: Option<EsploraConfig>,
    pub rpc: Option<RpcConfig>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EsploraConfig {
    pub url: String,
    pub network: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub url: String,
    pub username: String,
    pub password: String,
}

impl TestConfig {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, TestError> {
        let mut content = String::new();
        let mut file = OpenOptions::new().read(true).open(path)?;

        file.read_to_string(&mut content)?;

        Ok(toml::from_str(&content)?)
    }

    pub fn to_regtest_config(&self) -> RegtestConfig {
        RegtestConfig {
            mnemonic: self.mnemonic.clone(),
            bitcoins: self.bitcoins,
            rpc_port: None,
            esplora_port: None,
            rpc_user: None,
            rpc_password: None,
        }
    }

    pub fn to_file(&self, path: &impl AsRef<Path>) -> Result<(), TestError> {
        if let Some(parent_dir) = path.as_ref().parent() {
            fs::create_dir_all(parent_dir)?;
        }

        let mut file = OpenOptions::new().create(true).write(true).truncate(true).open(path)?;

        file.write_all(toml::to_string_pretty(&self).unwrap().as_bytes())?;
        file.flush()?;

        Ok(())
    }
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            mnemonic: DEFAULT_TEST_MNEMONIC.to_string(),
            bitcoins: DEFAULT_BITCOINS,
            esplora: None,
            rpc: None,
        }
    }
}
