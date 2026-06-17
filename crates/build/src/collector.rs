use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use simplicityhl::resolution::{DependencyMapBuilder, ValidatedDeps};
use simplicityhl::source::CanonPath;

use crate::config::Dependency;
use crate::{ArtifactsResolver, BuildConfig, DependencyConfig};

use super::error::BuildError;

/// A temporary context struct to hold global state during recursion.
/// This eliminates the need to pass `builder`, `visited`, and `config_filename`
/// into every single recursive call.
pub(crate) struct DepCollector {
    builder: DependencyMapBuilder,
    visited: HashSet<CanonPath>,
    config_filename: String,
    deps_dir: PathBuf,
}

impl DepCollector {
    pub(crate) fn new(config_filename: String, deps_dir: PathBuf) -> Self {
        Self {
            builder: DependencyMapBuilder::new(),
            visited: HashSet::new(),
            config_filename,
            deps_dir,
        }
    }

    pub(crate) fn collect(
        &mut self,
        deps_config: &DependencyConfig,
        root: &CanonPath,
        root_simf_dir: &CanonPath,
    ) -> Result<ValidatedDeps, BuildError> {
        self.visited.insert(root.clone());
        self.rec_collect(deps_config, root_simf_dir, root)?;

        self.builder
            .clone()
            .validate_deps()
            .map_err(|e| BuildError::DependencyMap(e.to_string()))
    }

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
            let loaded_context = self.resolve_dep_context(dep, context)?;

            let config_path = loaded_context.as_path().join(&self.config_filename);
            let config_source = fs::read_to_string(config_path)?;

            let loaded_src_dir = BuildConfig::from_source(&config_source)?.src_dir;
            let loaded_simf_dir = CanonPath::canonicalize(&loaded_context.as_path().join(loaded_src_dir))
                .map_err(BuildError::PathCanonicalization)?;

            self.builder
                .add_dependency(simf_dir.clone(), dep_name.clone(), loaded_simf_dir.clone());

            if !self.visited.insert(loaded_context.clone()) {
                continue;
            }

            let nested_deps = DependencyConfig::from_source(&config_source)?;

            self.rec_collect(&nested_deps, &loaded_simf_dir, &loaded_context)?;
        }

        Ok(())
    }

    /// Resolves the on-disk package root for a single dependency.
    ///
    /// - `path` deps resolve relative to the *parent* package (`context`).
    /// - `git` deps resolve into the flat root install dir (`self.deps_dir`).
    ///   reusing the exact hashed directory name produced by `install`.
    fn resolve_dep_context(&self, dep: &Dependency, context: &CanonPath) -> Result<CanonPath, BuildError> {
        let raw_path = match (&dep.path, &dep.git) {
            (Some(path), None) => context.as_path().join(path),
            (None, Some(git_url)) => {
                let hashed = ArtifactsResolver::generate_hashed_repo_path(git_url)
                    .ok_or_else(|| BuildError::InvalidGitUrl(git_url.clone()))?;
                self.deps_dir.join(hashed)
            }
            // `DependencyConfig::validate` guarantees exactly one of path/git is set.
            (Some(_), Some(_)) | (None, None) => {
                unreachable!("dependency source validated in 'DependencyConfig::validate'")
            }
        };

        CanonPath::canonicalize(&raw_path).map_err(BuildError::PathCanonicalization)
    }
}
