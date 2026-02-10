use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show current configuration
    Config,
}
