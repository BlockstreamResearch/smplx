use serde::Deserialize;
use std::path::{Path, PathBuf};

use simplex_build::BuildConfig;
use simplex_test::TestConfig;

use super::error::ConfigError;

pub const INIT_CONFIG: &str = include_str!("../../Simplex.default.toml");
pub const CONFIG_FILENAME: &str = "Simplex.toml";

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub test: Option<TestConfig>,
    pub build: BuildConfig,
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

        Ok(toml::from_str(conf_str.as_str()).map_err(ConfigError::UnableToDeserialize)?)
    }
}
