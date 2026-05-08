use clap::Parser;

fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    smplx_cli::Cli::parse().run()?;

    Ok(())
}
