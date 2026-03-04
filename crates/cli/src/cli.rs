use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::Parser;

use simplex_regtest::TestClient;
use simplex_template_gen_core::CodeGenerator;

use crate::commands::{Command, TestCommand, TestFlags};
use crate::config::{Config, INIT_CONFIG};
use crate::error::Error;

#[derive(Debug, Parser)]
#[command(name = "Simplex")]
#[command(about = "Simplicity development framework")]
pub struct Cli {
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
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
    pub async fn run(&self) -> Result<(), Error> {
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
                // let loaded_config =
                //     Config::load_or_discover(self.config.clone()).map_err(|e| Error::ConfigDiscoveryFailure(e))?;
                // println!("{loaded_config:#?}");

                // self.run_test_command(loaded_config, command)?;

                Ok(())
            }
            Command::Regtest => {
                let running = Arc::new(AtomicBool::new(true));
                let r = running.clone();

                ctrlc::set_handler(move || {
                    r.store(false, Ordering::SeqCst);
                })
                .expect("Error setting Ctrl-C handler");

                let mut client = TestClient::new();

                println!("======================================");
                println!("Waiting for Ctrl-C...");
                println!("rpc: {}", client.rpc_url());
                println!("esplora: {}", client.esplora_url());
                let auth = client.auth().get_user_pass().unwrap();
                println!("user: {:?}, password: {:?}", auth.0.unwrap(), auth.1.unwrap());
                println!("======================================");

                while running.load(Ordering::SeqCst) {}
                client.kill();

                println!("Exiting...");

                Ok(())
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

    // pub(crate) fn run_test_command(&self, config: Config, command: &TestCommand) -> Result<(), Error> {
    //     let cache_path = CacheStorage::save_cached_test_config(&config.test_config)?;
    //     let mut test_command = match command {
    //         TestCommand::Integration { additional_flags } => Self::form_test_command(TestParams {
    //             cache_path,
    //             test_path: TestPaths::AllIntegration,
    //             test_flags: *additional_flags,
    //         }),
    //         TestCommand::Run {
    //             tests,
    //             additional_flags,
    //         } => {
    //             let test_path = if tests.is_empty() {
    //                 TestPaths::AllIntegration
    //             } else {
    //                 TestPaths::Names(tests.clone())
    //             };
    //             Self::form_test_command(TestParams {
    //                 cache_path,
    //                 test_path,
    //                 test_flags: *additional_flags,
    //             })
    //         }
    //     };
    //     let output = test_command.output()?;
    //     match output.status.code() {
    //         Some(code) => {
    //             println!("Exit Status: {}", code);

    //             if code == 0 {
    //                 println!("{}", String::from_utf8(output.stdout).unwrap());
    //             }
    //         }
    //         None => {
    //             println!("Process terminated.");
    //         }
    //     }
    //     Ok(())
    // }

    // fn form_test_command(params: TestParams) -> std::process::Command {
    //     let mut test_command = std::process::Command::new("sh");
    //     test_command.arg("-c");
    //     let mut command_as_arg = String::new();
    //     match params.test_path {
    //         TestPaths::AllIntegration => {
    //             command_as_arg.push_str("cargo test --tests");
    //         }
    //         TestPaths::Names(names) => {
    //             let mut arg = "cargo test".to_string();
    //             for test_name in names {
    //                 arg.push_str(&format!(" --test {test_name}"));
    //             }
    //             command_as_arg.push_str(&arg);
    //         }
    //     }
    //     {
    //         let mut opt_params = String::new();
    //         if params.test_flags.show_output {
    //             opt_params.push_str(" --show-output");
    //         }
    //         if params.test_flags.nocapture {
    //             opt_params.push_str(" --nocapture");
    //         }
    //         if params.test_flags.ignored {
    //             opt_params.push_str(" --ignored");
    //         }
    //         if params.test_flags.show_output || params.test_flags.nocapture || params.test_flags.ignored {
    //             command_as_arg.push_str(" --");
    //             command_as_arg.push_str(&opt_params);
    //         }
    //     }
    //     test_command.args([command_as_arg]);
    //     let _ = dbg!(test_command.get_args());
    //     test_command
    //         .env(simplex_test::TEST_ENV_NAME, params.cache_path)
    //         .stdin(Stdio::inherit())
    //         .stderr(Stdio::inherit())
    //         .stdout(Stdio::inherit());
    //     test_command
    // }
}
