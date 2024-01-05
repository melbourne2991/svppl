use std::{
    net::SocketAddr,
    time::{Duration, SystemTime},
};

use anyhow::Context;
use clap::{Args, Parser, Subcommand, ValueEnum};
use nanoid::format;

use crate::{
    cluster_monitor::{self, ClusterMonitorConfig},
    resolve_addr,
};

#[derive(Debug, Parser)] // requires `derive` feature
struct Cli {
    #[arg(short, long, default_value = "localhost")]
    hostname: String,

    #[arg(short, long, default_value = "127.0.0.1:8920")]
    gossip_listen_addr: SocketAddr,

    #[arg(short, long, default_value = "8920")]
    gossip_port: u16,

    #[arg(short, long, default_value = "500")]
    gossip_intvl: u64,

    #[arg(short, long, default_value = "127.0.0.1:8921")]
    grpc_listen_addr: SocketAddr,

    #[arg(short, long, default_value = "8921")]
    grpc_port: u16,

    /// This node's unique identifier
    #[arg(short, long)]
    node_id: String,

    /// A comma separated list of seed node hostnames
    #[arg(short, long, value_parser, value_delimiter = ',', num_args = 1..)]
    seeds: Vec<String>,
}

pub async fn parse() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let gossip_public_addr = resolve_addr::resolve_socket_addr(&cli.hostname, cli.gossip_port)?;
    
    let config = ClusterMonitorConfig {
        listen_addr: cli.grpc_listen_addr,
        public_addr: gossip_public_addr,
        intvl: cli.gossip_intvl,
        node_id: cli.node_id,
        seeds: cli.seeds,

        initial_kv: vec![
            ("grpc_endpoint".to_string(), format!("{}:{}", cli.hostname, cli.grpc_port)),
        ],
    };

    let cluster_monitor = crate::cluster_monitor::start_gossip(config)
        .await
        .context("Failed to start gossip api")?;

    

    crate::frontend::start_frontend().await;
    

    Ok(())
}
