// arifFlow governance/mod.rs — Flow-level governance
//
// Checkpoint management, VAULT999 sealing, Kabarkan tracing hooks.
// Every governance component operates under A3 (Checkpoint-with-Verdict)
// and A5 (Metabolic-Closure).

pub mod checkpoint;
pub mod cooling;
pub mod invariants;
pub mod kabarkan;
pub mod kabarkan_fq;
pub mod tri_witness;
pub mod vault999;

pub use checkpoint::{CheckpointError, CheckpointManager, CheckpointState};
pub use cooling::{Convergence, CoolingEntry, CoolingLedger, CoolingSummary, DriftSeverity};
pub use invariants::{
    ActorFlowState, EnforcerAction, FlowInvariant, FqThresholds, InvariantCheck, InvariantEnforcer,
    InvariantReport, InvariantStatus,
};
pub use kabarkan::KabarkanTracer;
pub use kabarkan_fq::{
    FqAlertEvent, FqAlertSeverity, FqCoolingCorrelationEvent, FqLaneEvent, FqSnapshotEvent,
    FqTrend, KabarkanFqInstrument,
};
pub use tri_witness::{
    AgentCandidate, ConsensusResolution, TriWitness, TriWitnessVerdict, WitnessChannel,
    WitnessMergeResult, resolve_consensus_with_tiebreaker,
};
pub use vault999::Vault999Sealer;

