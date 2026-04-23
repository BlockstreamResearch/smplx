use std::path::PathBuf;

use clap::Parser;

use crate::commands::Command;
use crate::commands::build::Build;
use crate::commands::clean::Clean;
use crate::commands::init::Init;
use crate::commands::regtest::Regtest;
use crate::commands::test::Test;
use crate::config::Config;
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
    pub async fn run(&self) -> Result<(), CliError> {
        match &self.command {
            Command::Init { additional_flags } => {
                let simplex_conf_path = Config::get_default_path()?;

                Ok(Init::run(simplex_conf_path, additional_flags)?)
            }
            Command::Config => {
                let config_path = Config::get_default_path()?;
                let loaded_config = Config::load(config_path)?;

                println!("{loaded_config:#?}");

                Ok(())
            }
            Command::Test { name, additional_flags } => {
                let config_path = Config::get_default_path()?;
                let loaded_config = Config::load(config_path)?;

                let filter = name.clone().unwrap_or_default();

                Ok(Test::run(loaded_config.test, filter, additional_flags)?)
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
            Command::Clean => {
                let config_path = Config::get_default_path()?;
                let loaded_config = Config::load(&config_path)?;

                Ok(Clean::run(loaded_config.build)?)
            }
        }
    }
}
