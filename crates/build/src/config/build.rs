use serde::Deserialize;

use crate::error::BuildError;

// Default values for optional [build] fields.
pub(super) const DEFAULT_OUT_DIR_NAME: &str = "src/artifacts";
pub(super) const DEFAULT_INCLUDE_PATH: &str = "**/*.simf";
pub(super) const DEFAULT_SRC_DIR_NAME: &str = "simf";

// TOML section name.
pub(super) const BUILD_SECTION: &str = "build";

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BuildConfig {
    pub simf_files: Vec<String>,
    pub src_dir: String,
    pub out_dir: String,
}

impl BuildConfig {
    /// Parses the `[build]` section from TOML source text.
    ///
    /// The `[build]` table is nested, so this descends into it rather than reading
    /// top-level keys. If the section is absent, returns [`BuildConfig::default`].
    pub fn from_source(content: &str) -> Result<Self, BuildError> {
        let table: toml::Table = toml::from_str(content)?;

        match table.get(BUILD_SECTION) {
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
