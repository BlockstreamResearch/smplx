use serde::Deserialize;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, InlineTable, Item, Value};

use smplx_build::{BuildConfig, DependencyConfig, config::DEPENDENCIES_SECTION};
use smplx_regtest::RegtestConfig;
use smplx_test::TestConfig;

use crate::config::dep_spec::{DepSpec, Source};

use super::error::ConfigError;

pub const INIT_CONFIG: &str = include_str!("../../assets/Simplex.default.toml");
pub const CONFIG_FILENAME: &str = "Simplex.toml";

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

    /// Appends new entries to the `[dependencies]` table of the config file at `path`,
    /// preserving existing formatting and comments.
    ///
    /// # Errors
    /// - `ConfigError::MalformedDep`: If an entry in `deps` cannot be parsed as
    ///   `<source>` or `<alias>=<source>`.
    /// - `ConfigError::UnableToEdit`: If the file at `path` cannot be parsed as TOML
    ///   for editing.
    /// - `ConfigError::MalformedDependenciesTable`: If `[dependencies]` exists but
    ///   is not a TOML table.
    /// - `ConfigError::DuplicateAlias`: If an alias appears twice in `deps`, or
    ///   already exists in `[dependencies]`.
    /// - Any other I/O errors that may occur when reading or writing the file.
    pub fn add_dependency_to(path: &Path, deps: &[String]) -> Result<(), ConfigError> {
        if deps.is_empty() {
            return Ok(());
        }

        let specs: Vec<DepSpec> = deps
            .iter()
            .map(|raw| DepSpec::parse_dep(raw))
            .collect::<Result<_, _>>()?;

        let raw = std::fs::read_to_string(path)?;
        let mut doc: DocumentMut = raw.parse().map_err(|source| ConfigError::UnableToEdit {
            path: path.to_path_buf(),
            source,
        })?;

        let deps_table = doc
            .entry(DEPENDENCIES_SECTION)
            .or_insert(Item::Table(toml_edit::Table::new()))
            .as_table_mut()
            .ok_or(ConfigError::MalformedDependenciesTable)?;

        // Batches are small (typically <=10), so a linear scan over a Vec is cheaper
        // than the constant overhead of a HashSet.
        let mut seen_in_batch: Vec<&str> = Vec::with_capacity(specs.len());
        for spec in &specs {
            if seen_in_batch.contains(&spec.alias.as_str()) || deps_table.contains_key(&spec.alias) {
                return Err(ConfigError::DuplicateAlias(spec.alias.clone()));
            }
            seen_in_batch.push(spec.alias.as_str());
        }

        for spec in &specs {
            let mut inline = InlineTable::new();
            match &spec.source {
                Source::Git(url) => {
                    inline.insert("git", Value::from(url.as_str()));
                }
                Source::Path(p) => {
                    inline.insert("path", Value::from(p.as_str()));
                }
            }
            deps_table.insert(&spec.alias, Item::Value(Value::InlineTable(inline)));
        }

        std::fs::write(path, doc.to_string())?;
        println!("Added: {}", DepSpec::format_batch(&specs));

        Ok(())
    }

    fn validate(config: &Config) -> Result<(), ConfigError> {
        if let Some(esplora_config) = config.test.esplora.clone() {
            Self::validate_network(&esplora_config.network)?;

            if config.test.rpc.is_some() && esplora_config.network != "ElementsRegtest" {
                return Err(ConfigError::NetworkNameUnmatched(esplora_config.network.clone()));
            }
        }

        config.dependencies.validate()?;

        Ok(())
    }

    fn validate_network(network: &String) -> Result<(), ConfigError> {
        if network != "Liquid" && network != "LiquidTestnet" && network != "ElementsRegtest" {
            return Err(ConfigError::BadNetworkName(network.clone()));
        }

        Ok(())
    }
}
