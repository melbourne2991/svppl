use crate::{cluster_monitor::ClusterMonitorConfig, resolve_addr};
use anyhow::Context;
use tracing::info;

pub async fn start(opts: crate::opts::Opts) -> anyhow::Result<()> {
    info!(opts = ?opts, "app_start");

    let gossip_public_addr =
        resolve_addr::resolve_socket_addr(&opts.hostname, opts.gossip_port).await?;

    let config = ClusterMonitorConfig {
        listen_addr: opts.gossip_listen_addr,
        public_addr: gossip_public_addr,
        intvl: opts.gossip_intvl,
        node_id: opts.node_id,
        seeds: opts.seeds,

        initial_kv: vec![(
            "grpc_endpoint".to_string(),
            format!("{}:{}", opts.hostname, opts.grpc_port),
        )],
    };

    info!("gossip_start");
    let (mut cluster_monitor, _handle) = crate::cluster_monitor::start_gossip(config)
        .await
        .context("failed to start gossip api")?;

    info!("rpc_server_start");
    crate::rpc::server::start(opts.grpc_listen_addr, &mut cluster_monitor).await;

    Ok(())
}
