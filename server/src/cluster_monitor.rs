use std::{
    collections::{BTreeMap, HashSet},
    hash::Hasher,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::Result;
use chitchat::{
    spawn_chitchat, transport::UdpTransport, Chitchat, ChitchatConfig, ChitchatHandle, ChitchatId,
    FailureDetectorConfig, NodeState,
};
use futures::StreamExt;
use std::hash::Hash;
use tokio::sync::Mutex;
use tokio_stream::wrappers::WatchStream;

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct ClusterNodeKey(ChitchatId);

impl ClusterNodeKey {
    pub fn node_id(&self) -> &String {
        &self.0.node_id
    }

    pub fn eq_node_id(&self, other: &ClusterNodeKey) -> bool {
        self.node_id().eq(other.node_id())
    }
}

#[derive(Clone, Default)]
pub struct ClusterState(BTreeMap<ChitchatId, NodeState>);

pub struct ClusterNode<'a>(&'a NodeState);

impl<'a> ClusterNode<'a> {
    pub fn grpc_addr(&self) -> Result<SocketAddr> {
        let maybe_endpoint_str = self.0.get("grpc_endpoint");
        let endpoint_str = maybe_endpoint_str.ok_or(anyhow::anyhow!("grpc_endpoint not found"))?;
        let endpoint = endpoint_str.parse::<SocketAddr>()?;

        Ok(endpoint)
    }
}

impl ClusterState {
    pub fn keys(&self) -> impl Iterator<Item = ClusterNodeKey> + '_ {
        self.0.keys().map(|key| ClusterNodeKey(key.clone()))
    }

    pub fn get(&self, key: &ClusterNodeKey) -> Option<ClusterNode> {
        self.0.get(&key.0).map(|ns| ClusterNode(ns))
    }
}

pub struct ClusterMonitorConfig {
    pub listen_addr: SocketAddr,
    pub public_addr: SocketAddr,
    pub intvl: u64,
    pub node_id: String,
    pub seeds: Vec<String>,

    pub initial_kv: Vec<(String, String)>,
}

pub struct ClusterMonitor {
    chitchat: Arc<Mutex<Chitchat>>,
    live_nodes: HashSet<ClusterNodeIdEq>,
}

#[derive(Clone)]
pub struct ClusterStateChange {
    pub added: Vec<ClusterNodeKey>,
    pub removed: Vec<ClusterNodeKey>,
    pub state: ClusterState,
}

impl ClusterMonitor {
    pub fn new(chitchat: Arc<Mutex<Chitchat>>) -> Self {
        Self {
            chitchat,
            live_nodes: HashSet::new(),
        }
    }

    pub async fn watch(&mut self) -> WatchStream<ClusterStateChange> {
        let locked = self.chitchat.lock().await;
        let mut changes = locked.live_nodes_watcher().map(|tree| ClusterState(tree));

        let (tx, rx) = tokio::sync::watch::channel(ClusterStateChange {
            added: Vec::new(),
            removed: Vec::new(),
            state: ClusterState::default(),
        });

        while let Some(cs) = changes.next().await {
            let prev = self.live_nodes.clone();
            self.live_nodes.clear();

            cs.keys().for_each(|key| {
                self.live_nodes.insert(ClusterNodeIdEq(key));
            });

            if self.live_nodes != prev {
                let added = self.live_nodes.difference(&prev);
                let removed = prev.difference(&self.live_nodes);

                tx.send(ClusterStateChange {
                    added: added.map(|node| node.0.clone()).collect(),
                    removed: removed.map(|node| node.0.clone()).collect(),
                    state: cs,
                })
                .unwrap();
            }
        }

        tokio_stream::wrappers::WatchStream::new(rx)
    }

    pub async fn self_key(&mut self) -> ClusterNodeKey {
        let locked = self.chitchat.lock().await;
        ClusterNodeKey(locked.self_chitchat_id().clone())
    }
}

pub async fn start_gossip(
    config: ClusterMonitorConfig,
) -> Result<(ClusterMonitor, ChitchatHandle)> {
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

    Ok((ClusterMonitor::new(chitchat), chitchat_handle))
}

#[derive(Clone)]
pub struct ClusterNodeIdEq(pub ClusterNodeKey);

impl PartialEq for ClusterNodeIdEq {
    fn eq(&self, other: &Self) -> bool {
        self.0.node_id() == other.0.node_id()
    }
}

impl Eq for ClusterNodeIdEq {}

impl Hash for ClusterNodeIdEq {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.node_id().hash(state);
    }
}
