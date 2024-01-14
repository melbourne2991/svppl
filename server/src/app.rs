use crate::{
    cluster_monitor::{ClusterMonitorConfig, ClusterMonitorHandle},
    partition_resolver::{self, PartitionResolverHandle},
    resolve_addr,
    rpc::server::RpcServerHandle,
};
use anyhow::Context;
use tracing::info;

pub struct AppHandle {
    rpc_handle: RpcServerHandle,
    cluster_monitor_handle: ClusterMonitorHandle,
    partition_resolver_handle: PartitionResolverHandle,
}

impl AppHandle {
    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.rpc_handle.shutdown().await?;
        self.cluster_monitor_handle.shutdown().await?;
        self.partition_resolver_handle.shutdown().await?;

        Ok(())
    }
}

pub async fn start(opts: crate::opts::Opts) -> anyhow::Result<AppHandle> {
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

    let cluster_monitor_handle = crate::cluster_monitor::start(config)
        .await
        .context("failed to start gossip api")?;

    let partition_resolver_handle =
        partition_resolver::start(&cluster_monitor_handle.cluster_monitor()).await;

    let rpc_handle = crate::rpc::server::start(
        opts.grpc_listen_addr,
        partition_resolver_handle.partition_resolver(),
    )
    .await;

    let app_handle = AppHandle {
        rpc_handle,
        cluster_monitor_handle,
        partition_resolver_handle,
    };

    Ok(app_handle)
}
