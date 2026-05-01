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
        /// Name or a substring of the tests to run
        #[arg()]
        name: Option<String>,

        #[command(flatten)]
        additional_flags: TestFlags,
    },
    /// Generates the simplicity contracts artifacts
    Build,
    /// Clean Simplex artifacts in the current directory
    Clean,
}

#[derive(Debug, Args, Clone)]
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
    /// Log simplicity pruning stack trace
    #[arg(short = 'v', long)]
    pub verbose: bool,
}
