// Topology module — 4 fixed governed topologies
//
// Invariant A4 (Verifiable-Reduction): All merge functions are
// deterministic and auditable by F3 TRI-WITNESS.
//
// Topology count justification (F4: "Too many paths = untestable governance"):
//   1. Pipeline     — sequential stages with review loop
//   2. Fan-Out      — parallel dispatch with deterministic merge
//   3. Cascade      — multi-agent escalation with F3 witness
//   4. ControlledCycle — convergent WORK→VERIFY→[PASS|FAIL] loops
//      Justified because: only missing base shape from 12-pattern taxonomy,
//      every convergent workflow needs it, governable via budget+convergence.

pub mod cascade;
pub mod controlled_cycle;
pub mod fan_out;
pub mod pipeline;

// Re-export key types from controlled_cycle for convenience
pub use controlled_cycle::{
    ControlledCycle, ControlledCycleConfig, ConvergenceState, CycleError, CycleExitCondition,
    CycleRound, CycleSummary, EscalationTarget,
};

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
