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
