pub mod commands;

use crate::error::Error;

use crate::config::{default_config_path, Config};

use clap::Parser;
use corepc_node::client::client_sync::Auth;
use simplex_test::{DefaultElementsdParams, ElementsRpc, ElementsdParams};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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

                let bin_path = ElementsRpc::get_bin_path();
                let (args, auth, rpc_url) = get_conf();
                let mut child = Command::new(bin_path)
                    .args(args)
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()?;


                // TODO: maybe check connection?
                println!("======================================");
                println!("Waiting for Ctrl-C...");
                println!("url: {}", rpc_url);
                println!("auth: {:?}", auth.get_user_pass().unwrap());
                println!("======================================");
                while running.load(Ordering::SeqCst) {}

                child.kill()?;
                println!("Exiting...");
                Ok(())
            }
        }
    }
}

fn get_conf() -> (Vec<String>, Auth, String) {
    let port = get_random_port();
    let mut args = DefaultElementsdParams {}.get_bin_args();
    args.push("-rpcuser=admin".to_string());
    args.push("-rpcpassword=123".to_string());
    args.push(format!("-rpcport={port}"));

    (
        args,
        Auth::UserPass("admin".to_string(), "123".to_string()),
        format!("127.0.0.1:{port}"),
    )
}

fn get_random_port() -> u16 {
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    listener.local_addr().unwrap().port()
}
