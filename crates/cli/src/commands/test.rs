use std::path::PathBuf;
use std::process::Stdio;

use smplx_test::TestConfig;

use super::core::{TestCommand, TestFlags};
use super::error::CommandResult;

pub struct Test {}

impl Test {
    pub fn run(config: TestConfig, command: &TestCommand) -> CommandResult<()> {
        let cache_path = Self::get_test_config_cache_name()?;
        config.to_file(&cache_path)?;

        let mut cargo_test_command = Self::build_cargo_test_command(&cache_path, command);

        let output = cargo_test_command.output()?;

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

    fn build_cargo_test_command(cache_path: &PathBuf, command: &TestCommand) -> std::process::Command {
        let mut command_as_arg = String::new();

        match command {
            TestCommand::Integration { additional_flags } => {
                command_as_arg.push_str("cargo test --tests");

                let flag_args = Self::build_test_flags(additional_flags);

                if !flag_args.is_empty() {
                    command_as_arg.push_str(" --");
                    command_as_arg.push_str(&flag_args);
                }
            }
            TestCommand::Run {
                tests,
                additional_flags,
            } => {
                // TODO: check this behavior
                if tests.is_empty() {
                    command_as_arg.push_str("cargo test --tests");
                } else {
                    let mut arg = "cargo test".to_string();

                    for test_name in tests {
                        arg.push_str(&format!(" --test {test_name}"));
                    }

                    command_as_arg.push_str(&arg);
                }

                let flag_args = Self::build_test_flags(additional_flags);

                if !flag_args.is_empty() {
                    command_as_arg.push_str(" --");
                    command_as_arg.push_str(&flag_args);
                }
            }
        }

        let mut cargo_test_command = std::process::Command::new("sh");
        cargo_test_command.args(["-c".to_string(), command_as_arg]);

        cargo_test_command
            .env(smplx_test::TEST_ENV_NAME, cache_path)
            .stdin(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdout(Stdio::inherit());

        cargo_test_command
    }

    fn build_test_flags(flags: &TestFlags) -> String {
        let mut opt_params = String::new();

        if flags.nocapture {
            opt_params.push_str(" --nocapture");
        }

        if flags.show_output {
            opt_params.push_str(" --show-output");
        }

        if flags.ignored {
            opt_params.push_str(" --ignored");
        }

        opt_params
    }

    fn get_test_config_cache_name() -> CommandResult<PathBuf> {
        const TARGET_DIR_NAME: &str = "target";
        const SIMPLEX_CACHE_DIR_NAME: &str = "simplex";
        const SIMPLEX_TEST_CONFIG_NAME: &str = "simplex_test_config.toml";

        let cwd = std::env::current_dir()?;

        Ok(cwd
            .join(TARGET_DIR_NAME)
            .join(SIMPLEX_CACHE_DIR_NAME)
            .join(SIMPLEX_TEST_CONFIG_NAME))
    }
}
