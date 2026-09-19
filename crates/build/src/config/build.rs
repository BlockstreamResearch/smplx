use serde::Deserialize;

use crate::error::BuildError;

// Default values for optional [build] fields.
pub const DEFAULT_OUT_DIR_NAME: &str = "src/artifacts";
pub const DEFAULT_INCLUDE_PATH: &str = "**/*.simf";
pub const DEFAULT_SRC_DIR_NAME: &str = "simf";

// TOML section name.
pub const BUILD_SECTION: &str = "build";

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_build_values_are_loaded() {
        let config = BuildConfig::from_source(
            r#"
                [build]
                src_dir = "contracts"
                simf_files = ["one.simf", "nested/*.simf"]
                out_dir = "generated"
            "#,
        )
        .expect("explicit build config should parse");

        assert_eq!(config.src_dir, "contracts");
        assert_eq!(config.simf_files, ["one.simf", "nested/*.simf"]);
        assert_eq!(config.out_dir, "generated");
    }
}
