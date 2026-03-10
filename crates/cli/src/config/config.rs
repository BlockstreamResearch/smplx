use serde::Deserialize;
use std::path::{Path, PathBuf};

use simplex_build::BuildConfig;
use simplex_regtest::RegtestConfig;
use simplex_test::TestConfig;

use super::error::ConfigError;

pub const INIT_CONFIG: &str = include_str!("../../Simplex.default.toml");
pub const CONFIG_FILENAME: &str = "Simplex.toml";

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub build: BuildConfig,
    pub regtest: RegtestConfig,
    pub test: TestConfig,
}

impl Config {
    pub fn get_default_path() -> Result<PathBuf, ConfigError> {
        let cwd = std::env::current_dir()?;

        Ok(cwd.join(CONFIG_FILENAME))
    }

    pub fn load(path_buf: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path_buf.as_ref().to_path_buf();

        if !path.is_file() {
            return Err(ConfigError::PathIsNotFile(path));
        }

        if !path.exists() {
            return Err(ConfigError::PathNotExists(path));
        }

        let conf_str = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(conf_str.as_str()).map_err(ConfigError::UnableToDeserialize)?;

        Self::validate(&config)?;

        Ok(config)
    }

    fn validate(config: &Config) -> Result<(), ConfigError> {
        match config.test.esplora.clone() {
            Some(esplora_config) => {
                Self::validate_network(&esplora_config.network)?;

                if config.test.rpc.is_some() && esplora_config.network != "ElementsRegtest" {
                    return Err(ConfigError::NetworkNameUnmatched(esplora_config.network.clone()));
                }

                Ok(())
            }
            None => Ok(()),
        }
    }

    fn validate_network(network: &String) -> Result<(), ConfigError> {
        if network != "Liquid" && network != "LiquidTestnet" && network != "ElementsRegtest" {
            return Err(ConfigError::BadNetworkName(network.clone()));
        }

        Ok(())
    }
}
