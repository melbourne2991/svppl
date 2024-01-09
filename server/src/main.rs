use clap::Parser;
use server_lib::{app, opts};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = opts::Opts::parse();
    app::start(opts).await?;

    Ok(())
}
