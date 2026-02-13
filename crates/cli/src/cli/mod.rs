pub mod commands;

use crate::error::Error;

use crate::config::{Config, default_config_path};

use clap::Parser;
use std::path::PathBuf;

pub use commands::Command;

#[derive(Debug, Parser)]
#[command(name = "simplicity-dex")]
#[command(about = "CLI for Simplicity Options trading on Liquid")]
pub struct Cli {
    #[arg(short, long, default_value_os_t = default_config_path(), env = "SIMPLEX_CONFIG")]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    #[must_use]
    pub fn load_config(&self) -> Config {
        Config::load_or_default(&self.config)
    }

    /// Runs the CLI command.
    ///
    /// # Errors
    /// Returns an error if the command execution fails.
    pub async fn run(&self) -> Result<(), Error> {
        let config = self.load_config();

        match &self.command {
            Command::Config => {
                println!("{config:#?}");
                Ok(())
            }
        }
    }
}
