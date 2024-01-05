use std::{collections::HashSet, hash::Hasher};

use crate::cluster_monitor::{ClusterMonitor, ClusterNodeKey};
use conhash::{ConsistentHash, Node};
use futures::StreamExt;
use std::hash::Hash;

pub struct PartitionResolver<'a> {
    cluster_monitor: &'a mut ClusterMonitor,
    live_nodes: HashSet<ConHashKey>,
    conhash: ConsistentHash<ConHashKey>,
    replica_count: usize
}

impl<'a> PartitionResolver<'a> {
    pub fn new(cluster_monitor: &'a mut ClusterMonitor, replica_count: usize) -> Self {
        Self {
            cluster_monitor,
            live_nodes: HashSet::new(),
            conhash: ConsistentHash::new(),
            replica_count
        }
    }

    pub async fn start_sync(&mut self) {
        let mut str = self.cluster_monitor.watch().await;

        while let Some(cs) = str.next().await {
            let prev = self.live_nodes.clone();
            self.live_nodes.clear();

            cs.keys().for_each(|key| {
                self.live_nodes.insert(ConHashKey(key));
            });

            if self.live_nodes != prev {
                let added = self.live_nodes.difference(&prev);
                let removed = prev.difference(&self.live_nodes);

                for node in added {
                    self.conhash.add(node, self.replica_count);
                }

                for node in removed {
                    self.conhash.remove(node);
                }
            }
        }
    }

    pub fn resolve(&self, key: &[u8]) -> Option<&ClusterNodeKey> {
        let node = self.conhash.get(key);
        node.map(|node| &node.0)
    }
}

#[derive(Clone)]
struct ConHashKey(ClusterNodeKey);

impl Node for ConHashKey {
    fn name(&self) -> String {
        self.0.node_id().clone()
    }
}

impl PartialEq for ConHashKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.node_id() == other.0.node_id()
    }
}

impl Eq for ConHashKey {}

impl Hash for ConHashKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.node_id().hash(state);
    }
}
