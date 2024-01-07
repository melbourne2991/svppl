use std::{
    borrow::BorrowMut,
    collections::BTreeMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime}, hash::Hasher,
};

use anyhow::{Error, Result};
use chitchat::{
    spawn_chitchat, transport::UdpTransport, Chitchat, ChitchatConfig, ChitchatId,
    FailureDetectorConfig, NodeState,
};
use std::hash::Hash;
use futures::StreamExt;
use tokio::sync::{
    watch::{self, Receiver},
    Mutex,
};
use tokio_stream::{wrappers::WatchStream, Stream};

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
}

impl ClusterMonitor {
    pub fn new(chitchat: Arc<Mutex<Chitchat>>) -> Self {
        Self { chitchat }
    }

    pub async fn watch(&mut self) -> Box<dyn Stream<Item = ClusterState> + Send + Unpin> {
        let locked = self.chitchat.lock().await;
        let str = locked.live_nodes_watcher().map(|tree| ClusterState(tree));

        Box::new(str)
    }

    pub async fn self_key(&mut self) -> ClusterNodeKey {
        let locked = self.chitchat.lock().await;
        ClusterNodeKey(locked.self_chitchat_id().clone())
    }
}

pub async fn start_gossip(config: ClusterMonitorConfig) -> Result<ClusterMonitor> {
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

    let chitchat_handler =
        spawn_chitchat(chitchat_config, config.initial_kv, &UdpTransport).await?;

    let chitchat: std::sync::Arc<tokio::sync::Mutex<chitchat::Chitchat>> =
        chitchat_handler.chitchat();

    Ok(ClusterMonitor::new(chitchat))
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
