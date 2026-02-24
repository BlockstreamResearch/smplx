pub mod commands;

use crate::cache_storage::CacheStorage;
use crate::cli::commands::{TestCommand, TestFlags};
use crate::config::{Config, DEFAULT_CONFIG};
use crate::error::Error;
use clap::Parser;
use simplex_test::TestClientProvider;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Parser)]
#[command(name = "simplicity-dex")]
#[command(about = "CLI for Simplicity Options trading on Liquid")]
pub struct Cli {
    #[arg(short, long, env = "SIMPLEX_CONFIG")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: commands::Command,
}

struct TestParams {
    cache_path: PathBuf,
    test_path: TestPaths,
    test_flags: TestFlags,
}

enum TestPaths {
    AllIntegration,
    Names(Vec<String>),
}

impl Cli {
    /// Runs the CLI command.
    ///
    /// # Errors
    /// Returns an error if the command execution fails.
    pub async fn run(&self) -> Result<(), Error> {
        match &self.command {
            commands::Command::Init => {
                let config_path = Config::get_path()?;
                std::fs::write(&config_path, DEFAULT_CONFIG)?;
                println!("Config written to: '{}'", config_path.display());
                Ok(())
            }
            commands::Command::Config => {
                let loaded_config =
                    Config::load_or_discover(self.config.clone()).map_err(|e| Error::ConfigDiscoveryFailure(e))?;
                println!("{loaded_config:#?}");
                Ok(())
            }
            commands::Command::Test { command } => {
                let loaded_config =
                    Config::load_or_discover(self.config.clone()).map_err(|e| Error::ConfigDiscoveryFailure(e))?;
                println!("{loaded_config:#?}");

                self.run_test_command(loaded_config, command)?;

                Ok(())
            }
            commands::Command::Regtest => {
                let running = Arc::new(AtomicBool::new(true));
                let r = running.clone();

                ctrlc::set_handler(move || {
                    r.store(false, Ordering::SeqCst);
                })
                .expect("Error setting Ctrl-C handler");

                let mut node = TestClientProvider::create_default_node_with_stdin();

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

    pub(crate) fn run_test_command(&self, config: Config, command: &TestCommand) -> Result<(), Error> {
        let cache_path = CacheStorage::save_cached_test_config(&config.test_config)?;
        let mut test_command = match command {
            TestCommand::Integration { additional_flags } => Self::form_test_command(TestParams {
                cache_path,
                test_path: TestPaths::AllIntegration,
                test_flags: *additional_flags,
            }),
            TestCommand::Run {
                tests,
                additional_flags,
            } => {
                let test_path = if tests.is_empty() {
                    TestPaths::AllIntegration
                } else {
                    TestPaths::Names(tests.clone())
                };
                Self::form_test_command(TestParams {
                    cache_path,
                    test_path,
                    test_flags: *additional_flags,
                })
            }
        };
        let output = test_command.output()?;
        match output.status.code() {
            Some(code) => {
                println!("Exit Status: {}", code);

                if code == 0 {
                    println!("{}", String::from_utf8(output.stdout).unwrap());
                }
            }
            None => {
                println!("Process terminated.");
            }
        }
        Ok(())
    }

    fn form_test_command(params: TestParams) -> std::process::Command {
        let mut test_command = std::process::Command::new("sh");
        test_command.arg("-c");
        let mut command_as_arg = String::new();
        match params.test_path {
            TestPaths::AllIntegration => {
                command_as_arg.push_str("cargo test --tests");
            }
            TestPaths::Names(names) => {
                let mut arg = "cargo test".to_string();
                for test_name in names {
                    arg.push_str(&format!(" --test {test_name}"));
                }
                command_as_arg.push_str(&arg);
            }
        }
        {
            match params.test_flags.show_output {
                true => match params.test_flags.nocapture {
                    true => {
                        command_as_arg.push_str(&" -- --nocapture --show-output");
                    }
                    false => {
                        command_as_arg.push_str(&" -- --show-output");
                    }
                },
                false => match params.test_flags.nocapture {
                    true => {
                        command_as_arg.push_str(&" -- --nocapture");
                    }
                    false => {}
                },
            }
        }
        test_command.args([command_as_arg]);
        dbg!(test_command.get_args());
        test_command
            .env(simplex_test::TEST_ENV_NAME, params.cache_path)
            .stdin(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdout(Stdio::inherit());
        test_command
    }
}
