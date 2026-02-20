use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize project wiht default configuration
    Init,
    /// Show current configuration
    Config,
    /// Launch `elementsd` in regtest mode with a default config
    Regtest,
}
