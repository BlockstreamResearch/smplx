use std::fs;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::error::TestError;

pub const TEST_ENV_NAME: &str = "SIMPLEX_TEST_ENV";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub esplora: Option<String>,
    pub rpc: Option<RpcConfig>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub url: String,
    pub username: String,
    pub password: String,
}

impl TestConfig {
    pub fn to_file(&self, path: &impl AsRef<Path>) -> Result<(), TestError> {
        if let Some(parent_dir) = path.as_ref().parent() {
            fs::create_dir_all(parent_dir)?;
        }

        let mut file = OpenOptions::new().create(true).write(true).open(&path)?;

        file.write(toml::to_string_pretty(&self).unwrap().as_bytes())?;
        file.flush()?;

        Ok(())
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, TestError> {
        let mut content = String::new();
        let mut file = OpenOptions::new().read(true).open(path)?;

        file.read_to_string(&mut content)?;

        Ok(toml::from_str(&content)?)
    }
}
