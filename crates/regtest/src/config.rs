use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use super::error::RegtestError;

pub const DEFAULT_REGTEST_MNEMONIC: &str = "exist carry drive collect lend cereal occur much tiger just involve mean";

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RegtestConfig {
    pub mnemonic: String,
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
        }
    }
}
