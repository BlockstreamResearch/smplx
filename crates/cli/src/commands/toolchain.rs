use std::path::{Path, PathBuf};

use serde::Deserialize;
use smplx_build::BuildConfig;
use smplx_build::version::{self, Version};
use smplx_sdk::compiler;
use smplx_sdk::compiler::lockfile;

use super::error::{CommandError, ToolchainError};

pub struct Toolchain;

impl Toolchain {
    /// Resolves the version this project's directives require against the published
    /// releases, downloads and verifies that `simc`, and updates `simplex.lock`. This
    /// is the explicit resolve/update path; a satisfying lock keeps `simplex build` off
    /// the network.
    ///
    /// # Errors
    /// Returns a [`CommandError`] if directives cannot be read, the release index is
    /// unreachable, no release satisfies the directives, or the download fails.
    pub fn run(build: &BuildConfig) -> Result<(), CommandError> {
        let requirements = read_requirements(build)?;
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

    /// Lists the compilers installed in the store, annotating the one the nearest
    /// `simplex.lock` pins and whether its binary still matches the recorded hash.
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

    /// Installs an exact compiler version into the store (downloading it if
    /// absent), without touching `simplex.lock` — pinning stays an explicit,
    /// separate decision (`simplex toolchain` / `simplex build`).
    ///
    /// # Errors
    /// Returns a [`CommandError`] if the version cannot be downloaded or fails
    /// hash verification against a lock that pins this same version.
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

    /// Removes an installed compiler from the store. Safe by construction: the
    /// store is a cache — anything the lock still pins is re-downloaded (and
    /// hash-verified) on the next build.
    ///
    /// # Errors
    /// Returns a [`CommandError`] if the store root cannot be located or the
    /// directory cannot be deleted.
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

/// Resolve the compiler version `requirements` demand against the releases
/// published at [`compiler::RELEASE_SOURCE`], and provision that exact `simc`
/// (downloading it if the store lacks it). With no requirements, the newest
/// release is chosen. A binary hash already recorded in `simplex.lock` for the
/// selected version is enforced.
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

/// The version requirements declared by the project's `.simf` directives.
///
/// # Errors
/// Returns a [`CommandError`] on an unreadable source tree or a malformed directive.
pub fn read_requirements(build: &BuildConfig) -> Result<Vec<version::FileRequirement>, CommandError> {
    let src_dir = std::env::current_dir()?.join(&build.src_dir);
    Ok(version::collect_requirements(&src_dir, &build.simf_files)?)
}

#[derive(Deserialize)]
struct Tag {
    name: String,
}

/// The compiler versions published as `simplicityhl-<version>` release tags,
/// paged through the GitHub API. Set `GITHUB_TOKEN` (or `GH_TOKEN`) to
/// authenticate — unauthenticated requests share a small per-IP rate limit.
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

/// A short, readable form of a path for reporting.
fn display(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok())
        .unwrap_or(path)
        .display()
        .to_string()
}
