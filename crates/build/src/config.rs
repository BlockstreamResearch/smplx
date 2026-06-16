use std::collections::HashMap;

use serde::Deserialize;

use super::error::BuildError;
use super::error::DependencyValidationError;

pub const DEFAULT_OUT_DIR_NAME: &str = "src/artifacts";
pub const DEFAULT_INCLUDE_PATH: &str = "**/*.simf";
pub const DEFAULT_SRC_DIR_NAME: &str = "simf";
pub const DEFAULT_DEPENDENCY_DIR: &str = "deps";

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BuildConfig {
    pub simf_files: Vec<String>,
    pub src_dir: String,
    pub out_dir: String,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct DependencyConfig {
    #[serde(flatten)]
    pub inner: HashMap<String, Dependency>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Dependency {
    /// Exact path to dir, where `Simplex.toml` file was located
    pub path: Option<String>,

    /// Link to git repo
    pub git: Option<String>,
}

impl BuildConfig {
    /// Parses the `[build]` section from TOML source text.
    ///
    /// The `[build]` table is nested, so this descends into it rather than reading
    /// top-level keys. If the section is absent, returns [`BuildConfig::default`].
    pub fn from_source(content: &str) -> Result<Self, BuildError> {
        let table: toml::Table = toml::from_str(content)?;
        match table.get("build") {
            Some(section) => Ok(section.clone().try_into()?),
            None => Ok(Self::default()),
        }
    }
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            simf_files: vec![DEFAULT_INCLUDE_PATH.into()],
            src_dir: DEFAULT_SRC_DIR_NAME.into(),
            out_dir: DEFAULT_OUT_DIR_NAME.into(),
        }
    }
}

impl DependencyConfig {
    /// Parses and validates the `[dependencies]` section from TOML source text.
    ///
    /// The `[dependencies]` table is nested, so this descends into it rather than
    /// reading top-level keys. If the section is absent, returns the default
    /// (empty) config. Each dependency is validated to declare exactly one source.
    pub fn from_source(content: &str) -> Result<Self, BuildError> {
        let table: toml::Table = toml::from_str(content)?;
        let res: Self = match table.get("dependencies") {
            Some(section) => section.clone().try_into()?,
            None => Self::default(),
        };
        res.validate()?;
        Ok(res)
    }

    pub fn validate(&self) -> Result<(), DependencyValidationError> {
        for (drp_name, dep) in &self.inner {
            match (&dep.path, &dep.git) {
                (None, None) => return Err(DependencyValidationError::Missing(drp_name.clone())),
                (Some(_), Some(_)) => return Err(DependencyValidationError::Conflicting(drp_name.clone())),
                _ => {}
            }
        }
        Ok(())
    }
}
