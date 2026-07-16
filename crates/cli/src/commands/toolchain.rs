use std::path::{Path, PathBuf};

use serde::Deserialize;
use smplx_build::deps::DepSources;
use smplx_build::error::BuildError;
use smplx_build::version::{self, Version};
use smplx_build::{ArtifactsResolver, BuildConfig, DependencyConfig};
use smplx_sdk::compiler;
use smplx_sdk::compiler::lockfile;

use crate::config::CONFIG_FILENAME;

use super::error::{CommandError, ToolchainError};

pub struct Toolchain;

impl Toolchain {
    /// Resolves the project's directives against the published releases, provisions
    /// that `simc`, and pins it in `simplex.lock`.
    ///
    /// # Errors
    /// Returns a [`CommandError`] if directives cannot be read, resolution fails, or
    /// the download fails.
    pub fn run(build: &BuildConfig, deps: &DependencyConfig) -> Result<(), CommandError> {
        let dep_sources = if deps.inner.is_empty() {
            DepSources::default()
        } else {
            ArtifactsResolver::resolve_remappings(deps, CONFIG_FILENAME)?
        };
        let requirements = read_requirements(build, &dep_sources)?;
        if requirements.is_empty() {
            println!("No `simc` version directives found — resolving the newest release.");
        } else {
            println!("Declared compiler requirements:");
            for req in &requirements {
                println!("  {} -> {}", display(&req.path), req.requirement.req());
            }
        }

        let (selected, simc) = resolve_and_provision(&requirements)?;
        println!("Resolved compiler version: {selected}");

        lockfile::write(&std::env::current_dir()?, &selected.to_string(), &simc).map_err(ToolchainError::from)?;
        println!("Ready: {}; pinned in {}", simc.display(), lockfile::LOCKFILE);
        Ok(())
    }

    /// Lists installed compilers, annotating the one the nearest `simplex.lock` pins.
    ///
    /// # Errors
    /// Returns a [`CommandError`] if the store root cannot be located.
    pub fn list() -> Result<(), CommandError> {
        let store = compiler::compilers_dir().map_err(ToolchainError::from)?;
        let versions = compiler::installed_versions().map_err(ToolchainError::from)?;

        println!("Compiler store: {}", store.display());
        if versions.is_empty() {
            println!("  (empty — run `simplex toolchain` in a project, or `simplex toolchain install <version>`)");
            return Ok(());
        }

        let lock = std::env::current_dir().ok().and_then(|cwd| lockfile::nearest(&cwd));
        for version in versions {
            let simc = compiler::simc_path(&version).map_err(ToolchainError::from)?;
            let size = std::fs::metadata(&simc).map_or(0, |m| m.len());

            let mut notes = Vec::new();
            if let Some(lock) = lock.as_ref().filter(|lock| lock.version == version) {
                notes.push(format!("pinned by {}", lockfile::LOCKFILE));
                if let Some(expected) = lock.sha256_for_host() {
                    match compiler::sha256_hex(&simc) {
                        Ok(actual) if actual == expected => notes.push("hash verified".to_string()),
                        Ok(_) => notes.push("HASH MISMATCH — `simplex toolchain remove` and re-provision".to_string()),
                        Err(e) => notes.push(format!("unreadable: {e}")),
                    }
                }
            }

            let notes = if notes.is_empty() {
                String::new()
            } else {
                format!("  [{}]", notes.join(", "))
            };
            #[allow(clippy::cast_precision_loss)]
            let mib = size as f64 / (1024.0 * 1024.0);
            println!("  {version:<12} {mib:>6.1} MiB  {}{notes}", simc.display());
        }
        Ok(())
    }

    /// Installs an exact compiler version into the store without touching
    /// `simplex.lock` — pinning stays an explicit, separate decision.
    ///
    /// # Errors
    /// Returns a [`CommandError`] if the download or hash verification fails.
    pub fn install(version: &str) -> Result<(), CommandError> {
        let expected = std::env::current_dir()
            .ok()
            .and_then(|cwd| lockfile::nearest(&cwd))
            .filter(|lock| lock.version == version)
            .and_then(|lock| lock.sha256_for_host());
        let simc = compiler::provision(version, expected.as_deref()).map_err(ToolchainError::from)?;
        println!("Installed simc {version} at {}", simc.display());

        if let Some(lock) = std::env::current_dir().ok().and_then(|cwd| lockfile::nearest(&cwd))
            && lock.version != version
        {
            println!(
                "note: {} pins {}, so `simplex build` keeps using that; this install only populates the store.",
                lockfile::LOCKFILE,
                lock.version
            );
        }
        Ok(())
    }

    /// Removes an installed compiler; the store is a cache, so anything still
    /// pinned is re-downloaded and verified on the next build.
    ///
    /// # Errors
    /// Returns a [`CommandError`] if the directory cannot be located or deleted.
    pub fn remove(version: &str) -> Result<(), CommandError> {
        let dir = compiler::compilers_dir().map_err(ToolchainError::from)?.join(version);
        if !dir.join("bin").join("simc").is_file() {
            println!("Compiler {version} is not installed.");
            return Ok(());
        }
        std::fs::remove_dir_all(&dir)?;
        println!("Removed simc {version} ({})", dir.display());

        if std::env::current_dir()
            .ok()
            .and_then(|cwd| lockfile::nearest(&cwd))
            .is_some_and(|lock| lock.version == version)
        {
            println!(
                "note: {} still pins {version}; the next `simplex build` will re-download and verify it.",
                lockfile::LOCKFILE
            );
        }
        Ok(())
    }

    /// Prints the compiler store directory (`SIMPLEX_COMPILERS` or the default).
    ///
    /// # Errors
    /// Returns a [`CommandError`] if the store root cannot be located.
    pub fn dir() -> Result<(), CommandError> {
        println!("{}", compiler::compilers_dir().map_err(ToolchainError::from)?.display());
        Ok(())
    }
}

/// Resolves `requirements` against the published releases and provisions the
/// selected `simc`, enforcing any hash `simplex.lock` already records for it.
///
/// # Errors
/// Returns a [`CommandError`] if the release index is unreachable, no release
/// satisfies the requirements, or the download fails.
pub fn resolve_and_provision(requirements: &[version::FileRequirement]) -> Result<(Version, PathBuf), CommandError> {
    let candidates = fetch_versions()?;
    let selected =
        version::resolve(requirements, &candidates).map_err(|err| ToolchainError::Resolve(err.to_string()))?;
    let expected = std::env::current_dir()
        .ok()
        .and_then(|cwd| lockfile::read_at(&cwd))
        .filter(|lock| lock.version == selected.to_string())
        .and_then(|lock| lock.sha256_for_host());
    let simc = compiler::provision(&selected.to_string(), expected.as_deref()).map_err(ToolchainError::from)?;
    Ok((selected, simc))
}

/// The directives declared by the project's `.simf` files and every dependency
/// file — the pinned compiler compiles them all.
///
/// # Errors
/// Returns a [`CommandError`] on an unreadable source tree or a malformed directive.
pub fn read_requirements(
    build: &BuildConfig,
    deps: &DepSources,
) -> Result<Vec<version::FileRequirement>, CommandError> {
    let src_dir = std::env::current_dir()?.join(&build.src_dir);
    let mut requirements = version::collect_requirements(&src_dir, &build.simf_files)?;
    for file in &deps.files {
        let requirement =
            version::requirement_of(&file.contents).map_err(|error| BuildError::InvalidSimcDirective {
                file: file.real_path.clone(),
                error,
            })?;
        if let Some(requirement) = requirement {
            requirements.push(version::FileRequirement {
                path: file.real_path.clone(),
                requirement,
            });
        }
    }
    Ok(requirements)
}

#[derive(Deserialize)]
struct Tag {
    name: String,
}

/// The versions published as `simplicityhl-<version>` release tags. Set
/// `GITHUB_TOKEN`/`GH_TOKEN` — anonymous requests share a small per-IP rate limit.
fn fetch_versions() -> Result<Vec<Version>, ToolchainError> {
    let repo = compiler::source_repo().ok_or_else(|| {
        ToolchainError::Index(format!(
            "{}: only github.com release sources are supported",
            compiler::RELEASE_SOURCE
        ))
    })?;
    let token = ["GITHUB_TOKEN", "GH_TOKEN"]
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .filter(|token| !token.trim().is_empty());

    let mut versions: Vec<Version> = Vec::new();
    for page in 1..=10 {
        let api = format!("https://api.github.com/repos/{repo}/tags?per_page=100&page={page}");
        let mut request = minreq::get(&api)
            .with_header("User-Agent", "smplx-cli")
            .with_header("Accept", "application/vnd.github+json")
            .with_timeout(30);
        if let Some(token) = &token {
            request = request.with_header("Authorization", format!("Bearer {token}"));
        }
        let response = request
            .send()
            .map_err(|e| ToolchainError::Index(format!("{api}: {e}")))?;
        if response.status_code != 200 {
            return Err(ToolchainError::Index(format!("{api}: HTTP {}", response.status_code)));
        }

        let tags: Vec<Tag> =
            serde_json::from_slice(response.as_bytes()).map_err(|e| ToolchainError::Index(format!("{api}: {e}")))?;
        let full_page = tags.len() == 100;

        versions.extend(
            tags.iter()
                .filter_map(|tag| tag.name.strip_prefix(compiler::TAG_PREFIX))
                .filter_map(|v| Version::parse(v).ok()),
        );
        if !full_page {
            break;
        }
    }
    versions.sort();
    versions.dedup();

    if versions.is_empty() {
        return Err(ToolchainError::NoReleases(compiler::RELEASE_SOURCE.to_string()));
    }
    Ok(versions)
}

/// A cwd-relative form of a path for reporting.
fn display(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok())
        .unwrap_or(path)
        .display()
        .to_string()
}
