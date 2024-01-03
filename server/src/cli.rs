use std::{
    net::SocketAddr,
    time::{Duration, SystemTime},
};

use anyhow::Context;
use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)] // requires `derive` feature
struct Cli {
    #[arg(short, long, default_value = "127.0.0.1:8920")]
    gossip_listen_addr: SocketAddr,

    #[arg(short, long, default_value = "127.0.0.1:8920")]
    gossip_public_addr: SocketAddr,

    #[arg(short, long, default_value = "500")]
    gossip_intvl: u64,

    /// This node's unique identifier
    #[arg(short, long)]
    node_id: String,

    /// A comma separated list of seed node hostnames
    #[arg(short, long, value_parser, value_delimiter = ',', num_args = 1..)]
    seeds: Vec<String>,
}

pub async fn parse() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let gossip_api = crate::gossip::start_gossip(
        cli.gossip_listen_addr,
        cli.gossip_public_addr,
        cli.gossip_intvl,
        cli.node_id,
        cli.seeds,
    )
    .await
    .context("Failed to start gossip api")?;

    crate::frontend::start_frontend().await;

    Ok(())
}
