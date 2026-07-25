// arifFlow topology/fan_out.rs
// Fan-Out Topology: Parallel node dispatch with verifiable merge
//
// Invariant A4 (Verifiable-Reduction): Merge is deterministic and auditable.
//
// Topology:
//   Input ──▶ [Node A] ──▶
//              [Node B] ──▶  Merge ──▶ Output
//              [Node C] ──▶

use serde::{Deserialize, Serialize};
use super::{NodeResult, TopologyError};
use crate::channel::Channel;
use crate::merkle::{MerkleRoot, MerkleTree};

/// Configuration for a fan-out run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanOutConfig {
    pub node_ids: Vec<String>,
    pub max_concurrency: usize,
    pub merge_fn: MergeStrategy,
}

/// How the parallel outputs are merged
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeStrategy {
    /// Concatenate all outputs in node order (deterministic)
    OrderedConcat,
    /// Hash-merge: Merkle root of all output hashes
    MerkleRoot,
}

/// Fan-out topology executor
pub struct FanOutTopology {
    config: FanOutConfig,
    input_channel: Option<Channel<Vec<u8>>>,
    output_channel: Option<Channel<Vec<u8>>>,
}

impl FanOutTopology {
    pub fn new(config: FanOutConfig) -> Self {
        Self {
            config,
            input_channel: None,
            output_channel: None,
        }
    }

    /// Register input channel (receives the shared input state)
    pub fn set_input(&mut self, channel: Channel<Vec<u8>>) {
        self.input_channel = Some(channel);
    }

    /// Compute merged Merkle root from parallel node results.
    /// Invariant A4: merge is deterministic — same inputs → same output.
    pub fn merge_results(&self, results: &[NodeResult]) -> Result<Vec<u8>, TopologyError> {
        match self.config.merge_fn {
            MergeStrategy::OrderedConcat => {
                let mut merged = Vec::new();
                for r in results {
                    merged.extend_from_slice(&r.payload);
                }
                Ok(merged)
            }
            MergeStrategy::MerkleRoot => {
                let hashes: Vec<[u8; 32]> = results.iter().map(|r| r.receipt_hash).collect();
                let leaves: Vec<MerkleRoot> = hashes.into_iter().map(MerkleRoot).collect();
                let tree = MerkleTree::from_leaves(leaves).map_err(|_| TopologyError::DivergentMerge)?;
                Ok(tree.root().0.to_vec())
            }
        }
    }

    /// Verify that a merge result is consistent with the inputs (A4 audit).
    /// Returns Ok(count) where count = number of results consumed.
    pub fn verify_merge(
        &self,
        results: &[NodeResult],
        claimed_output: &[u8],
    ) -> Result<usize, TopologyError> {
        let expected = self.merge_results(results)?;
        if expected.as_slice() != claimed_output {
            return Err(TopologyError::DivergentMerge);
        }
        Ok(results.len())
    }

    pub fn config(&self) -> &FanOutConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(id: &str, payload: &[u8]) -> NodeResult {
        NodeResult {
            node_id: id.into(),
            payload: payload.to_vec(),
            receipt_hash: *blake3::hash(payload).as_bytes(),
        }
    }

    #[test]
    fn test_fanout_ordered_concat() {
        let config = FanOutConfig {
            node_ids: vec!["A".into(), "B".into()],
            max_concurrency: 2,
            merge_fn: MergeStrategy::OrderedConcat,
        };
        let topo = FanOutTopology::new(config);

        let results = vec![
            make_result("A", b"hello"),
            make_result("B", b"world"),
        ];
        let merged = topo.merge_results(&results).unwrap();
        assert_eq!(merged, b"helloworld");
    }

    #[test]
    fn test_fanout_merkle_root() {
        let config = FanOutConfig {
            node_ids: vec!["A".into(), "B".into()],
            max_concurrency: 2,
            merge_fn: MergeStrategy::MerkleRoot,
        };
        let topo = FanOutTopology::new(config);

        let results = vec![
            make_result("A", b"data_a"),
            make_result("B", b"data_b"),
        ];
        let merged = topo.merge_results(&results).unwrap();
        assert_eq!(merged.len(), 32); // Merkle root = 32 bytes
    }

    #[test]
    fn test_fanout_merge_verify() {
        let config = FanOutConfig {
            node_ids: vec!["A".into()],
            max_concurrency: 1,
            merge_fn: MergeStrategy::OrderedConcat,
        };
        let topo = FanOutTopology::new(config);

        let results = vec![make_result("A", b"verify_me")];
        let merged = topo.merge_results(&results).unwrap();
        let count = topo.verify_merge(&results, &merged).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_fanout_divergent_merge_detected() {
        let config = FanOutConfig {
            node_ids: vec!["A".into()],
            max_concurrency: 1,
            merge_fn: MergeStrategy::OrderedConcat,
        };
        let topo = FanOutTopology::new(config);

        let results = vec![make_result("A", b"real")];
        let tampered_output = b"fake";
        let result = topo.verify_merge(&results, tampered_output);
        assert!(result.is_err());
    }
}
