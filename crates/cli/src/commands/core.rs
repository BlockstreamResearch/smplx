use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initializes the Simplex project (TODO)
    Init,
    /// Prints the current Simplex config in use
    Config,
    /// Spins up the local Electrs + Elements regtest
    Regtest,
    /// Runs the Simplex tests
    Test {
        #[command(subcommand)]
        command: TestCommand,
    },
    /// Generates the simplicity contracts artifacts
    Build,
}

#[derive(Debug, Subcommand)]
pub enum TestCommand {
    /// Runs integration tests
    Integration {
        #[command(flatten)]
        additional_flags: TestFlags,
    },
    /// Runs specific tests
    Run {
        /// The list of test names to run
        #[arg(long)]
        tests: Vec<String>,
        #[command(flatten)]
        additional_flags: TestFlags,
    },
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
