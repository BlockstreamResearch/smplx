use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initializes Simplex project
    Init {
        /// Name of the new project
        name: Option<String>,
    },
    /// Prints current Simplex config in use
    Config,
    /// Spins up local Electrs + Elements regtest
    Regtest,
    /// Runs Simplex tests
    Test {
        #[command(flatten)]
        args: TestArguments,

        #[command(flatten)]
        flags: TestFlags,
    },
    /// Install a `SimplicityHL` dependency (requires the dep to be a simplex project)
    Install,
    /// Generates the simplicity contracts artifacts
    Build,
    /// Cleans Simplex artifacts in the current directory
    Clean,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Clone)]
pub struct TestArguments {
    /// Space-separated test name filters
    #[arg(value_name = "FILTER", num_args = 0..)]
    pub filters: Vec<String>,
    /// Integration test target to run
    #[arg(long = "target")]
    pub target: Option<String>,
    /// Number of tests to run simultaneously
    #[arg(long = "test-threads")]
    pub test_threads: Option<std::num::NonZeroUsize>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Clone)]
pub struct TestFlags {
    /// Show detailed output about running tests
    #[arg(long = "show-output")]
    pub show_output: bool,
    /// Run ignored tests
    #[arg(long)]
    pub ignored: bool,
    /// Run tests regardless of failure
    #[arg(long = "no-fail-fast")]
    pub no_fail_fast: bool,
    /// Verbosity level for test output (-v for debug, -vv for trace)
    #[arg(short = 'v', long, action = clap::ArgAction::Count)]
    pub verbose: u8,
    /// Do not print cargo log messages
    #[arg(short = 'q', long)]
    pub quiet: bool,
    /// Run non-simplex tests (may be used for running unit tests)
    #[arg(long = "no-simplex")]
    pub no_simplex: bool,
    /// Perform fuzzing via simplex
    #[arg(long = "fuzz")]
    pub fuzz: bool,
}
