use std::collections::HashMap;
use std::path::Path;

use toml_edit::{DocumentMut, InlineTable, Item, Value};

use serde::Deserialize;

use crate::dep_spec::DepSpec;
use crate::dep_spec::Source;
use crate::error::TomlEditError;

use super::error::BuildError;
use super::error::DependencyValidationError;

// Default values for optional [build] fields.
pub const DEFAULT_OUT_DIR_NAME: &str = "src/artifacts";
pub const DEFAULT_INCLUDE_PATH: &str = "**/*.simf";
pub const DEFAULT_SRC_DIR_NAME: &str = "simf";
pub const DEFAULT_DEPENDENCY_DIR: &str = "deps";

// TOML section names.
pub const BUILD_SECTION: &str = "build";
pub const DEPENDENCIES_SECTION: &str = "dependencies";

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

impl DependencyConfig {
    /// Parses and validates the `[dependencies]` section from TOML source text.
    ///
    /// The `[dependencies]` table is nested, so this descends into it rather than
    /// reading top-level keys. If the section is absent, returns the default
    /// (empty) config. Each dependency is validated to declare exactly one source.
    pub fn from_source(content: &str) -> Result<Self, BuildError> {
        let table: toml::Table = toml::from_str(content)?;
        let res: Self = match table.get(DEPENDENCIES_SECTION) {
            Some(section) => section.clone().try_into()?,
            None => Self::default(),
        };

        res.validate()?;

        Ok(res)
    }

    /// Appends new entries to the `[dependencies]` table of the config file at `path`,
    /// preserving existing formatting and comments.
    ///
    /// # Errors
    /// - `TomlEditError::MalformedDep`: If an entry in `deps` cannot be parsed as
    ///   `<source>` or `<alias>=<source>`.
    /// - `TomlEditError::UnableToEdit`: If the file at `path` cannot be parsed as TOML
    ///   for editing.
    /// - `TomlEditError::MalformedDependenciesTable`: If `[dependencies]` exists but
    ///   is not a TOML table.
    /// - `TomlEditError::DuplicateAlias`: If an alias appears twice in `deps`, or
    ///   already exists in `[dependencies]`.
    /// - Any other I/O errors that may occur when reading or writing the file.
    pub fn add_dependency_to(path: &Path, deps: &[String]) -> Result<(), TomlEditError> {
        if deps.is_empty() {
            return Ok(());
        }

        let specs: Vec<DepSpec> = deps
            .iter()
            .map(|raw| DepSpec::parse_dep(raw))
            .collect::<Result<_, _>>()?;

        let raw = std::fs::read_to_string(path)?;
        let mut doc: DocumentMut = raw.parse().map_err(|source| TomlEditError::UnableToEdit {
            path: path.to_path_buf(),
            source,
        })?;

        let deps_table = doc
            .entry(DEPENDENCIES_SECTION)
            .or_insert(Item::Table(toml_edit::Table::new()))
            .as_table_mut()
            .ok_or(TomlEditError::MalformedDependenciesTable)?;

        // Batches are small (typically <=10), so a linear scan over a Vec is cheaper
        // than the constant overhead of a HashSet.
        let mut seen_in_batch: Vec<&str> = Vec::with_capacity(specs.len());
        for spec in &specs {
            if seen_in_batch.contains(&spec.alias.as_str()) || deps_table.contains_key(&spec.alias) {
                return Err(TomlEditError::DuplicateAlias(spec.alias.clone()));
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

    pub fn validate(&self) -> Result<(), DependencyValidationError> {
        for (dep_name, dep) in &self.inner {
            match (&dep.path, &dep.git) {
                (None, None) => return Err(DependencyValidationError::Missing(dep_name.clone())),
                (Some(_), Some(_)) => return Err(DependencyValidationError::Conflicting(dep_name.clone())),
                _ => {}
            }
        }

        Ok(())
    }
}
