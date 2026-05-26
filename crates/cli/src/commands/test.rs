use std::path::PathBuf;
use std::process::Stdio;

use smplx_sdk::global::Verbosity;
use smplx_test::{TestConfig, smplx_test_marker};

use super::core::{TestArguments, TestFlags};
use super::error::CommandError;

/// Nextest dsl variable to filter and use only simplex tests
const SMPLX_NEXTEST_DSL_TEST_MARKER: &str = concat!("test(/", smplx_test_marker!(), "$/)");

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

        if flags.verbose > Verbosity::MAX_VERBOSITY_LEVEL {
            return Err(CommandError::BadVersbosityMode(flags.verbose));
        }

        config.verbosity = std::cmp::max(config.verbosity, Verbosity::new(flags.verbose));

        config.to_file(&cache_path)?;

        let mut cargo_nextest_command = Self::build_cargo_nextest_command(&cache_path, args, flags);

        let output = cargo_nextest_command.output()?;

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

    fn build_cargo_nextest_command(
        cache_path: &PathBuf,
        args: &TestArguments,
        flags: &TestFlags,
    ) -> std::process::Command {
        let mut cargo_nextest_command = std::process::Command::new("smplx-nextest");
        cargo_nextest_command.arg("nextest");
        cargo_nextest_command.arg("run");

        cargo_nextest_command.args(Self::build_cargo_nextest_args(args, flags));
        cargo_nextest_command.args(Self::build_test_bin_flags(flags));

        cargo_nextest_command
            .env(smplx_test::TEST_ENV_NAME, cache_path)
            .stdin(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdout(Stdio::inherit());

        cargo_nextest_command
    }

    fn build_cargo_nextest_args(args: &TestArguments, flags: &TestFlags) -> Vec<String> {
        let mut cargo_nextest_args = Vec::new();

        if !args.filters.is_empty() {
            cargo_nextest_args.extend(args.filters.iter().cloned());
        }

        cargo_nextest_args.push("--filterset".into());

        let dsl_marker = if flags.no_simplex {
            format!("not {SMPLX_NEXTEST_DSL_TEST_MARKER}")
        } else {
            SMPLX_NEXTEST_DSL_TEST_MARKER.into()
        };

        if let Some(target) = &args.target {
            cargo_nextest_args.push(format!("binary({target}) and {dsl_marker}"));
        } else {
            cargo_nextest_args.push(dsl_marker);
        }

        cargo_nextest_args.extend(Self::build_cargo_nextest_flags(flags));

        cargo_nextest_args
    }

    fn build_cargo_nextest_flags(flags: &TestFlags) -> Vec<String> {
        let mut cargo_nextest_flags = Vec::new();

        if flags.no_fail_fast {
            cargo_nextest_flags.push("--no-fail-fast".into());
        }

        if flags.no_capture {
            cargo_nextest_flags.push("--no-capture".into());
        }

        if flags.quiet {
            cargo_nextest_flags.push("--cargo-quiet".into());
        }

        if flags.verbose != 0 {
            cargo_nextest_flags.push("--verbose".into());
            cargo_nextest_flags.push("--cargo-verbose".into());

            // `-vvv` verbosity level
            if flags.verbose == Verbosity::MAX_VERBOSITY_LEVEL {
                cargo_nextest_flags.push("--cargo-verbose".into());
            }
        }

        cargo_nextest_flags
    }

    fn build_test_bin_flags(flags: &TestFlags) -> Vec<String> {
        let mut test_bin_args = Vec::new();

        if flags.ignored {
            test_bin_args.push("--ignored".into());
        }

        if !test_bin_args.is_empty() {
            test_bin_args.insert(0, "--".into());
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
