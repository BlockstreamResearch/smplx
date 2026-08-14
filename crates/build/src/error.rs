use std::io;
use std::path::PathBuf;

use globwalk::GlobError;

/// Neutral validation error for a single dependency declaration.
///
/// Returned by `DependencyConfig::validate` so it stays decoupled from any
/// specific crate's error type. Each caller converts it into their own variant.
#[derive(thiserror::Error, Debug)]
pub enum DependencyValidationError {
    #[error("Invalid dependency '{0}': you must specify either a 'path' or a 'git' repository")]
    Missing(String),

    #[error("Invalid dependency '{0}': cannot specify both 'path' and 'git', choose one")]
    Conflicting(String),

    #[error("Invalid dependency '{0}': `path` cannot be combined with git-only fields (rev/tag)")]
    PathWithGitField(String),

    #[error("Invalid dependency '{0}': only one of `rev`, `tag` may be set")]
    ConflictingGitRef(String),
}

/// Errors produced while editing `Simplex.toml` to add or modify a dependency.
#[derive(thiserror::Error, Debug)]
pub enum TomlEditError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("failed to parse `{path}` for editing: {source}")]
    UnableToEdit {
        path: PathBuf,
        source: toml_edit::TomlError,
    },

    #[error("`[dependencies]` in `Simplex.toml` is not a table")]
    MalformedDependenciesTable,

    #[error("malformed dependency spec `{0}` (expected `<source>` or `<alias>=<source>`)")]
    MalformedDep(String),

    #[error("dependency `{0}` already exists")]
    DuplicateAlias(String),
}

#[derive(thiserror::Error, Debug)]
pub enum BuildError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("failed to serialize metadata: {0}")]
    MetadataSerialization(#[from] serde_json::Error),

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

    #[error(transparent)]
    DependencyValidation(#[from] DependencyValidationError),

    #[error("Dependency '{dep_name}' is missing its configuration manifest at: {expected_path}")]
    MissingDependencyConfig { dep_name: String, expected_path: PathBuf },

    #[error("{0}")]
    PathCanonicalization(String),

    #[error("Failed to build dependency map: {0}")]
    DependencyMap(String),

    #[error("Failed to flatten program: {0}")]
    Flattening(String),

    #[error("Failed to dry run the program: {0}")]
    DryRun(String),

    #[error("Invalid git repository URL: '{0}'")]
    InvalidGitUrl(String),

    #[error(transparent)]
    TomlEdit(#[from] TomlEditError),
}
