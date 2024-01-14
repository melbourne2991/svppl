use std::{
    collections::{btree_map::Entry, BTreeMap, HashSet},
    fmt::{Display, Formatter},
    hash::Hasher,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::Result;
use chitchat::{
    spawn_chitchat, transport::UdpTransport, Chitchat, ChitchatConfig, ChitchatHandle, ChitchatId,
    ChitchatIdGenerationEq, FailureDetectorConfig, NodeState,
};
use futures::StreamExt;
use std::hash::Hash;
use tokio::sync::{watch::Receiver, Mutex, RwLock};
use tokio_stream::wrappers::WatchStream;
use tonic::transport::Channel;

pub(crate) const GRPC_ENDPOINT_KEY: &str = "grpc_endpoint";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClusterNodeId(pub String);

impl Display for ClusterNodeId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Default)]
pub struct ClusterState(BTreeMap<ChitchatIdGenerationEq, NodeState>);

#[derive(Clone)]
pub struct ClusterNode {
    chitchat_id: ChitchatId,
    grpc_endpoint: SocketAddr,
    grpc_channel: Channel,
    generation_id: u64,
}

impl ClusterNode {
    pub fn try_new(chitchat_id: &ChitchatId, node_state: &NodeState) -> Result<Self> {
        let grpc_endpoint_str = node_state
            .get(GRPC_ENDPOINT_KEY)
            .ok_or_else(|| anyhow::anyhow!("grpc_endpoint not found"))?;

        let grpc_endpoint = grpc_endpoint_str.parse::<SocketAddr>()?;

        let grpc_channel = Channel::from_shared(grpc_endpoint.to_string())
            .map_err(|e| anyhow::anyhow!("failed to create channel: {}", e))?
            .connect_lazy();

        Ok(Self {
            chitchat_id: chitchat_id.clone(),
            grpc_channel,
            grpc_endpoint,
            generation_id: chitchat_id.generation_id,
        })
    }

    pub fn node_id(&self) -> ClusterNodeId {
        ClusterNodeId(self.chitchat_id.node_id.clone())
    }

    pub fn grpc_channel(&self) -> Channel {
        self.grpc_channel.clone()
    }
}

impl ClusterState {
    pub fn keys(&self) -> impl Iterator<Item = &ChitchatIdGenerationEq> {
        self.0.keys()
    }
}

#[derive(Debug)]
pub struct ClusterMonitorConfig {
    pub listen_addr: SocketAddr,
    pub public_addr: SocketAddr,
    pub intvl: u64,
    pub node_id: String,
    pub seeds: Vec<String>,

    pub initial_kv: Vec<(String, String)>,
}

#[derive(Clone)]
pub struct ClusterMonitor {
    chitchat: Arc<Mutex<Chitchat>>,
    nodes: Arc<RwLock<BTreeMap<ClusterNodeId, ClusterNode>>>,
    prev_states: Arc<RwLock<BTreeMap<ChitchatIdGenerationEq, NodeState>>>,
    change_rx: Arc<Mutex<Option<Receiver<Vec<ClusterStateChange>>>>>,
}

pub struct ClusterMonitorHandle {
    chitchat_handle: ChitchatHandle,
    cluster_monitor: ClusterMonitor,
}

impl ClusterMonitorHandle {
    pub async fn shutdown(self) -> Result<()> {
        let chitchat_handle = self.chitchat_handle;
        chitchat_handle.shutdown().await
    }

    pub fn cluster_monitor(&self) -> ClusterMonitor {
        self.cluster_monitor.clone()
    }
}

#[derive(Clone)]
pub enum ClusterStateChange {
    Added(ClusterNode),
    Removed(ClusterNode),
    Updated(ClusterNode),
}

pub type ClusterStateChangeset = Vec<ClusterStateChange>;

#[async_trait::async_trait]
pub trait ClusterStateChangeListener {
    async fn on_cluster_state_change(&mut self, changeset: ClusterStateChangeset);
}

impl ClusterMonitor {
    pub fn new(chitchat: Arc<Mutex<Chitchat>>) -> Self {
        Self {
            chitchat,
            nodes: Arc::new(RwLock::new(BTreeMap::new())),
            prev_states: Arc::new(RwLock::new(BTreeMap::new())),
            change_rx: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn watch(&self) -> WatchStream<Vec<ClusterStateChange>> {
        let guard = self.change_rx.lock().await;

        match &*guard {
            Some(rx) => WatchStream::new(rx.clone()),
            None => {
                let rx = self.cluster_change_receiver().await;
                *self.change_rx.lock().await = Some(rx.clone());
                WatchStream::new(rx)
            }
        }
    }

    async fn cluster_change_receiver(&self) -> Receiver<Vec<ClusterStateChange>> {
        let locked = self.chitchat.lock().await;

        let mut changes = locked.live_nodes_watcher();
        let (tx, rx) = tokio::sync::watch::channel(Vec::new());

        while let Some(next_states) = changes.next().await {
            let prev_states = self.prev_states.write().await;
            let mut nodes = self.nodes.write().await;

            let mut mapped_changes: Vec<ClusterStateChange> = Vec::new();

            let prev_keys = prev_states.keys().collect::<HashSet<_>>();
            let new_keys = next_states.keys().collect::<HashSet<_>>();

            let added_keys = new_keys.difference(&prev_keys).collect::<Vec<_>>();
            let removed_keys = prev_keys.difference(&new_keys).collect::<Vec<_>>();
            let unchanged_keys = prev_keys.intersection(&new_keys).collect::<Vec<_>>();

            let updated_keys = unchanged_keys.into_iter().filter(|key| {
                let maybe_prev_node_state = prev_states.get(&key);
                let maybe_next_node_state = next_states.get(&key);

                match (maybe_prev_node_state, maybe_next_node_state) {
                    (Some(prev_node_state), Some(next_node_state)) => {
                        prev_node_state.max_version() != next_node_state.max_version()
                    }
                    _ => {
                        tracing::error!("prev_node_state_is_none");
                        false
                    }
                }
            });

            for added in added_keys {
                let node_id = ClusterNodeId(added.0.node_id.clone());
                let maybe_entry = nodes.entry(node_id.clone());

                if let Entry::Occupied(entry) = maybe_entry {
                    let prev_node = entry.get();

                    if added.0.generation_id < prev_node.generation_id {
                        let dead_node_ip = added.0.gossip_advertise_addr.ip();

                        // A node chitchat considered dead has been resurrected.
                        tracing::warn!(node_id=%node_id, node_ip=%dead_node_ip, "dead_node_resurrected");

                        // Ignore this node.
                        continue;
                    }
                }

                let created_node = next_states
                    .get(added)
                    .map(|next_node_state| ClusterNode::try_new(&added.0, next_node_state))
                    .ok_or_else(|| anyhow::anyhow!("next_states must contain key"))
                    .and_then(|inner| inner);

                match created_node {
                    Ok(node) => {
                        nodes.insert(ClusterNodeId(added.0.node_id.clone()), node.clone());
                        mapped_changes.push(ClusterStateChange::Added(node));
                    }
                    Err(err) => {
                        tracing::error!(err = ?err, "cluster_node_creation_failed");
                        continue;
                    }
                }
            }

            for updated in updated_keys {
                let updated_node = next_states
                    .get(&updated)
                    .map(|next_node_state| ClusterNode::try_new(&updated.0, next_node_state))
                    .ok_or_else(|| anyhow::anyhow!("next_states must contain key"))
                    .and_then(|inner| inner);

                match updated_node {
                    Ok(node) => {
                        nodes.insert(ClusterNodeId(updated.0.node_id.clone()), node.clone());
                        mapped_changes.push(ClusterStateChange::Updated(node));
                    }
                    Err(err) => {
                        tracing::error!(err = ?err, "cluster_node_creation_failed");
                        continue;
                    }
                }
            }

            for removed in removed_keys {
                let node_id = ClusterNodeId(removed.0.node_id.clone());
                let maybe_entry = nodes.entry(node_id.clone());

                if let Entry::Occupied(entry) = maybe_entry {
                    let previous_node = entry.remove();

                    if previous_node.generation_id == removed.0.generation_id {
                        mapped_changes.push(ClusterStateChange::Removed(previous_node));
                    }
                }
            }

            if let Err(send_err) = tx.send(mapped_changes) {
                tracing::error!(err = ?send_err, "cluster_state_change_send_failed");
            }
        }

        rx
    }

    pub async fn self_id(&mut self) -> ClusterNodeId {
        let locked = self.chitchat.lock().await;
        ClusterNodeId(locked.self_chitchat_id().node_id.clone())
    }

    pub async fn get_node_channel(&self, node_id: &ClusterNodeId) -> Result<Channel> {
        let locked_nodes = self.nodes.read().await;

        locked_nodes
            .get(node_id)
            .map(|node| node.grpc_channel())
            .ok_or_else(|| anyhow::anyhow!("node not found: {}", node_id))
    }
}

pub async fn start(config: ClusterMonitorConfig) -> Result<ClusterMonitorHandle> {
    let generation = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let chitchat_id = ChitchatId::new(config.node_id, generation, config.public_addr);

    let chitchat_config = ChitchatConfig {
        cluster_id: "svppl_cluster".to_string(),
        chitchat_id,
        gossip_interval: Duration::from_millis(config.intvl),
        listen_addr: config.listen_addr,
        seed_nodes: config.seeds,
        failure_detector_config: FailureDetectorConfig::default(),
        marked_for_deletion_grace_period: 10_000,
    };

    let chitchat_handle = spawn_chitchat(chitchat_config, config.initial_kv, &UdpTransport).await?;

    let chitchat: std::sync::Arc<tokio::sync::Mutex<chitchat::Chitchat>> =
        chitchat_handle.chitchat();

    Ok(ClusterMonitorHandle {
        chitchat_handle,
        cluster_monitor: ClusterMonitor::new(chitchat),
    })
}

// pub async fn bind_cluster_state_change(
//     monitor: &mut ClusterMonitor,
//     mut listener: impl ClusterStateChangeListener + Send + 'static,
//     mut rx_shutdown_signal: oneshot::Receiver<()>,
// ) -> JoinHandle<()> {
//     let mut changes = monitor.watch().await.unwrap();

//     tokio::spawn(async move {
//         loop {
//             tokio::select! {
//                 _ = &mut rx_shutdown_signal => {
//                     break;
//                 },

//                 Some(changeset) = changes.next() => {
//                     &listener.on_cluster_state_change(changeset).await;
//                 }
//             }
//         }
//     })
// }
