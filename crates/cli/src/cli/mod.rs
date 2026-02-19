pub mod commands;

use crate::error::Error;
use clap::Parser;
use simplex_config::Config;
use simplex_test::TestProvider;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const DEFAULT_CONFIG_PATH: &str = "config.toml";

#[derive(Debug, Parser)]
#[command(name = "simplicity-dex")]
#[command(about = "CLI for Simplicity Options trading on Liquid")]
pub struct Cli {
    #[arg(short, long, default_value_os_t = default_config_path(), env = "SIMPLEX_CONFIG")]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: commands::Command,
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
            commands::Command::Config => {
                println!("{config:#?}");
                Ok(())
            }
            commands::Command::Regtest => {
                let running = Arc::new(AtomicBool::new(true));
                let r = running.clone();

                ctrlc::set_handler(move || {
                    r.store(false, Ordering::SeqCst);
                })
                .expect("Error setting Ctrl-C handler");

                let mut node = TestProvider::create_default_node_with_stdin();

                println!("======================================");
                println!("Waiting for Ctrl-C...");
                println!("url: {}", node.rpc_url());
                let cookie_values = node.params.get_cookie_values()?.unwrap();
                println!("user: {:?}, password: {:?}", cookie_values.user, cookie_values.password);
                println!("======================================");

                while running.load(Ordering::SeqCst) {}
                let _ = node.stop();
                println!("Exiting...");
                Ok(())
            }
        }
    }
}

#[must_use]
pub fn default_config_path() -> PathBuf {
    PathBuf::from(DEFAULT_CONFIG_PATH)
}
