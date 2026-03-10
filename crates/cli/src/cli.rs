use std::path::PathBuf;

use clap::Parser;

use crate::commands::build::Build;
use crate::commands::commands::Command;
use crate::commands::regtest::Regtest;
use crate::commands::test::Test;
use crate::config::{Config, INIT_CONFIG};
use crate::error::CliError;

#[derive(Debug, Parser)]
#[command(name = "Simplex")]
#[command(about = "Simplicity development framework")]
pub struct Cli {
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub async fn run(&self) -> Result<(), CliError> {
        match &self.command {
            Command::Init => {
                let config_path = Config::get_default_path()?;
                std::fs::write(&config_path, INIT_CONFIG)?;

                println!("Config written to: '{}'", config_path.display());

                Ok(())
            }
            Command::Config => {
                let config_path = Config::get_default_path()?;
                let loaded_config = Config::load(config_path)?;

                println!("{loaded_config:#?}");

                Ok(())
            }
            Command::Test { command } => {
                let config_path = Config::get_default_path()?;
                let loaded_config = Config::load(config_path)?;

                Ok(Test::run(loaded_config.test, command)?)
            }
            Command::Regtest => {
                let config_path = Config::get_default_path()?;
                let loaded_config = Config::load(config_path)?;

                Ok(Regtest::run(loaded_config.regtest)?)
            }
            Command::Build => {
                let config_path = Config::get_default_path()?;
                let loaded_config = Config::load(config_path)?;

                Ok(Build::run(loaded_config.build)?)
            }
        }
    }
}
