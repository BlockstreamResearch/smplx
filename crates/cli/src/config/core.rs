use serde::Deserialize;
use std::path::{Path, PathBuf};

use smplx_build::BuildConfig;
use smplx_regtest::RegtestConfig;
use smplx_sdk::program::TrackerLogLevel;
use smplx_test::{TestConfig, config::Verbosity};

use super::error::ConfigError;

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
    /// Retrieves the default path for the configuration by using `std::env::current_dir()`
    ///
    /// # Errors
    /// This function can return a `ConfigError` in the following cases:
    /// - If the current working directory cannot be determined.
    /// - If the `get_path` function encounters an error, processing the retrieved path.
    pub fn get_default_path() -> Result<PathBuf, ConfigError> {
        Self::get_path(std::env::current_dir()?)
    }

    /// Constructs a complete configuration file path by joining the provided path with the
    /// predefined configuration file name `CONFIG_FILENAME`.
    ///
    /// # Errors
    /// This function will return an error if the provided `path` cannot be resolved for any reason
    /// that would result in a failure when interacting with path-related operations.
    pub fn get_path(path: impl AsRef<Path>) -> Result<PathBuf, ConfigError> {
        Ok(path.as_ref().join(CONFIG_FILENAME))
    }

    /// Loads a configuration file from a given path and deserializes its contents into a `Config` object.
    ///
    /// # Errors
    /// - `ConfigError::PathIsNotFile`: If the given path is not a file.
    /// - `ConfigError::PathNotExists`: If the given path does not exist.
    /// - `ConfigError::UnableToDeserialize`: If the file's contents cannot be parsed as valid TOML.
    /// - Any other I/O errors that may occur when reading the file.
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
        match config.test.verbosity {
            Some(verbosity) => Self::validate_verbosity(verbosity),
            None => Ok(()),
        }?;

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

    fn validate_verbosity(verbosity: Verbosity) -> Result<(), ConfigError> {
        match TryInto::<TrackerLogLevel>::try_into(verbosity) {
            Ok(_) => Ok(()),
            Err(val) => Err(ConfigError::BadVersbosityMode(val.0)),
        }
    }

    fn validate_network(network: &String) -> Result<(), ConfigError> {
        if network != "Liquid" && network != "LiquidTestnet" && network != "ElementsRegtest" {
            return Err(ConfigError::BadNetworkName(network.clone()));
        }

        Ok(())
    }
}
