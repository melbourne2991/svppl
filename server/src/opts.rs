use clap::Parser;
use std::net::SocketAddr;

#[derive(Debug, Parser)] // requires `derive` feature
pub struct Opts {
    #[arg(long, default_value = "localhost")]
    pub hostname: String,

    #[arg(long, default_value = "127.0.0.1:8920")]
    pub gossip_listen_addr: SocketAddr,

    #[arg(long, default_value = "8920")]
    pub gossip_port: u16,

    #[arg(long, default_value = "500")]
    pub gossip_intvl: u64,

    #[arg(long, default_value = "127.0.0.1:8921")]
    pub grpc_listen_addr: SocketAddr,

    #[arg(long, default_value = "8921")]
    pub grpc_port: u16,

    /// This node's unique identifier
    #[arg(long)]
    pub node_id: String,

    /// A comma separated list of seed node hostnames
    #[arg(long, value_parser, value_delimiter = ',', num_args = 1..)]
    pub seeds: Vec<String>,
}
