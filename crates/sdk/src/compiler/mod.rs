//! Out-of-process SimplicityHL compiler.
//!
//! A contract's compiled Simplicity (and thus its CMR, address and spend) must come
//! from the exact compiler version it was written for, not the linked `simplicityhl`.
//! This module locates or downloads the pinned `simc` binary in the store under
//! `~/.simplex/compilers/<version>/bin/simc` and runs it as a subprocess. The linked
//! frontend is used only for version-stable work (flattening, ABI typing).

pub mod lockfile;
mod provision;
mod store;

pub use provision::{ensure, provision, sha256_hex};
pub use store::{compilers_dir, host_asset, installed_versions, resolve_version, simc_path};

/// The oldest SimplicityHL compiler Simplex can drive: the first versioned release.
/// Older compilers predate the versioned `simc` interface and are rejected as too old
/// at provisioning. From this floor up the interface is additive-only.
pub const MIN_COMPILER_VERSION: &str = "0.7.0";

/// The GitHub repository Simplex downloads `simc` compiler releases from.
///
/// The single release source: change this one line (then rebuild the `simplex` CLI)
/// to point at a different SimplicityHL release repository. Must be a full
/// `https://github.com/<owner>/<repo>` URL with no trailing slash.
pub const RELEASE_SOURCE: &str = "https://github.com/Sdoba16/SimplicityHL";
/// Release tags are `simplicityhl-<version>`, matching the compiler's CI.
pub const TAG_PREFIX: &str = "simplicityhl-";

/// The `owner/repo` slug of [`RELEASE_SOURCE`], for GitHub API calls. `None` if it
/// is not a `github.com` URL (no release API to query).
#[must_use]
pub fn source_repo() -> Option<String> {
    RELEASE_SOURCE
        .strip_prefix("https://github.com/")
        .map(|slug| slug.trim_end_matches('/').trim_end_matches(".git").to_string())
}

/// Errors from locating, provisioning, or running an out-of-process `simc`.
#[derive(Debug, thiserror::Error)]
pub enum CompilerError {
    /// No compiler version could be determined for this program.
    #[error("no SimplicityHL compiler version is pinned (no simplex.lock or SIMPLEX_COMPILER_VERSION found)")]
    NoVersion,

    /// The requested version is not a semantic version. Also the path-safety gate:
    /// the version becomes a store path and a URL segment, and semver's charset
    /// cannot traverse either.
    #[error("invalid compiler version '{0}': not a semantic version")]
    InvalidVersion(String),

    /// The compiler store location cannot be determined.
    #[error("cannot locate the compiler store: neither SIMPLEX_COMPILERS nor HOME is set")]
    NoStore,

    /// The binary in the store (or just downloaded) does not match the recorded hash.
    #[error(
        "compiler {version} does not match its recorded sha256 (expected {expected}, got {actual}); \
         remove it from the store or update simplex.lock (`simplex toolchain`)"
    )]
    Verify {
        /// The compiler version whose binary failed verification.
        version: String,
        /// The hash recorded in `simplex.lock`.
        expected: String,
        /// The hash of the actual binary.
        actual: String,
    },

    /// The binary predates the versioned simc interface.
    #[error(
        "compiler {version} is too old: it predates the versioned simc interface; Simplex requires \
         SimplicityHL {min} or newer — raise the project's `simc` directives to a versioned release"
    )]
    TooOld {
        /// The compiler version whose binary is too old.
        version: String,
        /// The oldest supported release ([`MIN_COMPILER_VERSION`]).
        min: &'static str,
    },

    /// The requested compiler is not in the store and could not be downloaded.
    #[error("compiler {version} is not installed and could not be provisioned: {reason}")]
    NotProvisioned {
        /// The compiler version that was requested.
        version: String,
        /// Why provisioning failed.
        reason: String,
    },

    /// Downloading the release asset failed.
    #[error("failed to download {url}: {reason}")]
    Download {
        /// The asset URL that was requested.
        url: String,
        /// Why the download failed.
        reason: String,
    },

    /// Unpacking the downloaded archive failed.
    #[error("failed to unpack compiler archive: {0}")]
    Unpack(String),

    /// This host has no matching release asset.
    #[error("unsupported platform for prebuilt simc: {0}")]
    UnsupportedPlatform(String),

    /// Running `simc` as a subprocess failed, or it exited unsuccessfully.
    #[error("simc invocation failed: {0}")]
    Invoke(String),

    /// `simc` produced output Simplex could not turn into an artifact.
    #[error("could not read simc output: {0}")]
    Output(String),

    /// An underlying filesystem error.
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
}
