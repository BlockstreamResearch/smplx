use reqwest::blocking::Client;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::BuildError;

mod constants;
mod downloader;
mod versioning;

use downloader::CompilerDownloader;

/// Represents an executable instance of the Simplicity compiler.
#[derive(Clone)]
pub struct Simc {
    pub binary_path: PathBuf,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
}

impl Simc {
    /// Lists all locally installed Simplicity compiler versions
    pub fn list_downloaded_versions() -> Result<Vec<String>, BuildError> {
        let cache_dir = home::home_dir()
            .ok_or_else(|| BuildError::VersionResolution("No home dir".into()))?
            .join(constants::SMPLX_CACHE_DIR)
            .join(constants::COMPILERS_DIR_NAME);

        let downloader = CompilerDownloader::new(cache_dir);
        Ok(downloader.get_downloaded_versions())
    }

    fn fetch_available_versions() -> Result<Vec<String>, BuildError> {
        let client = Client::builder()
            .user_agent(constants::USER_AGENT)
            .build()
            .map_err(BuildError::Download)?;

        let response: Vec<GithubRelease> = client
            .get(constants::GITHUB_API_RELEASES_URL)
            .send()
            .map_err(BuildError::Download)?
            .json()
            .map_err(|e| {
                BuildError::VersionResolution(format!("Failed to parse GitHub releases API response: {}", e))
            })?;

        let versions = response
            .into_iter()
            .filter_map(|release| release.tag_name.strip_prefix("simplicityhl-").map(|v| v.to_string()))
            .collect();

        Ok(versions)
    }

    /// Resolves the correct compiler version.
    /// It checks for a native installation first, then falls back to downloading.
    pub fn resolve(version_req: &str) -> Result<Self, BuildError> {
        if let Some(native_path) = Self::try_native(version_req) {
            println!("--> Using native compiler at {}", native_path.display());
            return Ok(Self {
                binary_path: native_path,
            });
        }

        let cache_dir = home::home_dir()
            .ok_or_else(|| BuildError::VersionResolution("No home dir".into()))?
            .join(constants::SMPLX_CACHE_DIR)
            .join(constants::COMPILERS_DIR_NAME);

        let downloader = CompilerDownloader::new(cache_dir);

        let known_releases = match Self::fetch_available_versions() {
            Ok(versions) => versions,
            Err(_) => {
                let local_versions = downloader.get_downloaded_versions();
                if local_versions.is_empty() {
                    return Err(BuildError::VersionResolution(
                        "Offline and no local compilers found.".into(),
                    ));
                }
                println!("--> WARNING: Could not fetch releases from GitHub. Using local cache.");
                local_versions
            }
        };

        let exact_target = versioning::resolve_target_version(version_req, &known_releases)?;

        let min_supported = semver::Version::parse(constants::MIN_SUPPORTED_COMPILER_VERSION).unwrap();
        let resolved_version = semver::Version::parse(&exact_target)
            .map_err(|e| BuildError::VersionResolution(format!("Invalid resolved version: {}", e)))?;
        if resolved_version < min_supported {
            return Err(BuildError::VersionResolution(format!(
                "Resolved compiler version v{} is below the minimum supported version v{}.",
                exact_target,
                constants::MIN_SUPPORTED_COMPILER_VERSION
            )));
        }

        let max_supported = semver::Version::parse(constants::MAX_SUPPORTED_COMPILER_VERSION).unwrap();
        if resolved_version > max_supported {
            println!(
                "--> WARNING: Resolved compiler version v{} is higher than the maximum tested version v{}. You may experience ABI generation issues.",
                exact_target,
                constants::MAX_SUPPORTED_COMPILER_VERSION
            );
        }

        if version_req != exact_target {
            println!(
                "--> Resolved floating requirement {} to exact version v{}",
                version_req, exact_target
            );
        }

        if !downloader.is_compiler_downloaded(&exact_target) {
            downloader.download_compiler(&exact_target, true)?;
        }

        let compiler_info = downloader.get_compiler_binary(&exact_target)?;

        Ok(Self {
            binary_path: compiler_info.binary_path,
        })
    }

    /// Executes the compilation of a single .simf file
    pub fn compile(&self, src: &Path, out: &Path) -> Result<(), BuildError> {
        let output = Command::new(&self.binary_path)
            .arg(src)
            .output()
            .map_err(BuildError::Io)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BuildError::CompilationFailed(format!(
                "Failed to compile '{}'. Exit code: {}.\n\n{}",
                src.display(),
                output.status.code().unwrap_or(1),
                stderr.trim()
            )));
        }

        std::fs::write(out, &output.stdout).map_err(BuildError::Io)?;

        Ok(())
    }

    /// Attempts to find `simc` in the system PATH that matches the version requirement
    fn try_native(version_req: &str) -> Option<PathBuf> {
        let native_path = which::which("simc").ok()?;

        let output = Command::new(&native_path).arg("--version").output().ok()?;
        if !output.status.success() {
            return None;
        }

        let version_str = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .last()?
            .trim_start_matches('v')
            .to_string();

        let native_version = semver::Version::parse(&version_str).ok()?;
        let req = semver::VersionReq::parse(version_req).ok()?;

        let min_supported = semver::Version::parse(constants::MIN_SUPPORTED_COMPILER_VERSION).ok()?;
        if native_version < min_supported {
            return None;
        }

        if req.matches(&native_version) {
            Some(native_path)
        } else {
            None
        }
    }
}
