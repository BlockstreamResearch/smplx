use std::fs;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use smplx_regtest::RegtestConfig;
use smplx_sdk::global::Verbosity;

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
    pub fuzz: Option<FuzzConfig>,
    pub verbosity: Verbosity,
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct FuzzConfig {
    pub cases: Option<u32>,
    pub max_global_rejects: Option<u32>,
    pub max_local_rejects: Option<u32>,
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
            fuzz: None,
            verbosity: Verbosity::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips_and_converts_to_regtest() {
        let path = std::env::temp_dir().join(format!("smplx-test-config-{}.toml", std::process::id()));
        let config = TestConfig {
            mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
                .into(),
            bitcoins: 42,
            verbosity: Verbosity::Trace,
            esplora: Some(EsploraConfig {
                url: "http://localhost:3000".into(),
                network: "ElementsRegtest".into(),
            }),
            rpc: Some(RpcConfig {
                url: "http://localhost:18443".into(),
                username: "user".into(),
                password: "password".into(),
            }),
            ..Default::default()
        };

        config.to_file(&path).expect("test config should be written");
        let loaded = TestConfig::from_file(&path).expect("test config should be loaded");
        let regtest = loaded.to_regtest_config();

        assert_eq!(loaded.mnemonic, config.mnemonic);
        assert_eq!(loaded.bitcoins, config.bitcoins);
        assert_eq!(loaded.verbosity, Verbosity::Trace);
        let esplora = loaded.esplora.as_ref().unwrap();
        assert_eq!(esplora.url, "http://localhost:3000");
        assert_eq!(esplora.network, "ElementsRegtest");
        let rpc = loaded.rpc.as_ref().unwrap();
        assert_eq!(rpc.url, "http://localhost:18443");
        assert_eq!(rpc.username, "user");
        assert_eq!(rpc.password, "password");
        assert_eq!(regtest.mnemonic, config.mnemonic);
        assert_eq!(regtest.bitcoins, config.bitcoins);
        assert!(regtest.rpc_port.is_none());
        assert!(regtest.esplora_port.is_none());
        assert!(regtest.rpc_user.is_none());
        assert!(regtest.rpc_password.is_none());
        let _ = std::fs::remove_file(path);
    }
}
