use std::io;
use std::path::PathBuf;

use globwalk::GlobError;

#[derive(thiserror::Error, Debug)]
pub enum BuildError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Glob error: {0}")]
    Glob(#[from] GlobError),

    #[error("Failed to deserialize config: '{0}'")]
    ConfigDeserialize(#[from] toml::de::Error),

    #[error("Invalid generation path: '{0}'")]
    GenerationPath(String),

    #[error("Failed to extract content from path, err: '{0}'")]
    FailedToExtractContent(io::Error),

    #[error("Failed to generate file: {0}")]
    GenerationFailed(String),

    #[error(
        "Failed to resolve correct relative path for include_simf! macro, cwd: '{cwd:?}', simf_file: '{simf_file:?}'"
    )]
    FailedToFindCorrectRelativePath { cwd: PathBuf, simf_file: PathBuf },

    #[error("Failed to find prefix for a file: {0}")]
    NoBasePathForGeneration(#[from] std::path::StripPrefixError),

    #[error("Failed to download compiler: {0}")]
    Download(#[from] reqwest::Error),

    #[error("Could not resolve compiler version: {0}")]
    VersionResolution(String),

    #[error("Compiler verification failed after download")]
    VerificationFailed,

    #[error("Compilation failed with exit code: {0}")]
    CompilationFailed(String),

    #[error("Missing compiler version in '{0}'. Please add `#![compiler_version(\"...\")]` to the top of the file.")]
    MissingCompilerVersion(PathBuf),
}
