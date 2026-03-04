use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    /// Standard I/O errors.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Errors when parsing TOML configuration files.
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// Errors when parsing TOML configuration files.
    #[error("Unable to deserialize config: {0}")]
    UnableToDeserialize(toml::de::Error),

    /// Errors when parsing env variable.
    #[error("Unable to get env variable: {0}")]
    UnableToGetEnv(#[from] std::env::VarError),

    /// Errors when getting a path to config.
    #[error("Path doesn't a file: '{0}'")]
    PathIsNotFile(PathBuf),

    /// Errors when getting a path to config.
    #[error("Path doesn't exist: '{0}'")]
    PathNotExists(PathBuf),
}
