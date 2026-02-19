use serde::{Deserialize, Serialize};
use simplex_core::SimplicityNetwork;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::str::FromStr;

const MANIFEST_DIR: &str = "CARGO_MANIFEST_DIR";
const CONFIG_FILENAME: &str = "Simplex.toml";

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    /// Standard I/O errors.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Errors when parsing TOML configuration files.
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// Errors when parsing TOML configuration files.
    #[error("Unable to deserialize config: {0}")]
    UnableToDeserialize(toml::de::Error),

    /// Errors when parsing env variable.
    #[error("Unable to get env variable: {0}")]
    UnableToGetEnv(#[from] std::env::VarError),

    /// Errors when getting a path to config.
    #[error("Path doesn't a file: '{0}'")]
    PathIsNotFile(PathBuf),

    /// Errors when getting a path to config.
    #[error("Path doesn't exist: '{0}'")]
    PathIsNotEsixt(PathBuf),
}

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub provider_config: ProviderConfig,
    pub test_config: TestConfig,
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    simplicity_network: SimplicityNetwork,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub rpc_creds: RpcCreds,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum RpcCreds {
    Auth {
        rpc_username: String,
        rpc_password: String,
    },
    #[default]
    None,
}

#[derive(Debug, Default, Clone)]
pub struct ConfigOverride {
    pub rpc_creds: Option<TestConfig>,
    pub network: Option<SimplicityNetwork>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        ProviderConfig {
            simplicity_network: SimplicityNetwork::LiquidTestnet,
        }
    }
}

impl Config {
    pub fn discover(cfg_override: &ConfigOverride) -> Result<Option<Config>, ConfigError> {
        Config::_discover().map(|opt| {
            opt.map(|mut cfg| {
                if let Some(test_conf) = cfg_override.rpc_creds.clone() {
                    cfg.test_config = test_conf;
                }
                if let Some(network) = cfg_override.network {
                    cfg.provider_config.simplicity_network = network;
                }
                cfg
            })
        })
    }

    pub fn load_or_default(path_buf: impl AsRef<Path>) -> Self {
        Self::from_path(path_buf).unwrap_or_else(|_| {
            if let Ok(Some(conf)) = Self::_discover() {
                conf
            } else {
                Self::default()
            }
        })
    }

    fn _discover() -> Result<Option<Config>, ConfigError> {
        let cwd = std::env::current_dir()?;
        let path = cwd.join(CONFIG_FILENAME);
        dbg!(&path);
        if !path.is_file() {
            return Err(ConfigError::PathIsNotFile(path));
        }
        if !path.exists() {
            return Err(ConfigError::PathIsNotEsixt(path));
        }
        Ok(Some(Config::from_path(&path)?))
    }

    fn from_path(p: impl AsRef<Path>) -> Result<Self, ConfigError> {
        std::fs::read_to_string(p.as_ref())?.parse()
    }
}

impl FromStr for Config {
    type Err = ConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cfg: _Config = toml::from_str(s).map_err(ConfigError::UnableToDeserialize)?;
        Ok(Config {
            provider_config: ProviderConfig {
                simplicity_network: cfg.network.unwrap_or_default().into(),
            },
            test_config: cfg.test.unwrap_or_default(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct _Config {
    network: Option<_NetworkName>,
    test: Option<TestConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum _NetworkName {
    #[default]
    Liquid,
    LiquidTestnet,
    ElementsRegtest,
}

impl Into<SimplicityNetwork> for _NetworkName {
    fn into(self) -> SimplicityNetwork {
        match self {
            _NetworkName::Liquid => SimplicityNetwork::Liquid,
            _NetworkName::LiquidTestnet => SimplicityNetwork::LiquidTestnet,
            _NetworkName::ElementsRegtest => SimplicityNetwork::default_regtest(),
        }
    }
}
