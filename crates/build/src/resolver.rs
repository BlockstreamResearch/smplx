use std::fs;
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::path::{Component, Path, PathBuf};

#[cfg(feature = "full")]
use globwalk::FileType;

#[cfg(feature = "full")]
use simplicityhl::parse::{self, ParseFromStr};
use simplicityhl::resolution::ValidatedDeps;
use simplicityhl::source::CanonPath;
#[cfg(feature = "full")]
use simplicityhl::str::FunctionName;

use crate::collector::DepCollector;
use crate::config::{DEFAULT_DEPENDENCY_DIR, Dependency, GitRef};
use crate::{BuildConfig, CONFIG_FILENAME, DependencyConfig};

use super::error::BuildError;

const BASE58_ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Base58 digits needed for any `u64`.
const BASE58_U64_LEN: usize = 11;

pub struct ArtifactsResolver {}

impl ArtifactsResolver {
    #[cfg(feature = "full")]
    pub fn resolve_files_to_build(
        root_dir: &Path,
        src_dir: &String,
        simfs: &[String],
    ) -> Result<Vec<PathBuf>, BuildError> {
        let base = root_dir.join(src_dir);

        let mut paths = Vec::new();

        let walker = globwalk::GlobWalkerBuilder::from_patterns(base, simfs)
            .follow_links(true)
            .file_type(FileType::FILE)
            .build()?
            .filter_map(Result::ok);

        for img in walker {
            let path = img.path().to_path_buf().canonicalize()?;
            let content = std::fs::read_to_string(&path)?;

            match parse::Program::parse_from_str(&content) {
                Ok(program) if Self::rec_main_checker(program.items(), &FunctionName::main()) => paths.push(path),
                Ok(_) => {}
                Err(error) => eprintln!("warning: skipping '{}' as it doesn't parse: {error}", path.display()),
            }
        }

        Ok(paths)
    }

    pub fn resolve_local_dir(root_dir: &Path, path: &impl AsRef<Path>) -> Result<PathBuf, BuildError> {
        // `join` keeps `path` as is when it is already absolute
        let path_outer = root_dir.join(path);

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

    /// Resolves `out_dir` like [`Self::resolve_local_dir`], refusing one that holds the project or its sources.
    pub fn resolve_out_dir(
        root_dir: &Path,
        out_dir: &impl AsRef<Path>,
        src_dir: &impl AsRef<Path>,
    ) -> Result<PathBuf, BuildError> {
        let resolved = Self::resolve_local_dir(root_dir, out_dir)?;

        let out = Self::normalize(&resolved);
        let root = Self::normalize(root_dir);
        let src = Self::normalize(&root_dir.join(src_dir));

        if root.starts_with(&out) || src.starts_with(&out) {
            return Err(BuildError::OutDirHoldsSources(resolved));
        }

        Ok(resolved)
    }

    /// Builds a [`ValidatedDeps`] by recursively walking the dependency tree
    /// starting from the current working directory.
    ///
    /// Each dependency may have its own config file declaring further dependencies.
    /// Those are registered with their own directory as the context, so that
    /// `crate::` and sibling imports resolve correctly relative to each package root.
    pub fn resolve_remappings(root_dir: &Path, deps_config: &DependencyConfig) -> Result<ValidatedDeps, BuildError> {
        let canon_root = CanonPath::canonicalize(root_dir).map_err(BuildError::PathCanonicalization)?;

        let config_source = fs::read_to_string(canon_root.as_path().join(CONFIG_FILENAME))?;
        let root_src_dir = BuildConfig::from_source(&config_source)?.src_dir;
        let root_simf_dir = CanonPath::canonicalize(&canon_root.as_path().join(&root_src_dir))
            .map_err(BuildError::PathCanonicalization)?;

        // Flat install dir shared by every git dependency at any nesting depth,
        // mirroring `install`. Left un-canonicalized so pure-path projects
        // (which never create `deps/`) don't fail here.
        let deps_dir = canon_root.as_path().join(DEFAULT_DEPENDENCY_DIR);

        DepCollector::new().collect(deps_config, &canon_root, &root_simf_dir, &deps_dir)
    }

    /// Resolves the on-disk package root for a single dependency.
    ///
    /// - `path` dependencies resolve relative to the parent package (`context`).
    /// - `git` dependencies resolve into the flat install dir (`deps_dir`), using the same
    ///   hashed directory name `install` creates. `deps` dir is unused for `path` dependencies.
    pub fn resolve_dep_context(
        dep: &Dependency,
        context: &CanonPath,
        deps_dir: &Path,
    ) -> Result<CanonPath, BuildError> {
        let raw_path = match dep {
            Dependency::Path(path) => context.as_path().join(path),
            Dependency::Git {
                url,
                reference,
                package,
            } => {
                let hashed = ArtifactsResolver::generate_hashed_repo_path(url, reference.as_ref(), package.as_deref())
                    .ok_or_else(|| BuildError::InvalidGitUrl(url.clone()))?;
                deps_dir.join(hashed)
            }
        };

        CanonPath::canonicalize(&raw_path).map_err(BuildError::PathCanonicalization)
    }

    /// Converts "https://github.com/smplx/core.git"
    /// into a Cargo-style path: "core-5HueCGU8rMj" (11 base58 characters)
    ///
    /// # Returns
    ///
    /// - `Some(PathBuf)` when a repository name can be extracted from the URL.
    /// - `None` when the URL is empty or malformed such that no repository name
    ///   can be determined.
    pub fn generate_hashed_repo_path(url: &str, reference: Option<&GitRef>, package: Option<&str>) -> Option<PathBuf> {
        let clean_url = url.strip_suffix(".git").unwrap_or(url);
        let repo_name = clean_url.split('/').next_back()?;

        // Only fields that are actually set take part in the key, so adding a new
        // optional field never changes the directory of dependencies that don't use it.
        let reference = reference.map(|reference| match reference {
            GitRef::Rev(rev) => format!("rev={rev}"),
            GitRef::Tag(tag) => format!("tag={tag}"),
            GitRef::Branch(branch) => format!("branch={branch}"),
        });
        let package = package.map(|package| format!("package={package}"));

        let key = std::iter::once(url.to_owned())
            .chain(reference)
            .chain(package)
            .collect::<Vec<_>>();

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash_value = hasher.finish();

        let dir_name = format!("{}-{}", repo_name, Self::encode_base58(hash_value));

        Some(PathBuf::from(dir_name))
    }

    /// Resolves `.` and `..` without touching the filesystem, as `path` may not exist yet.
    fn normalize(path: &Path) -> PathBuf {
        let mut normalized = PathBuf::new();

        for component in path.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    normalized.pop();
                }
                other => normalized.push(other),
            }
        }

        normalized
    }

    /// Encodes `value` in base58, left-padded to a fixed [`BASE58_U64_LEN`] characters.
    fn encode_base58(mut value: u64) -> String {
        let mut digits = [BASE58_ALPHABET[0]; BASE58_U64_LEN];

        for digit in digits.iter_mut().rev() {
            *digit = BASE58_ALPHABET[(value % 58) as usize];
            value /= 58;
        }

        digits.iter().map(|&digit| char::from(digit)).collect()
    }

    #[cfg(feature = "full")]
    /// Recursively searches `items` (descending into nested modules) for a
    /// function named `main`, finding it even when nested inside `mod { ... }` blocks.
    fn rec_main_checker(items: &[parse::Item], main_name: &FunctionName) -> bool {
        items.iter().any(|item| match item {
            parse::Item::Function(func) => func.name() == main_name,
            parse::Item::Module(module) => Self::rec_main_checker(module.items(), main_name),
            _ => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_dir_cannot_hold_the_project_or_its_sources() {
        let root = Path::new("/project");

        assert!(ArtifactsResolver::resolve_out_dir(root, &"src/artifacts", &"simf").is_ok());

        for out_dir in ["", ".", "..", "simf", "./simf/../simf", "/"] {
            assert!(
                ArtifactsResolver::resolve_out_dir(root, &out_dir, &"simf").is_err(),
                "{out_dir}"
            );
        }
        assert!(ArtifactsResolver::resolve_out_dir(root, &"contracts", &"contracts/simf").is_err());
    }
}
