use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("Network name should either be `Liquid`, `LiquidTestnet` or `ElementsRegtest`, got: {0}")]
    BadNetworkName(String),

    #[error("Network name should be `ElementsRegtest` when RPC is specified, got: {0}")]
    NetworkNameUnmatched(String),

    #[error("Unable to deserialize config: {0}")]
    UnableToDeserialize(toml::de::Error),

    #[error("Unable to get env variable: {0}")]
    UnableToGetEnv(#[from] std::env::VarError),

    #[error("Path doesn't a file: '{0}'")]
    PathIsNotFile(PathBuf),

    #[error("Path doesn't exist: '{0}'")]
    PathNotExists(PathBuf),
}
