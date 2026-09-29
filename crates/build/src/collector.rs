use std::collections::HashSet;
use std::fs;
use std::path::Path;

use simplicityhl::resolution::{DependencyMapBuilder, ValidatedDeps};
use simplicityhl::source::CanonPath;

use crate::CONFIG_FILENAME;
use crate::{ArtifactsResolver, BuildConfig, DependencyConfig};

use super::error::BuildError;

/// A temporary context struct to hold global state during recursion.
/// This eliminates the need to pass `builder` and `visited`
/// into every single recursive call.
pub(crate) struct DepCollector {
    builder: DependencyMapBuilder,
    visited: HashSet<CanonPath>,
}

impl DepCollector {
    pub(crate) fn new() -> Self {
        Self {
            builder: DependencyMapBuilder::new(),
            visited: HashSet::new(),
        }
    }

    pub(crate) fn collect(
        &mut self,
        deps_config: &DependencyConfig,
        root: &CanonPath,
        root_simf_dir: &CanonPath,
        deps_dir: &Path,
    ) -> Result<ValidatedDeps, BuildError> {
        self.visited.insert(root.clone());
        self.rec_collect(deps_config, root_simf_dir, root, deps_dir)?;

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
        deps_dir: &Path,
    ) -> Result<(), BuildError> {
        for (dep_name, dep) in &deps_config.inner {
            let loaded_context = ArtifactsResolver::resolve_dep_context(dep, context, deps_dir)?;

            let config_path = loaded_context.as_path().join(CONFIG_FILENAME);
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

            self.rec_collect(&nested_deps, &loaded_simf_dir, &loaded_context, deps_dir)?;
        }

        Ok(())
    }
}
