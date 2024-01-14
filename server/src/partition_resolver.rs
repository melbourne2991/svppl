use std::sync::Arc;

use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tonic::transport::Channel;

use crate::cluster_monitor::{
    self, ClusterMonitor, ClusterNodeId, ClusterStateChange, ClusterStateChangeset,
};
use crate::conhash::{ConsistentHash, DefaultBytesHasher, Node};
use tokio_stream::StreamExt;
#[derive(Clone)]
pub struct PartitionResolver {
    conhash: Arc<RwLock<ConsistentHash<ClusterNodeId>>>,
    replica_count: usize,
    cluster_monitor: ClusterMonitor,
}

impl Node for ClusterNodeId {
    fn name(&self) -> String {
        self.0.clone()
    }
}

impl PartitionResolver {
    pub fn new(cluster_monitor: &ClusterMonitor, replica_count: usize, seed: (u64, u64)) -> Self {
        Self {
            cluster_monitor: cluster_monitor.clone(),
            conhash: Arc::new(RwLock::new(ConsistentHash::<
                ClusterNodeId,
                DefaultBytesHasher,
            >::with_seed((0, 0)))),
            replica_count: 10,
        }
    }

    pub async fn sync(&mut self, cs: &ClusterStateChangeset) {
        for node in cs {
            let mut conhash_guard = self.conhash.write().await;

            match node {
                ClusterStateChange::Added(node)
                    if node.node_id() != self.cluster_monitor.self_id().await =>
                {
                    conhash_guard.add(&node.node_id(), self.replica_count);
                }
                ClusterStateChange::Removed(node) => {
                    conhash_guard.remove(&node.node_id());
                }
                _ => {}
            }
        }
    }

    pub async fn resolve_node_id(&self, key: &[u8]) -> Option<ClusterNodeId> {
        let conhash_guard = self.conhash.read().await;
        conhash_guard.get(key).map(|value| value.clone())
    }

    pub async fn resolve(&self, key: &[u8]) -> Option<Channel> {
        let node_id = self.resolve_node_id(key).await?;
        let node = self.cluster_monitor.get_node_channel(&node_id).await;

        match node {
            Err(err) => {
                tracing::error!(err=?err, "node_id_missing_channel");
                None
            }
            Ok(channel) => Some(channel),
        }
    }
}

pub struct PartitionResolverHandle {
    partition_resolver: PartitionResolver,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    join_handle: JoinHandle<()>
}

impl PartitionResolverHandle {
    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.shutdown_tx.send(()).ok();
        self.join_handle.await.ok();

        Ok(())
    }

    pub fn partition_resolver(&self) -> PartitionResolver {
        self.partition_resolver.clone()
    }
}

pub async fn start(cluster_monitor: &ClusterMonitor) -> PartitionResolverHandle {
    let partition_resolver = PartitionResolver::new(&cluster_monitor, 50, (0, 0));
    let mut ws = cluster_monitor.watch().await;
    let mut pr = partition_resolver.clone();

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let sync_join_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    break;
                },

                Some(changeset) = ws.next() => {
                    pr.sync(&changeset).await;
                }
            }
        }
    });

    let handle = PartitionResolverHandle {
        partition_resolver,
        shutdown_tx,
        join_handle: sync_join_handle,
    };

    handle
}
