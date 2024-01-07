use std::{collections::HashSet, hash::Hasher};

use crate::cluster_monitor::{ClusterMonitor, ClusterNodeKey, ClusterState, ClusterNodeIdEq};
use conhash::{ConsistentHash, Node};
use futures::StreamExt;
use std::hash::Hash;


pub struct PartitionResolver {
    live_nodes: HashSet<ClusterNodeIdEq>,
    conhash: ConsistentHash<ClusterNodeIdEq>,
    replica_count: usize,
}

impl Node for ClusterNodeIdEq {
    fn name(&self) -> String {
        self.0.node_id().clone()
    }
}

impl PartitionResolver {
    pub fn new(replica_count: usize) -> Self {
        Self {
            live_nodes: HashSet::new(),
            conhash: ConsistentHash::new(),
            replica_count,
        }
    }

    pub async fn sync(&mut self, cs: &ClusterState) {
        let prev = self.live_nodes.clone();
        self.live_nodes.clear();

        cs.keys().for_each(|key| {
            self.live_nodes.insert(ClusterNodeIdEq(key));
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

    pub fn resolve(&self, key: &[u8]) -> Option<&ClusterNodeKey> {
        let node = self.conhash.get(key);
        node.map(|node| &node.0)
    }
}

