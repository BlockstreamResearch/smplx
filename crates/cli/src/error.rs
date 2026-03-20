use crate::{commands, config};

#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error(transparent)]
    Config(#[from] config::error::ConfigError),

    #[error(transparent)]
    Command(#[from] commands::error::CommandError),

    #[error("IO error: '{0}'")]
    Io(#[from] std::io::Error),
}

pub type CliResult<T> = Result<T, CliError>;
