// Copyright 2016 conhash-rs developers
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use siphasher::sip::SipHasher24;
use std::collections::{BTreeMap, HashMap};

pub trait BytesHasher {
    fn hash(&self, bytes: &[u8]) -> Vec<u8>;
}

#[derive(Default)]
pub struct DefaultBytesHasher {
    sip_hasher: SipHasher24,
}

impl DefaultBytesHasher {
    fn with_seed((p1, p2): (u64, u64)) -> Self {
        DefaultBytesHasher {
            sip_hasher: SipHasher24::new_with_keys(p1, p2),
        }
    }
}

impl BytesHasher for DefaultBytesHasher {
    fn hash(&self, bytes: &[u8]) -> Vec<u8> {
        self.sip_hasher.hash(bytes).to_le_bytes().to_vec()
    }
}

/// Consistent Hash
pub struct ConsistentHash<N: Node, H: BytesHasher = DefaultBytesHasher> {
    hash_fn: H,
    nodes: BTreeMap<Vec<u8>, N>,
    replicas: HashMap<String, usize>,
}

impl<N: Node, H: BytesHasher> ConsistentHash<N, H> {
    /// Construct with customized hash function
    pub fn with_hash(hash_fn: H) -> ConsistentHash<N, H> {
        ConsistentHash {
            hash_fn,
            nodes: BTreeMap::new(),
            replicas: HashMap::new(),
        }
    }

    pub fn with_seed(seed: (u64, u64)) -> ConsistentHash<N, DefaultBytesHasher> {
        ConsistentHash::with_hash(DefaultBytesHasher::with_seed(seed))
    }

    /// Add a new node
    pub fn add(&mut self, node: &N, num_replicas: usize) {
        let node_name = node.name();

        // Remove it first
        self.remove(node);

        self.replicas.insert(node_name.clone(), num_replicas);
        for replica in 0..num_replicas {
            let node_ident = format!("{}:{}", node_name, replica);
            let key = self.hash_fn.hash(node_ident.as_bytes());

            self.nodes.insert(key, node.clone());
        }
    }

    /// Get a node by key. Return `None` if no valid node inside
    pub fn get<'a>(&'a self, key: &[u8]) -> Option<&'a N> {
        if self.nodes.is_empty() {
            return None;
        }

        let hashed_key = self.hash_fn.hash(key);
        let entry = self.nodes.range(hashed_key..).next();

        if let Some((_k, v)) = entry {
            return Some(v);
        }

        // Back to the first one
        let first = self.nodes.iter().next();
        let (_k, v) = first.unwrap();

        Some(v)
    }

    /// Get a node by string key
    pub fn get_str<'a>(&'a self, key: &str) -> Option<&'a N> {
        self.get(key.as_bytes())
    }

    /// Get a node by key. Return `None` if no valid node inside
    pub fn get_mut<'a>(&'a mut self, key: &[u8]) -> Option<&'a mut N> {
        let hashed_key = self.get_node_hashed_key(key);
        hashed_key.and_then(move |k| self.nodes.get_mut(&k))
    }

    // Get a node's hashed key by key. Return `None` if no valid node inside
    fn get_node_hashed_key(&self, key: &[u8]) -> Option<Vec<u8>> {
        if self.nodes.is_empty() {
            return None;
        }

        let hashed_key = self.hash_fn.hash(key);

        let entry = self.nodes.range(hashed_key..).next();
        if let Some((k, _)) = entry {
            return Some(k.clone());
        }

        // Back to the first one

        let first = self.nodes.iter().next();

        let (k, _) = first.unwrap();

        Some(k.clone())
    }

    /// Get a node by string key
    pub fn get_str_mut<'a>(&'a mut self, key: &str) -> Option<&'a mut N> {
        self.get_mut(key.as_bytes())
    }

    /// Remove a node with all replicas (virtual nodes)
    pub fn remove(&mut self, node: &N) {
        let node_name = node.name();

        let num_replicas = match self.replicas.remove(&node_name) {
            Some(val) => val,
            None => {
                return;
            }
        };

        for replica in 0..num_replicas {
            let node_ident = format!("{}:{}", node.name(), replica);
            let key = self.hash_fn.hash(node_ident.as_bytes());
            self.nodes.remove(&key);
        }
    }

    /// Number of nodes
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<N: Node> Default for ConsistentHash<N, DefaultBytesHasher> {
    /// Construct with default hash function (Md5)
    fn default() -> ConsistentHash<N, DefaultBytesHasher> {
        ConsistentHash::with_hash(DefaultBytesHasher::default())
    }
}

pub trait Node: Clone {
    fn name(&self) -> String;
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug, Clone, Eq, PartialEq)]
    struct ServerNode {
        host: String,
        port: u16,
    }

    impl Node for ServerNode {
        fn name(&self) -> String {
            format!("{}:{}", self.host, self.port)
        }
    }

    impl ServerNode {
        fn new(host: &str, port: u16) -> ServerNode {
            ServerNode {
                host: host.to_owned(),
                port,
            }
        }
    }

    #[test]
    fn test_balance() {
        let nodes = [
            ServerNode::new("localhost", 12345),
            ServerNode::new("localhost", 12346),
            ServerNode::new("localhost", 12347),
            ServerNode::new("localhost", 12348),
            ServerNode::new("localhost", 12349),
            ServerNode::new("localhost", 12350),
            ServerNode::new("localhost", 12351),
            ServerNode::new("localhost", 12352),
            ServerNode::new("localhost", 12353),
        ];

        const REPLICAS: usize = 50;

        let mut ch = ConsistentHash::<ServerNode, DefaultBytesHasher>::default();

        for node in nodes.iter() {
            ch.add(node, REPLICAS);
        }

        let mut hash_map: HashMap<String, usize> = HashMap::new();

        for item in 0..1000 {
            let server_id = ch.get_str(format!("node_{}", item.to_string()).as_str()).unwrap();
            let count = hash_map.get(&server_id.name());
            let new_count = count.map_or(0, |c| c + 1);
            hash_map.insert(server_id.name(), new_count);
        }

        for (key, value) in &hash_map {
            println!("{}: {}", key, value);
        }

        let sum = hash_map.values().fold(0, |acc: usize, count| acc + count);
        let count = hash_map.len() as f64;
        let mean: f64 = sum as f64 / count;

        let mut sqdiff: HashMap<String, f64> = HashMap::new();

        for (key, value) in hash_map {
            sqdiff.insert(key, (value as f64 - mean).powf(2.0));
        }

        let sqdiff_total = sqdiff.values().fold(0.0, |acc: f64, count| acc + count);
        let variance = sqdiff_total / count;
        let stdev = variance.sqrt();

        println!("std deviation: {stdev}");

        assert!(stdev < 20.0);  
    }

    #[test]
    fn test_basic() {
        let nodes = [
            ServerNode::new("localhost", 12345),
            ServerNode::new("localhost", 12346),
            ServerNode::new("localhost", 12347),
            ServerNode::new("localhost", 12348),
            ServerNode::new("localhost", 12349),
            ServerNode::new("localhost", 12350),
            ServerNode::new("localhost", 12351),
            ServerNode::new("localhost", 12352),
            ServerNode::new("localhost", 12353),
        ];

        const REPLICAS: usize = 20;

        let mut ch = ConsistentHash::<ServerNode, DefaultBytesHasher>::default();

        for node in nodes.iter() {
            ch.add(node, REPLICAS);
        }

        assert_eq!(ch.len(), nodes.len() * REPLICAS);

        let example_node = ch.get_str("hindrance").unwrap().clone();
        assert_eq!(example_node, ServerNode::new("localhost", 12346));

        ch.remove(&ServerNode::new("localhost", 12353));
        assert_eq!(ch.get_str("hindrance").unwrap().clone(), example_node);
        assert_eq!(ch.len(), (nodes.len() - 1) * REPLICAS);

        ch.remove(&ServerNode::new("localhost", 12346));
        assert_ne!(ch.get_str("hindrance").unwrap().clone(), example_node);

        assert_eq!(ch.len(), (nodes.len() - 2) * REPLICAS);
    }

    #[test]
    fn get_from_empty() {
        let mut ch = ConsistentHash::<ServerNode, DefaultBytesHasher>::default();
        assert_eq!(ch.get_str(""), None);
        assert_eq!(ch.get_str_mut(""), None);
    }

    #[test]
    fn get_from_one_node() {
        let mut node = ServerNode::new("localhost", 12345);
        for replicas in 1..10_usize {
            let mut ch = ConsistentHash::<ServerNode, DefaultBytesHasher>::default();
            ch.add(&node, replicas);
            assert_eq!(ch.len(), replicas);
            for i in 0..replicas * 100 {
                let s = format!("{}", i);
                assert_eq!(ch.get_str(&s), Some(&node));
                assert_eq!(ch.get_str_mut(&s), Some(&mut node));
            }
        }
    }

    #[test]
    fn get_from_two_nodes() {
        let mut node0 = ServerNode::new("localhost", 12345);
        let mut node1 = ServerNode::new("localhost", 54321);
        for replicas in 1..10_usize {
            let mut ch = ConsistentHash::<ServerNode, DefaultBytesHasher>::default();
            ch.add(&node0, replicas);
            ch.add(&node1, replicas);
            assert_eq!(ch.len(), 2 * replicas);
            for i in 0..replicas * 100 {
                let s = format!("{}", i);
                let n = ch.get_str(&s).unwrap();
                assert!(n == &node0 || n == &node1);
                let n = ch.get_str(&s).unwrap();
                assert!(n == &mut node0 || n == &mut node1);
            }
        }
    }

    #[test]
    fn get_exact_node() {
        let mut ch = ConsistentHash::<ServerNode, DefaultBytesHasher>::default();
        const NODES: usize = 1000;
        const REPLICAS: usize = 20;
        let mut nodes = Vec::<ServerNode>::with_capacity(NODES);
        for i in 0..NODES {
            let node = ServerNode::new("localhost", 10000 + i as u16);
            ch.add(&node, REPLICAS);
            nodes.push(node);
        }
        assert_eq!(ch.len(), NODES * REPLICAS);
        for i in 0..NODES {
            for r in 0..REPLICAS {
                let s = format!("{}:{}", nodes[i].name(), r);
                assert_eq!(ch.get_str(&s), Some(&nodes[i]));
                assert_eq!(ch.get_str_mut(&s).cloned().as_ref(), Some(&nodes[i]));
            }
        }
    }
}
