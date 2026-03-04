use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::error::TestError;

pub const TEST_ENV_NAME: &str = "SIMPLEX_TEST_ENV";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    #[serde(default)]
    pub esplora: Option<String>,
    #[serde(default)]
    pub rpc: Option<RpcConfig>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    url: String,
    username: String,
    password: String,
}

impl TestConfig {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, TestError> {
        let mut content = String::new();
        let mut file = OpenOptions::new().read(true).open(path)?;

        file.read_to_string(&mut content)?;

        Ok(toml::from_str(&content)?)
    }
}
