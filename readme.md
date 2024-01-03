
1. Join the cluster
2. Look up node identifier in the database to find existing configuration for this node (which includes queue partition to node assignments)

Each queue partition belongs to a particular node on the hash ring.

The key that we pass to the consistent hashing algorithm is queueid/partition

Tasks are 