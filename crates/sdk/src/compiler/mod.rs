//! Out-of-process SimplicityHL compiler: a contract's compiled Simplicity (and thus
//! its address) must come from the exact compiler version it was written for, so the
//! pinned `simc` from the store runs as a subprocess. The linked frontend does only
//! version-stable work (flattening, ABI typing).

mod backend;
pub mod lockfile;
mod provision;
mod store;

pub use backend::compile;
pub use provision::{ensure, provision, sha256_hex};
pub use store::{compilers_dir, host_asset, installed_versions, resolve_version, simc_path};

/// The oldest compiler Simplex can drive: the first versioned release. From this
/// floor up the `simc` interface is additive-only.
pub const MIN_COMPILER_VERSION: &str = "0.7.0";

/// The single release source `simc` compilers are downloaded from: change this one
/// line to point at a different repository. Full URL, no trailing slash.
pub const RELEASE_SOURCE: &str = "https://github.com/Sdoba16/SimplicityHL";
/// Release tags are `simplicityhl-<version>`, matching the compiler's CI.
pub const TAG_PREFIX: &str = "simplicityhl-";

/// The `owner/repo` slug of [`RELEASE_SOURCE`]; `None` if not a `github.com` URL.
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

    /// Not a semantic version. Also the path-safety gate: the version becomes a
    /// store path and a URL segment, and semver's charset cannot traverse either.
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
