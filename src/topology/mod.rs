// Topology module — 3 fixed governed topologies
//
// Invariant A4 (Verifiable-Reduction): All merge functions are
// deterministic and auditable by F3 TRI-WITNESS.

pub mod fan_out;
pub mod pipeline;
pub mod cascade;

use thiserror::Error;

/// Shared error type for all topology operations
#[derive(Debug, Error)]
pub enum TopologyError {
    #[error("Divergent merge — results do not match claimed output (A4 violation)")]
    DivergentMerge,
    #[error("Node {0} failed: {1}")]
    NodeFailed(String, String),
    #[error("Channel error: {0}")]
    Channel(String),
    #[error("Lease expired or invalid (A1 violation)")]
    LeaseViolation,
    #[error("F3 witness divergence detected — 888_HOLD required")]
    WitnessDivergence,
}

/// A single node result for merge operations
#[derive(Debug, Clone)]
pub struct NodeResult {
    pub node_id: String,
    pub payload: Vec<u8>,
    pub receipt_hash: [u8; 32],
}
