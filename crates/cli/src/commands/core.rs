use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initializes Simplex project
    Init {
        #[command(flatten)]
        additional_flags: InitFlags,
    },
    /// Prints current Simplex config in use
    Config,
    /// Spins up the local Electrs + Elements regtest
    Regtest,
    /// Runs Simplex tests
    Test {
        /// Name of test to run
        #[arg()]
        filter: Option<String>,

        /// Integration tests to run
        #[arg(long = "test", num_args = 1)]
        integration_tests: Vec<String>,

        #[command(flatten)]
        additional_flags: TestFlags,
    },
    /// Generates the simplicity contracts artifacts
    Build,
    /// Clean Simplex artifacts in the current directory
    Clean,
}

#[derive(Debug, Args, Copy, Clone)]
pub struct InitFlags {
    /// Generate a draft Rust library instead of just `Simplex.toml`
    #[arg(long)]
    pub lib: bool,
}

#[derive(Debug, Args, Copy, Clone)]
pub struct TestFlags {
    /// Show output from successful tests
    #[arg(long)]
    pub nocapture: bool,
    /// Show grouped output after the test completion
    #[arg(long = "show-output")]
    pub show_output: bool,
    /// Run ignored tests
    #[arg(long)]
    pub ignored: bool,
}
