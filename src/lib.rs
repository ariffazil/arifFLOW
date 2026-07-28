// arifFlow — Governed Parallel Execution Engine
// Law: arifOS (Ω) · Flow: arifFlow (Φ) · Hands: A-FORGE (Ψ)
// DITEMPA BUKAN DIBERI
//
//! # arifFlow
//!
//! A governed parallel execution engine that replaces LangGraph's role under
//! arifOS constitutional law. arifFlow is a **scheduler, not a governor**.
//! It executes exactly 3 fixed topologies (fan-out, pipeline, cascade) under
//! lease-bound authority from arifOS 888_JUDGE.
//!
//! ## Architecture
//!
//! ```text
//! arifOS ──lease──▶ arifFlow ──schedule──▶ A-FORGE ──act──▶ World
//! World ──result──▶ A-FORGE ──receipt──▶ arifFlow ──seal──▶ arifOS
//! ```
//!
//! ## Invariants
//!
//! - **A1 Constitutional-First**: No execution without lease + 888_JUDGE
//! - **A2 Plane-Isolated**: State crosses planes only via signed envelopes
//! - **A3 Checkpoint-with-Verdict**: Each step persists Merkle root + verdict
//! - **A4 Verifiable-Reduction**: Merge functions are deterministic + auditable
//! - **A5 Metabolic-Closure**: Every run ends with VAULT999 receipt, leases closed

pub mod channel;
pub mod merkle;
pub mod receipt;
pub mod scheduler;

pub mod bridge;
pub mod governance;
pub mod topology;

/// Re-export core types at crate level
pub use channel::{Channel, ChannelId, Message};
pub use merkle::{MerkleRoot, MerkleTree};
pub use scheduler::{
    CheckpointEnvelope, SchedulerError, SuperStepResult, SuperStepScheduler, TopologyKind,
    VerdictClass,
};

/// Version constant — matches arifOS release cadence
pub const VERSION: &str = "2026.7.26";
pub const CODENAME: &str = "LAW_FLOW_HANDS";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_defined() {
        assert!(!VERSION.is_empty());
        assert_eq!(CODENAME, "LAW_FLOW_HANDS");
    }
}
