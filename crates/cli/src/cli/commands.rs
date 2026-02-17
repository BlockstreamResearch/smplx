use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show current configuration
    Config,
    /// Launch `elementsd` in regtest mode with a default config
    Regtest,
}
