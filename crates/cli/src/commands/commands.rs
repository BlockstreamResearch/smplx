use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub enum Command {
    Init,
    Config,
    Regtest,
    Test {
        #[command(subcommand)]
        command: TestCommand,
    },
    Build,
}

#[derive(Debug, Subcommand)]
pub enum TestCommand {
    Integration {
        #[command(flatten)]
        additional_flags: TestFlags,
    },
    Run {
        #[arg(long)]
        tests: Vec<String>,
        #[command(flatten)]
        additional_flags: TestFlags,
    },
}

#[derive(Debug, Args, Copy, Clone)]
pub struct TestFlags {
    #[arg(long)]
    pub nocapture: bool,
    #[arg(long = "show-output")]
    pub show_output: bool,
    #[arg(long)]
    pub ignored: bool,
}
