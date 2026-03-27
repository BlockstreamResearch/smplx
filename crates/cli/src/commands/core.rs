use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Creates a new Simplex project in a new directory
    New {
        /// Name of the new project
        name: String,
    },
    /// Scaffolds an example Simplex project into a new directory
    Example {
        /// Name of the example to scaffold
        example: ExampleName,
    },
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
        #[command(subcommand)]
        command: TestCommand,
    },
    /// Generates the simplicity contracts artifacts
    Build,
    /// Clean Simplex artifacts in the current directory
    Clean,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ExampleName {
    /// Basic p2pk example with contract scaffolding
    Basic,
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

#[derive(Debug, Args, Copy, Clone)]
pub struct InitFlags {
    /// Generate a draft Rust library instead of just `Simplex.toml`
    #[arg(long)]
    pub lib: bool,
}
