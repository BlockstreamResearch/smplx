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

    #[error("Invalid dependency '{0}': you must specify either a 'path' or a 'git' repository")]
    InvalidDependency(String),

    #[error("Dependency '{dep_name}' is missing its configuration manifest at: {expected_path}")]
    MissingDependencyConfig { dep_name: String, expected_path: PathBuf },

    #[error("{0}")]
    PathCanonicalization(String),

    #[error("Failed to build dependency map: {0}")]
    DependencyMap(String),

    #[error("Failed to flatten program: {0}")]
    Flattening(String),
}
