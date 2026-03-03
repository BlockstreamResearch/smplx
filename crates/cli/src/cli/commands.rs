use crate::config::ConfigOverride;
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize a project with the default configuration
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
    Build,
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

/// Build override arguments
#[derive(Debug, Args, Clone)]
pub struct OverrideArgs {
    #[command(flatten)]
    pub build_args: BuildOverrideArgs,
}

/// Build override arguments
#[derive(Debug, Args, Clone)]
pub struct BuildOverrideArgs {
    /// Output directory for build artifacts
    #[arg(global = true, long, env = "OUT_DIR")]
    pub out_dir: Option<PathBuf>,
    /// Flag to generate only files for contracts without module artifacts
    #[arg(global = true, long)]
    pub only_files: bool,
}

impl OverrideArgs {
    pub fn generate(self) -> Option<ConfigOverride> {
        Some(ConfigOverride {
            rpc_creds: None,
            network: None,
            build_conf: if self.build_args.out_dir.is_none() && !self.build_args.only_files {
                None
            } else {
                Some(self.build_args)
            },
        })
    }
}
