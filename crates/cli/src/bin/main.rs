#![warn(clippy::all, clippy::pedantic)]

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    simplex_cli::logging::init();

    Box::pin(simplex_cli::cli::Cli::parse().run()).await?;

    Ok(())
}
