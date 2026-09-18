use std::collections::HashMap;
use std::path::Path;

use toml_edit::{DocumentMut, Item};

use serde::Deserialize;

use super::dep_spec::DepSpec;

use crate::error::{BuildError, DependencyValidationError, TomlEditError};

/// The default directory name used for Simplex project dependencies.
pub const DEFAULT_DEPENDENCY_DIR: &str = "deps";

// TOML section name.
pub const DEPENDENCIES_SECTION: &str = "dependencies";

#[derive(Debug, Default, Clone)]
pub struct DependencyConfig {
    pub inner: HashMap<String, Dependency>,
}

#[derive(Debug, Clone)]
pub enum Dependency {
    Path(String),
    Git { url: String, reference: Option<GitRef> },
}

#[derive(Debug, Clone)]
pub enum GitRef {
    Rev(String),
    Tag(String),
    Branch(String),
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawDependencyConfig {
    #[serde(flatten)]
    inner: HashMap<String, RawDependency>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDependency {
    /// The exact path to the directory containing the `Simplex.toml` file.
    path: Option<String>,
    /// The URL of the Git repository.
    git: Option<String>,
    /// The specific commit to download (only applicable if `git` is provided).
    rev: Option<String>,
    /// The specific tag to download (only applicable if `git` is provided).
    tag: Option<String>,
    /// The specific branch to download (only applicable if `branch` is provided)
    branch: Option<String>,
}

impl DependencyConfig {
    /// Parses and validates the `[dependencies]` section from TOML source text.
    ///
    /// The `[dependencies]` table is nested, so this descends into it rather than
    /// reading top-level keys. If the section is absent, returns the default
    /// (empty) config. Each dependency is validated to declare exactly one source.
    pub fn from_source(content: &str) -> Result<Self, BuildError> {
        let table: toml::Table = toml::from_str(content)?;

        match table.get(DEPENDENCIES_SECTION) {
            Some(section) => Ok(section.clone().try_into()?),
            None => Ok(Self::default()),
        }
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

        for spec in &specs {
            if deps_table.contains_key(&spec.alias) {
                return Err(TomlEditError::DuplicateAlias(spec.alias.clone()));
            }

            deps_table.insert(&spec.alias, Item::Value(spec.to_inline().into()));
        }

        std::fs::write(path, doc.to_string())?;

        println!("Added: {}", DepSpec::format_batch(&specs));

        Ok(())
    }
}

impl RawDependency {
    fn into_dependency(self, name: &str) -> Result<Dependency, DependencyValidationError> {
        match (self.path, self.git) {
            (Some(_), Some(_)) => Err(DependencyValidationError::Conflicting(name.into())),
            (None, None) => Err(DependencyValidationError::Missing(name.into())),
            (Some(p), None) => {
                if self.rev.is_some() || self.tag.is_some() || self.branch.is_some() {
                    return Err(DependencyValidationError::PathWithGitField(name.into()));
                }

                Ok(Dependency::Path(p))
            }
            (None, Some(url)) => {
                let reference = match (self.rev, self.tag, self.branch) {
                    (None, None, None) => None,
                    (Some(v), None, None) => Some(GitRef::Rev(Self::reject_dash_prefix(name, "rev", v)?)),
                    (None, Some(t), None) => Some(GitRef::Tag(Self::reject_dash_prefix(name, "tag", t)?)),
                    (None, None, Some(b)) => Some(GitRef::Branch(Self::reject_dash_prefix(name, "branch", b)?)),
                    _ => return Err(DependencyValidationError::ConflictingGitRef(name.into())),
                };

                let url = Self::reject_dash_prefix(name, "git", url)?;

                Ok(Dependency::Git { url, reference })
            }
        }
    }

    fn reject_dash_prefix(name: &str, field: &'static str, value: String) -> Result<String, DependencyValidationError> {
        if value.starts_with('-') {
            return Err(DependencyValidationError::DashPrefixed {
                name: name.into(),
                field,
                value,
            });
        }

        Ok(value)
    }
}

impl<'de> Deserialize<'de> for DependencyConfig {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = RawDependencyConfig::deserialize(d)?;
        let mut inner = HashMap::with_capacity(raw.inner.len());

        for (name, r) in raw.inner {
            let dep = r.into_dependency(&name).map_err(serde::de::Error::custom)?;
            inner.insert(name, dep);
        }

        Ok(Self { inner })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_and_git_references_are_loaded() {
        let config = DependencyConfig::from_source(
            r#"
                [dependencies]
                local = { path = "../local" }
                revision = { git = "https://example.com/revision.git", rev = "abc" }
                release = { git = "https://example.com/release.git", tag = "v1" }
                branch = { git = "https://example.com/branch.git", branch = "main" }
            "#,
        )
        .expect("dependency config should parse");

        assert!(matches!(config.inner.get("local"), Some(Dependency::Path(path)) if path == "../local"));
        assert!(
            matches!(config.inner.get("revision"), Some(Dependency::Git { reference: Some(GitRef::Rev(value)), .. }) if value == "abc")
        );
        assert!(
            matches!(config.inner.get("release"), Some(Dependency::Git { reference: Some(GitRef::Tag(value)), .. }) if value == "v1")
        );
        assert!(
            matches!(config.inner.get("branch"), Some(Dependency::Git { reference: Some(GitRef::Branch(value)), .. }) if value == "main")
        );
    }

    #[test]
    fn conflicting_dependency_sources_are_rejected() {
        let result = DependencyConfig::from_source(
            r#"
                [dependencies]
                broken = { path = "../local", git = "https://example.com/broken.git" }
            "#,
        );

        assert!(
            matches!(result, Err(BuildError::ConfigDeserialize(ref error)) if error.to_string().contains("cannot specify both 'path' and 'git'"))
        );
    }

    #[test]
    fn dependency_edit_preserves_existing_toml() {
        let path = std::env::temp_dir().join(format!("smplx-dependency-edit-{}.toml", std::process::id()));
        std::fs::write(&path, "# keep this comment\n[package]\nname = \"fixture\"\n")
            .expect("fixture should be writable");

        DependencyConfig::add_dependency_to(&path, &["local=../local".to_string()])
            .expect("dependency should be added");
        let content = std::fs::read_to_string(&path).expect("fixture should be readable");

        assert!(content.contains("# keep this comment"));
        assert!(content.contains("local = { path = \"../local\" }"));

        let _ = std::fs::remove_file(path);
    }
}
