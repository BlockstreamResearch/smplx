use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use globwalk::FileType;
use regex::Regex;

use simplicityhl::resolution::{DependencyMapBuilder, ValidatedDeps};
use simplicityhl::source::CanonPath;

use crate::{BuildConfig, DependencyConfig};

use super::error::BuildError;

pub struct ArtifactsResolver {}

impl ArtifactsResolver {
    pub fn resolve_files_to_build(src_dir: &String, simfs: &[String]) -> Result<Vec<PathBuf>, BuildError> {
        let cwd = env::current_dir()?;
        let base = cwd.join(src_dir);

        let mut paths = Vec::new();

        let walker = globwalk::GlobWalkerBuilder::from_patterns(base, simfs)
            .follow_links(true)
            .file_type(FileType::FILE)
            .build()?
            .filter_map(Result::ok);

        for img in walker {
            let path = img.path().to_path_buf().canonicalize()?;
            let content = std::fs::read_to_string(&path)?;

            if Self::contains_main(&content) {
                paths.push(path);
            }
        }

        Ok(paths)
    }

    pub fn resolve_local_dir(path: &impl AsRef<Path>) -> Result<PathBuf, BuildError> {
        let mut path_outer = PathBuf::from(path.as_ref());

        if !path_outer.is_absolute() {
            let manifest_dir = env::current_dir()?;

            let mut path_local = manifest_dir;
            path_local.push(path_outer);

            path_outer = path_local;
        }

        if path_outer.extension().is_some() {
            return Err(BuildError::GenerationPath(format!(
                "Directories can't have an extension, path: '{}'",
                path_outer.display()
            )));
        }

        if path_outer.is_file() {
            return Err(BuildError::GenerationPath(format!(
                "Directory can't be a path, path: '{}'",
                path_outer.display()
            )));
        }

        // TODO: canonicalize? but this path may not exist
        Ok(path_outer)
    }

    /// Builds a [`ValidatedDeps`] by recursively walking the dependency tree
    /// starting from the current working directory.
    ///
    /// Each dependency may have its own config file declaring further dependencies.
    /// Those are registered with their own directory as the context, so that
    /// `crate::` and sibling imports resolve correctly relative to each package root.
    pub fn resolve_remappings(
        deps_config: &DependencyConfig,
        config_filename: &str,
    ) -> Result<ValidatedDeps, BuildError> {
        let root_dir = env::current_dir()?;
        let canon_root = CanonPath::canonicalize(&root_dir).map_err(BuildError::PathCanonicalization)?;

        let root_build_cfg = BuildConfig::from_file(canon_root.as_path().join(config_filename))?;
        let root_simf_dir = CanonPath::canonicalize(&canon_root.as_path().join(&root_build_cfg.src_dir))
            .map_err(BuildError::PathCanonicalization)?;

        let mut collector = DepCollector {
            builder: DependencyMapBuilder::new(),
            visited: HashSet::new(),
            config_filename: config_filename.to_string(),
        };
        collector.visited.insert(canon_root.clone());

        collector.rec_collect(deps_config, &root_simf_dir, &canon_root)?;

        collector
            .builder
            .validate_deps()
            .map_err(|e| BuildError::DependencyMap(e.to_string()))
    }

    /// Checks whether the source contains a `fn main(...)` declaration,
    /// regardless of visibility (`pub fn main` or `fn main`) or nesting
    /// inside `mod { ... }` blocks; The regex matches anywhere in the text.
    fn contains_main(source: &str) -> bool {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| Regex::new(r"(?m)^\s*(pub\s+)?fn\s+main\s*\(").unwrap());

        re.is_match(source)
    }
}

/// A temporary context struct to hold global state during recursion.
/// This eliminates the need to pass `builder`, `visited`, and `config_filename`
/// into every single recursive call.
struct DepCollector {
    builder: DependencyMapBuilder,
    visited: HashSet<CanonPath>,
    config_filename: String,
}

impl DepCollector {
    /// Recursively registers each dependency's `simf` directory under its parent context,
    /// then recurses into the dependency's own config to discover transitive dependencies.
    ///
    /// # Example
    ///
    /// Given the dependency graph:
    /// ```text
    /// root -> A -> B
    /// root -> B
    /// ```
    ///
    /// Processing proceeds as follows:
    ///
    /// 1. Starting at `root`, register `A` as a dependency under `root`'s context,
    ///    then mark `root` as visited and recurse into `A`.
    /// 2. Inside `A`, register `B` as a dependency under `A`'s context, mark `A` as
    ///    visited, and recurse into `B`.
    /// 3. Inside `B`, `deps_config` is empty, so nothing is registered — recursion
    ///    simply returns.
    /// 4. Back at `root`, register `B` as a dependency under `root`'s context as well.
    ///    Note that the dependency is registered *before* checking `visited` — `root`
    ///    must record its own link to `B`, even though `B` itself was already visited
    ///    and does not need to be recursed into again.
    fn rec_collect(
        &mut self,
        deps_config: &DependencyConfig,
        simf_dir: &CanonPath,
        context: &CanonPath,
    ) -> Result<(), BuildError> {
        for (dep_name, dep) in &deps_config.inner {
            let Some(dep_path_str) = dep.path.as_ref() else {
                // TODO: git support
                continue;
            };

            let loaded_context = CanonPath::canonicalize(&context.as_path().join(dep_path_str))
                .map_err(BuildError::PathCanonicalization)?;

            // TODO: This code could be further optimized by using a `from_str` method inside `BuildConfig` and `DependencyConfig`
            let config_path = loaded_context.as_path().join(&self.config_filename);

            let loaded_src_dir = BuildConfig::from_file(&config_path)?.src_dir;
            let loaded_simf_dir = CanonPath::canonicalize(&loaded_context.as_path().join(loaded_src_dir))
                .map_err(BuildError::PathCanonicalization)?;

            self.builder
                .add_dependency(simf_dir.clone(), dep_name.clone(), loaded_simf_dir.clone());

            if !self.visited.insert(loaded_context.clone()) {
                continue;
            }

            let nested_deps = DependencyConfig::from_file(&config_path)?;
            self.rec_collect(&nested_deps, &loaded_simf_dir, &loaded_context)?;
        }

        Ok(())
    }
}
