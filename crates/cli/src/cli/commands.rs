use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize project wiht default configuration
    Init,
    /// Show the current configuration
    Config,
    /// Launch `elementsd` in regtest mode with a default config
    Regtest,
    /// Launch test with
    Test {
        #[command(subcommand)]
        command: TestCommand,
    },
}

/// Test management commands
#[derive(Debug, Subcommand)]
pub enum TestCommand {
    /// Run integration tests using simplex conventions
    Integration {
        #[command(flatten)]
        additional_flags: TestFlags,
    },
    /// Run only specific files by path for testing
    Run {
        #[arg(short = 't', long)]
        tests: Vec<String>,
        #[command(flatten)]
        additional_flags: TestFlags,
    },
}

/// Additional flags for tests management
#[derive(Debug, Args, Copy, Clone)]
pub struct TestFlags {
    /// Flag for not capturing output in tests
    #[arg(long)]
    pub nocapture: bool,
    /// Show output
    #[arg(long = "show-output")]
    pub show_output: bool,
    /// Run ignored tests
    #[arg(long = "ignored")]
    pub ignored: bool,
}
