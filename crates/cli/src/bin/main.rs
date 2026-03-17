use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    Box::pin(smplx_cli::Cli::parse().run()).await?;

    Ok(())
}
