use crate::cluster_monitor::{ClusterNodeIdEq, ClusterNodeKey, ClusterStateChange};
use crate::conhash::{ConsistentHash, Node, DefaultBytesHasher};

pub struct PartitionResolver {
    conhash: ConsistentHash<ClusterNodeIdEq>,
    replica_count: usize,
}

impl Node for ClusterNodeIdEq {
    fn name(&self) -> String {
        self.0.node_id().clone()
    }
}

impl PartitionResolver {
    pub fn new(replica_count: usize, seed: (u64, u64)) -> Self {
        Self {
            conhash: ConsistentHash::<ClusterNodeIdEq, DefaultBytesHasher>::with_seed(seed),
            replica_count,
        }
    }

    pub async fn sync(&mut self, cs: &ClusterStateChange) {
        for node in &cs.added {
            self.conhash
                .add(&ClusterNodeIdEq(node.clone()), self.replica_count);
        }

        for node in &cs.removed {
            self.conhash.remove(&ClusterNodeIdEq(node.clone()));
        }
    }

    pub fn resolve(&self, key: &[u8]) -> Option<&ClusterNodeKey> {
        let node = self.conhash.get(key);
        node.map(|node| &node.0)
    }
}
