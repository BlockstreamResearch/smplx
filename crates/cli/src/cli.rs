use std::path::PathBuf;

use clap::Parser;

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

                println!("{loaded_config:#?}");

                let test_config = loaded_config.test.unwrap_or_default();

                Ok(Test::run(test_config, command)?)
            }
            Command::Regtest => {
                // TODO: pass config
                Ok(Regtest::run()?)
            }
            Command::Build { out_dir: _out_dir } => {
                // let loaded_config =
                //     Config::load_or_discover(self.config.clone()).map_err(|e| Error::ConfigDiscoveryFailure(e))?;

                // if loaded_config.build_config.is_none() {
                //     return Err(Error::Config(
                //         "No build config to build contracts environment, please add appropriate config".to_string(),
                //     ));
                // }

                // let build_config = loaded_config.build_config.unwrap();
                // if build_config.compile_simf.is_empty() {
                //     return Err(Error::Config("No files listed to build contracts environment, please check glob patterns or 'compile_simf' field in config.".to_string()));
                // }

                // CodeGenerator::generate_files(&build_config.out_dir, &build_config.compile_simf)?;

                // println!("{build_config:#?}");

                Ok(())
            }
        }
    }
}
