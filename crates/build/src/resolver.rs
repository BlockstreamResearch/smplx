use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::path::{Path, PathBuf};
use std::{env, fs};

use globwalk::FileType;

use simplicityhl::parse::{self, ParseFromStr};
use simplicityhl::resolution::ValidatedDeps;
use simplicityhl::source::CanonPath;
use simplicityhl::str::FunctionName;

use crate::collector::DepCollector;
use crate::config::DEFAULT_DEPENDENCY_DIR;
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

        let config_source = fs::read_to_string(canon_root.as_path().join(config_filename))?;
        let root_src_dir = BuildConfig::from_source(&config_source)?.src_dir;
        let root_simf_dir = CanonPath::canonicalize(&canon_root.as_path().join(&root_src_dir))
            .map_err(BuildError::PathCanonicalization)?;

        // Flat install dir shared by every git dependency at any nesting depth,
        // mirroring `install`. Left un-canonicalized so pure-path projects
        // (which never create `deps/`) don't fail here.
        let deps_dir = PathBuf::from(DEFAULT_DEPENDENCY_DIR);

        let mut collector = DepCollector::new(config_filename.to_string(), deps_dir);

        collector.collect(deps_config, &canon_root, &root_simf_dir)
    }

    /// Converts "https://github.com/smplx/core.git"
    /// into a Cargo-style path: "core-a1b2c3d4e5f67890"
    ///
    /// # Returns
    ///
    /// - `Some(PathBuf)` when a repository name can be extracted from the URL.
    /// - `None` when the URL is empty or malformed such that no repository name
    ///   can be determined.
    pub fn generate_hashed_repo_path(url: &str) -> Option<PathBuf> {
        let clean_url = url.strip_suffix(".git").unwrap_or(url);
        let repo_name = clean_url.split('/').next_back()?;

        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        let hash_value = hasher.finish();

        // Do it the Rust way: EXACTLY 16 hex characters
        let dir_name = format!("{}-{:016x}", repo_name, hash_value);

        Some(PathBuf::from(dir_name))
    }

    /// Checks whether the source declares a `fn main(...)`,
    /// finding it even when nested inside `mod { ... }` blocks.
    fn contains_main(source: &str) -> bool {
        let Ok(parsed_program) = parse::Program::parse_from_str(source) else {
            return false;
        };

        Self::rec_main_checker(parsed_program.items(), &FunctionName::main())
    }

    /// Recursively searches `items` (descending into nested modules) for a
    /// function named `main`.
    fn rec_main_checker(items: &[parse::Item], main_name: &FunctionName) -> bool {
        items.iter().any(|item| match item {
            parse::Item::Function(func) => func.name() == main_name,
            parse::Item::Module(module) => Self::rec_main_checker(module.items(), main_name),
            _ => false,
        })
    }
}
