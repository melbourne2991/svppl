use server_lib::{opts, app};
use clap::{Args, Parser, Subcommand, ValueEnum};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = opts::Opts::parse();
    app::start(opts).await?;

    Ok(())
}

