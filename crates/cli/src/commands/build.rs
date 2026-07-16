use smplx_build::deps::DepSources;
use smplx_build::version::Version;
use smplx_build::{ArtifactsGenerator, ArtifactsResolver, BuildConfig, DependencyConfig};
use smplx_sdk::compiler;
use smplx_sdk::compiler::lockfile;

use crate::config::CONFIG_FILENAME;

use super::error::{CommandError, ToolchainError};
use super::toolchain;

pub struct Build {}

impl Build {
    /// Builds the project and generates artifacts based on the provided configuration.
    /// The compiler is pinned lock-first and baked into the generated bindings, so
    /// the runtime compiles each contract with the same `simc`.
    ///
    /// # Errors
    /// Returns a `CommandError` if compiler pinning, file resolution, or artifact
    /// generation fails.
    pub fn run(config: &BuildConfig, deps: &DependencyConfig) -> Result<(), CommandError> {
        // NOTE: Assumes that remappings are already installed
        let dep_sources = ArtifactsResolver::resolve_remappings(deps, CONFIG_FILENAME)?;

        let (version, simc) = pin_compiler(config, &dep_sources)?;

        let output_dir = ArtifactsResolver::resolve_local_dir(&config.out_dir)?;
        let src_dir = ArtifactsResolver::resolve_local_dir(&config.src_dir)?;

        let files_to_build = ArtifactsResolver::resolve_files_to_build(&config.src_dir, &config.simf_files)?;

        Ok(ArtifactsGenerator::generate_artifacts(
            &output_dir,
            &src_dir,
            &files_to_build,
            &dep_sources,
            &version.to_string(),
            simc.as_path(),
        )?)
    }
}

/// The compiler this build uses: the locked one when it satisfies every declared
/// directive (no network), otherwise a fresh resolution recorded back into the lock.
fn pin_compiler(config: &BuildConfig, deps: &DepSources) -> Result<(Version, std::path::PathBuf), CommandError> {
    let cwd = std::env::current_dir()?;
    let requirements = toolchain::read_requirements(config, deps)?;

    if let Some(locked) = lockfile::read_at(&cwd) {
        // A lock pinning garbage must stop the build, not silently re-pin: the
        // version determines every contract's address.
        let version = Version::parse(&locked.version).map_err(|e| {
            ToolchainError::InvalidLock(format!(
                "{} pins invalid version '{}' ({e}); fix it or re-pin with `simplex toolchain`",
                lockfile::LOCKFILE,
                locked.version
            ))
        })?;

        if requirements.iter().all(|req| req.requirement.matches(&version)) {
            let simc = compiler::provision(&locked.version, locked.sha256_for_host().as_deref())
                .map_err(ToolchainError::from)?;
            // Adopt this platform's binary hash if the lock lacks it, so a committed
            // lock accumulates one entry per platform the team builds on.
            if locked.sha256_for_host().is_none() {
                lockfile::write(&cwd, &locked.version, &simc).map_err(ToolchainError::from)?;
            }
            println!(
                "Using locked SimplicityHL compiler {version} (see {})",
                lockfile::LOCKFILE
            );
            return Ok((version, simc));
        }
        println!("Locked SimplicityHL compiler {version} no longer satisfies the declared directives; re-resolving.");
    } else if cwd.join(lockfile::LOCKFILE).exists() {
        // Same principle for a lock that does not even parse: refuse to guess.
        return Err(ToolchainError::InvalidLock(format!(
            "{} exists but cannot be parsed; fix it or delete it and re-pin with `simplex toolchain`",
            lockfile::LOCKFILE
        ))
        .into());
    }

    let (version, simc) = toolchain::resolve_and_provision(&requirements)?;
    lockfile::write(&cwd, &version.to_string(), &simc).map_err(ToolchainError::from)?;
    println!("Pinned SimplicityHL compiler {version} (see {})", lockfile::LOCKFILE);
    Ok((version, simc))
}
