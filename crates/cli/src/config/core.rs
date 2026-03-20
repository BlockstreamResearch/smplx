use serde::Deserialize;
use std::path::{Path, PathBuf};

use smplx_build::BuildConfig;
use smplx_regtest::RegtestConfig;
use smplx_test::TestConfig;

use super::error::{ConfigError, ConfigResult};

pub const INIT_CONFIG: &str = include_str!("../../assets/Simplex.default.toml");
pub const CONFIG_FILENAME: &str = "Simplex.toml";

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub build: BuildConfig,
    pub regtest: RegtestConfig,
    pub test: TestConfig,
}

impl Config {
    pub fn get_default_path() -> ConfigResult<PathBuf> {
        let cwd = std::env::current_dir()?;
        Self::get_derived_default_path(cwd)
    }

    pub fn pwd() -> ConfigResult<PathBuf> {
        Ok(std::env::current_dir()?)
    }

    pub fn get_derived_default_path(path: impl AsRef<Path>) -> ConfigResult<PathBuf> {
        Ok(path.as_ref().join(CONFIG_FILENAME))
    }

    pub fn load(path_buf: impl AsRef<Path>) -> ConfigResult<Self> {
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

    fn validate(config: &Config) -> ConfigResult<()> {
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

    fn validate_network(network: &String) -> ConfigResult<()> {
        if network != "Liquid" && network != "LiquidTestnet" && network != "ElementsRegtest" {
            return Err(ConfigError::BadNetworkName(network.clone()));
        }

        Ok(())
    }
}
