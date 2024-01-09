use crate::{cluster_monitor::ClusterMonitorConfig, resolve_addr};
use anyhow::Context;

pub async fn start(opts: crate::opts::Opts) -> anyhow::Result<()> {
    let gossip_public_addr =
        resolve_addr::resolve_socket_addr(&opts.hostname, opts.gossip_port).await?;

    let config = ClusterMonitorConfig {
        listen_addr: opts.grpc_listen_addr,
        public_addr: gossip_public_addr,
        intvl: opts.gossip_intvl,
        node_id: opts.node_id,
        seeds: opts.seeds,

        initial_kv: vec![(
            "grpc_endpoint".to_string(),
            format!("{}:{}", opts.hostname, opts.grpc_port),
        )],
    };

    let (_cluster_monitor, _handle) = crate::cluster_monitor::start_gossip(config)
        .await
        .context("Failed to start gossip api")?;

    crate::frontend::start_frontend().await;

    Ok(())
}
