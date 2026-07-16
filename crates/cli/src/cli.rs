use std::path::PathBuf;

use clap::Parser;

use smplx_build::DependencyConfig;

use crate::commands::build::Build;
use crate::commands::clean::Clean;
use crate::commands::init::Init;
use crate::commands::install::Install;
use crate::commands::regtest::Regtest;
use crate::commands::test::Test;
use crate::commands::toolchain::Toolchain;
use crate::commands::{Command, ToolchainAction};
use crate::config::Config;
use crate::config::error::ConfigError;
use crate::error::CliError;

#[derive(Debug, Parser)]
#[command(name = "Simplex")]
#[command(version, about = "A blazingly-fast, ux-first simplicity development framework")]
pub struct Cli {
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    /// Executes the parsed command and routes it to the corresponding sub-handler.
    ///
    /// # Errors
    /// Returns a `CliError` if loading the configuration fails, an underlying command execution encounters an error,
    /// or if there are file system I/O errors (for example, attempting to initialize over an existing project directory).
    pub fn run(&self) -> Result<(), CliError> {
        match &self.command {
            Command::Init { name } => {
                let simplex_conf_path = match name {
                    Some(name) => {
                        let dir = std::env::current_dir()?.join(name);

                        if dir.exists() {
                            return Err(CliError::Io(std::io::Error::from(std::io::ErrorKind::AlreadyExists)));
                        }

                        std::fs::create_dir_all(&dir)?;
                        dir.join("Simplex.toml")
                    }
                    None => Config::get_default_path()?,
                };

                Ok(Init::run(simplex_conf_path)?)
            }
            Command::Config => {
                let config_path = Config::get_default_path()?;
                let loaded_config = Config::load(config_path)?;

                println!("{loaded_config:#?}");

                Ok(())
            }
            Command::Test { args, flags } => {
                let config_path = Config::get_default_path()?;
                let loaded_config = Config::load(config_path)?;

                Ok(Test::run(loaded_config.test, args, flags)?)
            }
            Command::Regtest => {
                let config_path = Config::get_default_path()?;
                let loaded_config = Config::load(config_path)?;

                Ok(Regtest::run(&loaded_config.regtest)?)
            }
            Command::Install { deps } => {
                let config_path = Config::get_default_path()?;
                DependencyConfig::add_dependency_to(&config_path, deps).map_err(ConfigError::from)?;
                let loaded_config = Config::load(config_path)?;

                Ok(Install::run(&loaded_config.dependencies)?)
            }
            Command::Toolchain { action } => match action {
                // Only the default action needs the project config; store
                // management works from anywhere.
                None => {
                    let config_path = Config::get_default_path()?;
                    let loaded_config = Config::load(config_path)?;

                    Ok(Toolchain::run(&loaded_config.build, &loaded_config.dependencies)?)
                }
                Some(ToolchainAction::List) => Ok(Toolchain::list()?),
                Some(ToolchainAction::Install { version }) => Ok(Toolchain::install(version)?),
                Some(ToolchainAction::Remove { version }) => Ok(Toolchain::remove(version)?),
                Some(ToolchainAction::Dir) => Ok(Toolchain::dir()?),
            },
            Command::Build => {
                let config_path = Config::get_default_path()?;
                let loaded_config = Config::load(config_path)?;

                Ok(Build::run(&loaded_config.build, &loaded_config.dependencies)?)
            }
            Command::Clean => {
                let config_path = Config::get_default_path()?;
                let loaded_config = Config::load(&config_path)?;

                Ok(Clean::run(&loaded_config.build)?)
            }
        }
    }
}
