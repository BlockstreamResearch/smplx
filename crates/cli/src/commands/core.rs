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
    /// If `deps` is empty, install everything from `Simplex.toml`.
    Install {
        /// Dependencies to install, as `<source>` or `<alias>=<source>`.
        /// The bare name `std` pins the latest `SimplicityHL` standard library release.
        #[arg(value_name = "DEP")]
        deps: Vec<String>,
    },
    /// Generates the simplicity contracts artifacts
    Build,
    /// Cleans Simplex artifacts in the current directory
    Clean {
        #[command(flatten)]
        flags: CleanFlags,
    },
    /// Formats the configured Simplex source files using simfmt
    Fmt {
        #[command(flatten)]
        opts: FormatOpts,
    },
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
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Clone)]
pub struct CleanFlags {
    /// Remove all files created by Simplex, including installed dependencies
    #[arg(long = "all")]
    pub remove_all: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args)]
pub struct FormatOpts {
    /// Path to the file.
    #[arg(value_hint = clap::ValueHint::FilePath, value_name = "PATH", num_args(1..))]
    pub files: Vec<std::path::PathBuf>,

    /// No output printed to stdout
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Use verbose output
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Print simfmt version and exit
    #[arg(long = "version")]
    pub version: bool,

    /// Specify path to Simplex.toml
    #[arg(long = "manifest-path", value_name = "manifest-path")]
    pub manifest_path: Option<String>,

    #[arg(
        short = 'f',
        long = "message-format",
        value_name = "message-format",
        help = format!("Specify message-format: {}", crate::commands::fmt::MessageFormat::OPTIONS)
    )]
    pub message_format: Option<String>,

    /// Options passed to simfmt
    // `raw = true` makes the `--` separator explicit.
    #[arg(id = "simfmt_options", raw = true)]
    pub simfmt_options: Vec<String>,

    /// Run simfmt in check mode
    #[arg(long = "check")]
    pub check: bool,
}
