use crate::{commands::error::CommandError, config};

#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error(transparent)]
    Config(#[from] config::error::ConfigError),

    #[error(transparent)]
    Command(#[from] CommandError),

    #[error("IO error: '{0}'")]
    Io(#[from] std::io::Error),

    #[error("Occurred code generation error, error: '{0}'")]
    CodeGenerator(#[from] simplex_template_gen_core::CodeGeneratorError),
}
