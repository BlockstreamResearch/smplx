use serde::{Deserialize, Serialize};
use simplicityhl::elements::AddressParams;
use std::path::{Path, PathBuf};

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    /// Standard I/O errors.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Errors when parsing TOML configuration files.
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub network: NetworkConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    #[serde(default = "default_network")]
    pub name: NetworkName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NetworkName {
    #[default]
    Testnet,
    Mainnet,
}

impl NetworkName {
    #[must_use]
    pub const fn address_params(self) -> &'static AddressParams {
        match self {
            Self::Testnet => &AddressParams::LIQUID_TESTNET,
            Self::Mainnet => &AddressParams::LIQUID,
        }
    }
}

impl Config {
    /// Loads configuration from the specified path.
    ///
    /// # Errors
    /// Returns `Error::Io` if the file cannot be read, or `Error::TomlParse` if the content
    /// is not valid TOML.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn load_or_default(path: impl AsRef<Path>) -> Self {
        Self::load(path).unwrap_or_default()
    }

    #[must_use]
    pub const fn address_params(&self) -> &'static AddressParams {
        self.network.name.address_params()
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            name: default_network(),
        }
    }
}

const fn default_network() -> NetworkName {
    NetworkName::Testnet
}
