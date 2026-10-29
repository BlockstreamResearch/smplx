use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use smplx_build::{BuildConfig, CONFIG_FILENAME, DependencyConfig};
use smplx_regtest::RegtestConfig;
use smplx_sdk::provider::SimplicityNetwork;
use smplx_test::TestConfig;

use super::error::ConfigError;

pub const INIT_CONFIG: &str = include_str!("../../assets/Simplex.default.toml");

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub build: BuildConfig,

    /// Key is dependency root path name
    pub dependencies: DependencyConfig,
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
        Self::get_path(std::env::current_dir().map_err(ConfigError::CurrentDir)?)
    }

    /// Finds the nearest `Simplex.toml` in the current directory or its ancestors.
    ///
    /// # Errors
    /// Returns a [`ConfigError`] if the current directory cannot be determined
    /// or no manifest is found.
    pub fn get_ancestors_manifest_path() -> Result<PathBuf, ConfigError> {
        let current_dir = Self::get_default_path()?;
        Self::resolve_manifest_path(&current_dir)
    }

    /// Returns the parent directory of the given manifest path.
    ///
    /// # Errors
    /// Returns a [`ConfigError`] if the path has no parent directory.
    pub fn get_project_root(manifest_path: impl AsRef<Path>) -> Result<PathBuf, ConfigError> {
        let manifest_path = manifest_path.as_ref();
        let root = manifest_path
            .parent()
            .ok_or_else(|| ConfigError::InvalidManifestPath(manifest_path.to_path_buf()))?;

        Ok(root.to_path_buf())
    }

    /// Constructs a complete configuration file path by joining the provided path with the
    /// predefined configuration file name [`CONFIG_FILENAME`].
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
        if let Some(esplora_config) = config.test.esplora.as_ref() {
            Self::validate_network(&esplora_config.network)?;

            if config.test.rpc.is_some() && esplora_config.network != "ElementsRegtest" {
                return Err(ConfigError::NetworkNameUnmatched(esplora_config.network.clone()));
            }
        }

        if let Some(network) = config.test.fuzz.as_ref().and_then(|fuzz| fuzz.network.as_deref()) {
            Self::validate_network(network)?;
        }

        Ok(())
    }

    fn validate_network(network: &str) -> Result<(), ConfigError> {
        let _ = network.parse::<SimplicityNetwork>()?;

        Ok(())
    }

    // Resolves manifest path by moving through ancestors.
    fn resolve_manifest_path(current_dir: impl AsRef<Path>) -> Result<PathBuf, ConfigError> {
        let current_dir = current_dir.as_ref();
        let manifest_path = {
            let path = current_dir
                .ancestors()
                .map(|directory| directory.join(CONFIG_FILENAME))
                .find(|candidate| candidate.is_file());

            if path.is_none() {
                return Err(ConfigError::FailedToFindManifest);
            }

            path.unwrap()
        };

        if manifest_path.file_name() != Some(OsStr::new(CONFIG_FILENAME)) {
            return Err(ConfigError::InvalidManifestPath(manifest_path));
        }

        if manifest_path.is_absolute() {
            Ok(manifest_path)
        } else {
            Ok(current_dir.join(manifest_path))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    #[test]
    fn discovers_manifest_in_current_directory() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");

        let manifest_path = Config::resolve_manifest_path(&project_root).expect("manifest should be discovered");

        assert_eq!(manifest_path, project_root.join(CONFIG_FILENAME));
    }

    #[test]
    fn discovers_manifest_from_nested_directory() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let nested_directory = project_root.join("simf/nested/nested_2/imp2");

        let manifest_path = Config::resolve_manifest_path(&nested_directory).expect("manifest should be discovered");

        assert_eq!(manifest_path, project_root.join(CONFIG_FILENAME));
    }

    #[test]
    fn discovers_nearest_manifest() {
        let fixtures_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let project_root = fixtures_root.join("deps/math");
        let nested_directory = project_root.join("simf");

        let manifest_path = Config::resolve_manifest_path(&nested_directory).expect("manifest should be discovered");

        assert_eq!(manifest_path, project_root.join(CONFIG_FILENAME));
    }
}
