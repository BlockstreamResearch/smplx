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
    Install(#[from] InstallError),

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
pub enum InstallError {
    #[error(transparent)]
    Config(#[from] crate::config::error::ConfigError),

    #[error("Invalid Git repository URL (cannot extract name): '{0}'")]
    InvalidUrl(String),

    #[error("Failed to create directory at '{1}': {0}")]
    CreateDir(std::io::Error, PathBuf),

    #[error("Failed to read directory at '{1}': {0}")]
    ReadDir(std::io::Error, PathBuf),

    #[error("Failed to execute git process for '{1}': {0}")]
    GitExecution(std::io::Error, String),

    #[error("Git clone command failed for repository: '{0}'")]
    GitCloneFailed(String),
}
