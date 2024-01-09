use std::net::SocketAddr;

use crate::{
    cluster_monitor::{self, ClusterMonitorConfig},
    resolve_addr,
};
use anyhow::Context;
use clap::{Parser};

#[derive(Debug, Parser)] // requires `derive` feature
pub struct Opts {
    #[arg(short, long, default_value = "localhost")]
    pub hostname: String,

    #[arg(short, long, default_value = "127.0.0.1:8920")]
    pub gossip_listen_addr: SocketAddr,

    #[arg(short, long, default_value = "8920")]
    pub gossip_port: u16,

    #[arg(short, long, default_value = "500")]
    pub gossip_intvl: u64,

    #[arg(short, long, default_value = "127.0.0.1:8921")]
    pub grpc_listen_addr: SocketAddr,

    #[arg(short, long, default_value = "8921")]
    pub grpc_port: u16,

    /// This node's unique identifier
    #[arg(short, long)]
    pub node_id: String,

    /// A comma separated list of seed node hostnames
    #[arg(short, long, value_parser, value_delimiter = ',', num_args = 1..)]
    pub seeds: Vec<String>,
}
