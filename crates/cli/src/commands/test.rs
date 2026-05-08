use std::path::PathBuf;
use std::process::Stdio;

use smplx_test::config::Verbosity;
use smplx_test::{SMPLX_TEST_MARKER, TestConfig};

use super::core::{TestArguments, TestFlags};
use super::error::CommandError;

pub struct Test {}

impl Test {
    /// Runs tests based on the given configuration, filter, and flags.
    ///
    /// # Errors
    /// Returns a `CommandError` if building the cache filename fails, writing the config to file fails, or running the system process fails.
    ///
    /// # Panics
    /// Panics if the output of the cargo test command is not valid UTF-8.
    pub fn run(mut config: TestConfig, args: &TestArguments, flags: &TestFlags) -> Result<(), CommandError> {
        let cache_path = Self::get_test_config_cache_name()?;

        if flags.verbose {
            config.verbosity = Some(Verbosity(4));
        }

        config.to_file(&cache_path)?;

        let mut cargo_test_command = Self::build_cargo_test_command(&cache_path, args, flags);

        let output = cargo_test_command.output()?;

        match output.status.code() {
            Some(code) => {
                println!("Exit Status: {code}");

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

    fn build_cargo_test_command(
        cache_path: &PathBuf,
        args: &TestArguments,
        flags: &TestFlags,
    ) -> std::process::Command {
        let mut cargo_test_command = std::process::Command::new("cargo");
        cargo_test_command.arg("test");

        cargo_test_command.args(Self::build_cargo_test_args(args, flags));
        cargo_test_command.args(Self::build_test_bin_args(args, flags));

        cargo_test_command
            .env(smplx_test::TEST_ENV_NAME, cache_path)
            .stdin(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdout(Stdio::inherit());

        cargo_test_command
    }

    fn build_cargo_test_args(args: &TestArguments, flags: &TestFlags) -> Vec<String> {
        let mut cargo_test_args = Vec::new();

        if let Some(target) = &args.target {
            cargo_test_args.push("--test".into());
            cargo_test_args.push(target.clone());
        }

        if flags.no_fail_fast {
            cargo_test_args.push("--no-fail-fast".into());
        }

        cargo_test_args
    }

    fn build_test_bin_args(args: &TestArguments, flags: &TestFlags) -> Vec<String> {
        let mut test_bin_args = Vec::new();

        test_bin_args.push("--".into());

        // TODO: custom filters may run non-simplex tests due to cargo limitations. Figure out how to fix this
        if args.filters.is_empty() {
            test_bin_args.push(SMPLX_TEST_MARKER.to_string());
        } else {
            test_bin_args.extend(args.filters.iter().cloned());
        }

        test_bin_args.extend(Self::build_test_bin_flags(flags));

        test_bin_args
    }

    fn build_test_bin_flags(flags: &TestFlags) -> Vec<String> {
        let mut test_bin_args = Vec::new();

        if flags.nocapture {
            test_bin_args.push("--nocapture".into());
        }
        if flags.show_output {
            test_bin_args.push("--show-output".into());
        }
        if flags.ignored {
            test_bin_args.push("--ignored".into());
        }
        if flags.quiet {
            test_bin_args.push("--quiet".into());
        }

        test_bin_args
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
