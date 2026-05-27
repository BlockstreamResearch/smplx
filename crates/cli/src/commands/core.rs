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
    /// Generates the simplicity contracts artifacts
    Build,
    /// Clean Simplex artifacts in the current directory
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
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Clone)]
pub struct TestFlags {
    /// Show output from successful tests
    #[arg(long = "no-capture", alias = "nocapture")]
    pub no_capture: bool,
    /// Show grouped output after the test completion
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
    /// Display one character per test instead of one line
    #[arg(short = 'q', long)]
    pub quiet: bool,
    /// Run non-simplex tests (may be used for running unit tests)
    #[arg(long = "no-simplex")]
    pub no_simplex: bool,
}
