use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};

use anyhow::Result;
use chitchat::{
    spawn_chitchat, transport::UdpTransport, Chitchat, ChitchatConfig, ChitchatId,
    FailureDetectorConfig,
};
use tokio::sync::Mutex;
pub struct GossipApi {
    chitchat: Arc<Mutex<Chitchat>>,
}

impl GossipApi {
    pub fn new(chitchat: Arc<Mutex<Chitchat>>) -> Self {
        Self { chitchat }
    }

    
}

pub async fn start_gossip(
    listen_addr: SocketAddr,
    public_addr: SocketAddr,
    intvl: u64,
    node_id: String,
    seeds: Vec<String>,
) -> Result<GossipApi> {
    let generation = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let chitchat_id = ChitchatId::new(node_id, generation, public_addr);

    let config = ChitchatConfig {
        cluster_id: "svppl_cluster".to_string(),
        chitchat_id,
        gossip_interval: Duration::from_millis(intvl),
        listen_addr,
        seed_nodes: seeds.clone(),
        failure_detector_config: FailureDetectorConfig::default(),
        marked_for_deletion_grace_period: 10_000,
    };

    let chitchat_handler = spawn_chitchat(config, Vec::new(), &UdpTransport).await?;
    let chitchat: std::sync::Arc<tokio::sync::Mutex<chitchat::Chitchat>> =
        chitchat_handler.chitchat();

    Ok(GossipApi::new(chitchat))
}
