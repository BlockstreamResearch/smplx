use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error(transparent)]
    Signer(#[from] smplx_sdk::signer::SignerError),

    #[error(transparent)]
    Regtest(#[from] smplx_regtest::error::RegtestError),

    #[error(transparent)]
    Test(#[from] smplx_test::error::TestError),

    #[error(transparent)]
    Build(#[from] smplx_build::error::BuildError),

    #[error(transparent)]
    Init(#[from] InitError),

    #[error(transparent)]
    Clean(#[from] CleanError),

    #[error(transparent)]
    NextestVersionCheck(#[from] NextestVersionCheckError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Verbosity level should be either -v or -vv, got: -v x {0}")]
    BadVersbosityMode(u8),
}

#[derive(thiserror::Error, Debug)]
pub enum InitError {
    #[error("Failed to open file '{1}': {0}")]
    OpenFile(std::io::Error, PathBuf),

    #[error("Failed to write to file '{1}': {0}")]
    WriteToFile(std::io::Error, PathBuf),

    #[error("Failed to format file with rustfmt: {0}")]
    FmtError(std::io::Error),

    #[error("Failed to resolve parent directory for: {0}")]
    ResolveParent(PathBuf),

    #[error("Failed to create directories at '{1}': {0}")]
    CreateDirs(std::io::Error, PathBuf),

    #[error("Failed to fetch crate info from crates.io: {0}")]
    CratesIoFetch(String),

    #[error("Cannot auto-detect package name from path: {0}")]
    PackageName(PathBuf),

    #[error("Cannot create package with a non-unicode name: '{0}'")]
    NonUnicodeName(String),
}

#[derive(thiserror::Error, Debug)]
pub enum CleanError {
    #[error("Failed to resolve out_dir from config, err: '{0}'")]
    ResolveOutDir(String),

    #[error("Failed to remove output directory '{1}': {0}")]
    RemoveOutDir(std::io::Error, PathBuf),

    #[error("Failed to remove file '{1}': {0}")]
    RemoveFile(std::io::Error, PathBuf),
}

#[derive(thiserror::Error, Debug)]
pub enum NextestVersionCheckError {
    #[error("Nextest is not installed. Please run `simplexup` to install.")]
    NextestNotInstalled,

    #[error("Failed to parse nextest version string '{0}': {1}")]
    NextestVersionParseError(String, semver::Error),

    #[error("Failed to parse the required version bound '{0}': {1}")]
    NextestVersionReqParseError(String, semver::Error),

    #[error(
        "Your nextest version {current_version} does not meet the requirement: {required_bound}. Please run `simplexup` to install."
    )]
    UnsupportedNextestVersion {
        current_version: String,
        required_bound: String,
    },
}
