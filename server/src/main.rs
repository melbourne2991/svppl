use server_lib::cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cli::parse().await?;

    Ok(())
}

