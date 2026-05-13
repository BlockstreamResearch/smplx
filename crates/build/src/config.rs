use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use super::error::BuildError;

pub const DEFAULT_OUT_DIR_NAME: &str = "src/artifacts";
pub const DEFAULT_INCLUDE_PATH: &str = "**/*.simf";
pub const DEFAULT_SRC_DIR_NAME: &str = "simf";

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BuildConfig {
    pub simf_files: Vec<String>,
    pub src_dir: String,
    pub out_dir: String,
}

impl BuildConfig {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, BuildError> {
        let mut content = String::new();
        let mut file = OpenOptions::new().read(true).open(path)?;

        file.read_to_string(&mut content)?;

        Ok(toml::from_str(&content)?)
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

/// It will be located inside the `src_dir`
pub const DEFAULT_DEPENDENCY_DIR: &str = "deps";

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct DependencyConfig {
    #[serde(flatten)]
    pub inner: HashMap<String, Dependency>,
}

impl DependencyConfig {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, BuildError> {
        let mut content = String::new();
        let mut file = OpenOptions::new().read(true).open(path)?;

        file.read_to_string(&mut content)?;

        let res: Self = toml::from_str(&content)?;
        if let Err(err) = res.validate() {
            Err(BuildError::InvalidDependency(err))
        } else {
            Ok(res)
        }
    }

    /// When error occured, returned, return drp_name of the invalid dependency
    pub fn validate(&self) -> Result<(), String> {
        for (drp_name, dep) in &self.inner {
            if dep.path.is_none() && dep.git.is_none() {
                return Err(drp_name.clone());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Dependency {
    pub path: Option<String>,
    pub git: Option<String>,
}
