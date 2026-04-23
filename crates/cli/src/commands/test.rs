use std::path::PathBuf;
use std::process::Stdio;

use smplx_test::{SMPLX_TEST_MARKER, TestConfig};

use super::core::TestFlags;
use super::error::CommandError;

pub struct Test {}

impl Test {
    pub fn run(config: TestConfig, filter: String, flags: &TestFlags) -> Result<(), CommandError> {
        let cache_path = Self::get_test_config_cache_name()?;
        config.to_file(&cache_path)?;

        let mut cargo_test_command = Self::build_cargo_test_command(&cache_path, filter, flags);

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

    fn build_cargo_test_command(cache_path: &PathBuf, filter: String, flags: &TestFlags) -> std::process::Command {
        let mut cargo_test_command = std::process::Command::new("sh");

        cargo_test_command.args(["-c".to_string(), Self::build_test_command(filter, flags)]);

        cargo_test_command
            .env(smplx_test::TEST_ENV_NAME, cache_path)
            .stdin(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdout(Stdio::inherit());

        cargo_test_command
    }

    fn build_test_command(filter: String, flags: &TestFlags) -> String {
        let mut command_as_arg = String::new();

        command_as_arg.push_str(&format!("cargo test {filter}_{SMPLX_TEST_MARKER}"));

        let flag_args = Self::build_test_flags(flags);

        if !flag_args.is_empty() {
            command_as_arg.push_str(" --");
            command_as_arg.push_str(&flag_args);
        }

        command_as_arg
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

    fn get_test_config_cache_name() -> Result<PathBuf, CommandError> {
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
