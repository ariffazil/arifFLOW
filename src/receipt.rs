// arifFlow — Governed Parallel Execution Engine
// Flow Receipt v1 — Unit atom of governed flow
// DITEMPA BUKAN DIBERI

//! # Flow Receipt v1
//!
//! The unit atom of governed flow. Every hop, every execute, every verify,
//! every cool — recorded in an immutable, chained, Merkle-anchored receipt.
//!
//! ## Flow Quotient (FQ)
//!
//! The primary metric for measuring whether an agent is **in flow** or
//! trapped in self-monitoring:
//!
//! ```text
//! FQ = Σ(Execute.cost_ns) / Σ(Verify.cost_ns + preceding_verify_cost_ns)
//! ```
//!
//! | FQ Range | Verdict | Meaning |
//! |----------|---------|---------|
//! | > 10.0   | Overheat | Execute far outruns verify — under-verification risk. THROTTLE. |
//! | 3.0–10.0 | Optimal | Agent in flow. Governance in the architecture. |
//! | 1.0–3.0  | Balanced | Healthy verification. |
//! | 0.5–1.0  | Watching | Self-monitoring competes with execution. |
//! | < 0.5    | Stuck | Self-monitoring has become the task. mPFC takeover. |

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::fmt;
use uuid::Uuid;

use crate::merkle::MerkleRoot;

// ── Step Type ────────────────────────────────────────────────────────────

/// The kind of atomic step this receipt records.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepType {
    /// Actual work — computation, forge, deploy
    Execute,
    /// Verification, audit, floor check
    Verify,
    /// Cooling queue action — hold, clamp, bypass
    Cool,
    /// VAULT999 seal — irreversible commit
    Seal,
    /// Parallel barrier — wait for N lanes
    Barrier,
    /// Merge step — combine N lane outputs
    Merge,
    /// Routing — dispatch to another organ
    Route,
}

impl StepType {
    /// Returns true if this step type is counted as execution in FQ computation.
    pub fn is_execution(&self) -> bool {
        matches!(self, StepType::Execute | StepType::Seal | StepType::Merge)
    }

    /// Returns true if this step type is counted as verification in FQ computation.
    pub fn is_verification(&self) -> bool {
        matches!(self, StepType::Verify)
    }

    /// Returns true if this step type is a barrier/heartbeat step.
    pub fn is_barrier(&self) -> bool {
        matches!(self, StepType::Barrier)
    }
}

impl fmt::Display for StepType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StepType::Execute => write!(f, "Execute"),
            StepType::Verify => write!(f, "Verify"),
            StepType::Cool => write!(f, "Cool"),
            StepType::Seal => write!(f, "Seal"),
            StepType::Barrier => write!(f, "Barrier"),
            StepType::Merge => write!(f, "Merge"),
            StepType::Route => write!(f, "Route"),
        }
    }
}

// ── Epistemic Label ──────────────────────────────────────────────────────

/// Truth status of this step's output per F2/F7.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EpistemicLabel {
    /// OBS — Direct sensed reality
    Observation,
    /// DER — Logical deduction from evidence
    Derivation,
    /// INT — Inference under uncertainty
    Interpretation,
    /// SPEC — Plan or intended action
    Specification,
    /// SEAL — Irreversible commitment
    Seal,
}

impl fmt::Display for EpistemicLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EpistemicLabel::Observation => write!(f, "OBS"),
            EpistemicLabel::Derivation => write!(f, "DER"),
            EpistemicLabel::Interpretation => write!(f, "INT"),
            EpistemicLabel::Specification => write!(f, "SPEC"),
            EpistemicLabel::Seal => write!(f, "SEAL"),
        }
    }
}

// ── Risk Class ────────────────────────────────────────────────────────────

/// Autonomy tier classification for risk-weighted FQ thresholds.
///
/// Maps F13 SOVEREIGN autonomy ladder to FQ enforcement floors.
/// Higher risk = higher minimum FQ required before execution.
///
/// | RiskClass | FQ Required | Maps to |
/// |-----------|-------------|---------|
/// | T0Observe | 0.1 | Read, probe, grep |
/// | T1Mutate  | 0.3 | Edit, test, commit |
/// | T2Deploy  | 0.5 | Deploy, restart, multi-file |
/// | T3Irreversible | 1.0 | Irreversible, credential rotation, F13-gated |
///
/// Forged 2026-08-14 — FQ vector operationalization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum RiskClass {
    /// T0: Read, probe, observe — minimal risk. FQ floor: 0.1
    #[default]
    T0Observe,
    /// T1: Edit, test, commit, lint — moderate risk. FQ floor: 0.3
    T1Mutate,
    /// T2: Deploy, restart, multi-file — elevated risk. FQ floor: 0.5
    T2Deploy,
    /// T3: Irreversible, credential rotation, production — critical risk. FQ floor: 1.0
    T3Irreversible,
}

impl RiskClass {
    /// Minimum FQ required before this risk class can execute.
    /// Higher risk = higher verification floor.
    /// Derived from F13 SOVEREIGN autonomy ladder.
    pub fn fq_required(&self) -> f64 {
        match self {
            Self::T0Observe => 0.1,
            Self::T1Mutate => 0.3,
            Self::T2Deploy => 0.5,
            Self::T3Irreversible => 1.0,
        }
    }

    /// Short code for logging and JSON serialization.
    pub fn code(&self) -> &'static str {
        match self {
            Self::T0Observe => "T0",
            Self::T1Mutate => "T1",
            Self::T2Deploy => "T2",
            Self::T3Irreversible => "T3",
        }
    }
}

impl fmt::Display for RiskClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

// ── Floor Verdict ────────────────────────────────────────────────────────

/// F1–F13 constitutional verdict for this step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FloorVerdict {
    /// All applicable floors satisfied
    Pass,
    /// Soft floor tension (F5/F6) — proceed with awareness
    Caution,
    /// Hard floor violation — 888_HOLD
    Hold,
    /// Critical violation — blocked permanently
    Void,
}

impl fmt::Display for FloorVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FloorVerdict::Pass => write!(f, "PASS"),
            FloorVerdict::Caution => write!(f, "CAUTION"),
            FloorVerdict::Hold => write!(f, "HOLD"),
            FloorVerdict::Void => write!(f, "VOID"),
        }
    }
}

// ── Cooling Decision ─────────────────────────────────────────────────────

/// Cooling queue action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CoolingDecision {
    /// No cooling needed
    None,
    /// Cool down — pause execution
    Hold,
    /// Reduce intensity/speed
    Clamp,
    /// Expedite — skip cooling queue
    Bypass,
}

impl fmt::Display for CoolingDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoolingDecision::None => write!(f, "NONE"),
            CoolingDecision::Hold => write!(f, "HOLD"),
            CoolingDecision::Clamp => write!(f, "CLAMP"),
            CoolingDecision::Bypass => write!(f, "BYPASS"),
        }
    }
}

// ── Tri-Witness Votes ────────────────────────────────────────────────────

/// Aggregated witness scores for F3 TRI-WITNESS compliance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TriWitnessVotes {
    /// Human witness confidence (0.0 – 1.0)
    pub human: f64,
    /// AI witness confidence (0.0 – 1.0)
    pub ai: f64,
    /// Earth/data witness confidence (0.0 – 1.0)
    pub earth: f64,
}

impl TriWitnessVotes {
    /// Create a new TriWitnessVotes with validated ranges.
    pub fn new(human: f64, ai: f64, earth: f64) -> Result<Self, String> {
        for (name, val) in [("human", human), ("ai", ai), ("earth", earth)] {
            if !(0.0..=1.0).contains(&val) {
                return Err(format!(
                    "{} witness vote must be 0.0–1.0, got {}",
                    name, val
                ));
            }
        }
        Ok(Self { human, ai, earth })
    }

    /// Compute the Nash-aggregated witness score (F3 threshold: ≥ 0.75).
    pub fn nash_score(&self) -> f64 {
        self.human * self.ai * self.earth
    }

    /// Returns true if the Nash score meets the F3 TRI-WITNESS threshold.
    pub fn meets_f3_threshold(&self) -> bool {
        self.nash_score() >= 0.75
    }
}

impl Default for TriWitnessVotes {
    fn default() -> Self {
        Self {
            human: 0.0,
            ai: 0.0,
            earth: 0.0,
        }
    }
}

// ── Flow Verdict ─────────────────────────────────────────────────────────

/// Flow health verdict based on Flow Quotient (v2.2 — 2026-08-14).
///
/// Seven-state band per Arif F13 spec + Helix Codex Lock 2:
///   UNKNOWN    — verify_count == 0 (missing data)
///   CAUTION    — verify_count < 2 (insufficient pattern)
///   FOSSILIZED — quotient > 3.0 (verify:execute > 3:1 — contact, no motion)
///   OPTIMAL    — quotient >= 1.0 (verification leads execution)
///   FLOWING    — quotient >= 0.5 (healthy metabolism)
///   STUCK      — quotient >= 0.1 (verification lagging)
///   BURNING    — quotient < 0.1 (execute:verify > 10:1 — motion, no witness)
///
/// Helix Codex Lock 2 (Calhoun ratio-pole, both poles are sink):
///   verify:execute > 3:1  → FOSSILIZED  (contact exists, nothing moves)
///   execute:verify > 3:1  → BURNING     (motion without witness)
///
/// Legacy variants (retained for backward compat):
///   Overheat, Balanced, Watching
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FlowVerdict {
    /// verify_count == 0 — no data to ratio against (TS: UNKNOWN)
    Unknown,
    /// verify_count < 2 — single verification is coincidence, not pattern
    Caution,
    /// quotient > 3.0 — verification far exceeds execution (Helix Codex Lock 2)
    /// Calhoun fossilisation: contact exists, nothing moves. HOLD.
    Fossilized,
    /// quotient >= 1.0 — verification leads execution (v2.2)
    Optimal,
    /// quotient >= 0.5 — healthy metabolism
    Flowing,
    /// quotient >= 0.1 — verification lagging execution
    Stuck,
    /// quotient < 0.1 — execution far outruns verification
    Burning,
    // ── Legacy variants (retained for backward compat deserialization) ──
    /// Legacy alias for Unknown (pre-v2.2)
    Unmeasured,
    /// Legacy: FQ > 10.0 — under-verification risk
    Overheat,
    /// Legacy: FQ 1.0–3.0 — healthy verification
    Balanced,
    /// Legacy: FQ 0.5–1.0 — self-monitoring competes
    Watching,
}

impl fmt::Display for FlowVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlowVerdict::Unknown => write!(f, "UNKNOWN"),
            FlowVerdict::Caution => write!(f, "CAUTION"),
            FlowVerdict::Fossilized => write!(f, "FOSSILIZED"),
            FlowVerdict::Optimal => write!(f, "OPTIMAL"),
            FlowVerdict::Flowing => write!(f, "FLOWING"),
            FlowVerdict::Stuck => write!(f, "STUCK"),
            FlowVerdict::Burning => write!(f, "BURNING"),
            FlowVerdict::Unmeasured => write!(f, "UNKNOWN"), // legacy alias
            FlowVerdict::Overheat => write!(f, "OVERHEAT"),
            FlowVerdict::Balanced => write!(f, "BALANCED"),
            FlowVerdict::Watching => write!(f, "WATCHING"),
        }
    }
}

// ── Flow Quotient ────────────────────────────────────────────────────────

/// Computed Flow Quotient over a window of receipts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowQuotient {
    /// Number of execution steps in the window
    pub execute_count: usize,
    /// Total execution cost in nanoseconds
    pub execute_cost_ns: u64,
    /// Number of verification steps in the window
    pub verify_count: usize,
    /// Total verification cost in nanoseconds (including preceding)
    pub verify_cost_ns: u64,
    /// Number of barrier/heartbeat steps in the window (not counted in FQ ratio)
    pub barrier_count: usize,
    /// Flow Quotient = verify_count / execute_count (v2.1: count-based, inverted)
    /// None when verify_count == 0 (undefined — no verification to ratio against)
    pub quotient: Option<f64>,
    /// Health verdict
    pub verdict: FlowVerdict,
    /// Window size used
    pub window_size: usize,
    /// Latest APEX block number seen in the window (B6 follow-up, 2026-08-06)
    /// Links FQ pulse to constitutional G/J evaluation cycles.
    /// None when no APEX block registered in window.
    pub apex_block: Option<u64>,
}

impl FlowQuotient {
    /// Compute FQ from a slice of receipts (v2.1 formula — 2026-08-05).
    ///
    /// v2.1 changes (Arif F13 spec):
    ///   - Quotient direction: verify_count / execute_count (was exec/verify)
    ///   - verify_count == 0 → UNKNOWN, quotient = None (was OPTIMAL, f64::MAX)
    ///   - verify_count < 2 → CAUTION, quotient = None
    ///   - Six-state band: UNKNOWN → CAUTION → OPTIMAL → FLOWING → STUCK → BURNING
    ///   - formula_hash: sha256:arifflow-fq-v2.2-2026-08-14
    ///   - formula_version: qg.v0.2
    pub fn compute(receipts: &[FlowReceipt]) -> Self {
        let mut execute_cost = 0u64;
        let mut verify_cost = 0u64;
        let mut execute_count = 0usize;
        let mut verify_count = 0usize;
        let mut barrier_count = 0usize;

        for r in receipts {
            if r.step_type.is_execution() {
                execute_cost += r.cost_ns;
                execute_count += 1;
            }
            if r.step_type.is_verification() {
                verify_cost += r.cost_ns;
                verify_count += 1;
            }
            if r.step_type.is_barrier() {
                barrier_count += 1;
            }
            if let Some(preceding) = r.preceding_verify_cost_ns {
                verify_cost = verify_cost.saturating_add(preceding);
            }
        }

        // v2.1: count-based quotient = verify / execute (inverted from v2.0)
        // None when verify_count == 0 — undefined, not 0, not ∞
        let raw_quotient = if verify_count == 0 {
            None
        } else if execute_count == 0 {
            // All verification, no execution — observe-only window
            // Return None to signal "no ratio possible" rather than ∞
            None
        } else {
            Some(verify_count as f64 / execute_count as f64)
        };

        // v2.2: seven-state band per Arif F13 spec + Helix Codex Lock 2
        // Both ratio poles are Calhoun sink:
        //   verify:execute > 3:1 → FOSSILIZED (contact, no motion)
        //   execute:verify > 3:1 → BURNING (motion, no witness)
        let verdict = if verify_count == 0 {
            FlowVerdict::Unknown
        } else if verify_count < 2 {
            FlowVerdict::Caution
        } else if execute_count == 0 {
            FlowVerdict::Flowing
        } else {
            let q = raw_quotient.unwrap_or(0.0);
            if q > 3.0 {
                // Helix Codex Lock 2: fossilisation pole
                // verify:execute > 3:1 — contact exists, nothing moves
                FlowVerdict::Fossilized
            } else if q >= 1.0 {
                FlowVerdict::Optimal
            } else if q >= 0.5 {
                FlowVerdict::Flowing
            } else if q >= 0.1 {
                FlowVerdict::Stuck
            } else {
                // Helix Codex Lock 2: burn pole
                // execute:verify > 10:1 — motion without witness
                FlowVerdict::Burning
            }
        };

        // v2.2: quotient is None for Unknown/Caution (undefined or insufficient data)
        let quotient = match verdict {
            FlowVerdict::Unknown | FlowVerdict::Unmeasured | FlowVerdict::Caution => None,
            _ => raw_quotient,
        };

        // B6: Extract the largest APEX block number seen in this window.
        // Links FQ pulse to constitutional G/J evaluation cycles.
        let apex_block = receipts.iter().filter_map(|r| r.apex_block).max();

        Self {
            execute_count,
            execute_cost_ns: execute_cost,
            verify_count,
            verify_cost_ns: verify_cost,
            barrier_count,
            quotient,
            verdict,
            window_size: receipts.len(),
            apex_block,
        }
    }
}

// ── Flow Receipt ─────────────────────────────────────────────────────────

/// The unit atom of governed flow.
///
/// Every hop, every execute, every verify, every cool — recorded in an
/// immutable, chained, Merkle-anchored receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowReceipt {
    // ── Identity ──
    /// Globally unique receipt identifier (UUID v4)
    pub receipt_id: Uuid,
    /// SHA3-256 hex hash of the previous receipt in this flow chain.
    /// `None` for the first receipt in a session.
    pub previous_receipt_hash: Option<String>,
    /// Nanosecond-precision timestamp
    pub created_at: DateTime<Utc>,

    // ── Actor ──
    /// The agent or human who performed this step
    pub actor_id: String,
    /// Governing session (from arif_init)
    pub session_id: String,
    /// SCT session token if governed by arifOS
    pub session_token: Option<String>,

    // ── Flow Step ──
    /// What kind of step was this
    pub step_type: StepType,
    /// Risk class for this receipt — maps F13 autonomy tier to FQ floor
    #[serde(default)]
    pub risk_class: RiskClass,
    /// Which topology (fan-out/pipeline/cascade)
    pub topology_id: Option<String>,
    /// Which parallel lane within a topology
    pub lane_id: Option<u32>,
    /// Monotonic step number within this session
    pub step_number: u64,

    // ── Graph Edges (2026-09-12) ──
    /// Graph engineering Step 4 (Inspectable Routing): which organ did arif_route
    /// classify this receipt to? Makes routing decisions auditable on-chain.
    /// None when routing hasn't occurred or isn't applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routed_organ: Option<String>,
    /// Graph engineering Step 3 (Edges as Data Contracts): DAG parent edges.
    /// Enables fan-out merge points where a receipt has multiple parents.
    /// `previous_receipt_hash` remains the single-chain anchor (backward compat).
    /// `parent_receipt_ids` provides multi-parent DAG support for composed topologies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parent_receipt_ids: Vec<String>,

    /// RG-PH (2026-09-13): canonical JCS SHA3-256 of THIS receipt, computed
    /// over the receipt with `jcs_body_hash` itself excluded (block-header
    /// trick). Cross-language verifiable under `arifflow-jcs-v1`. Stamped by
    /// the daemon at ingest — client-supplied values are recomputed
    /// server-side, never trusted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jcs_body_hash: Option<String>,
    /// RG-PH (2026-09-13): parallel to `parent_receipt_ids` — the canonical
    /// `jcs_body_hash` of each parent AT EDGE-CREATION TIME. Binds content,
    /// not just identity: a tampered or replaced parent diverges from the
    /// recorded hash and the causal edge breaks VISIBLY. Must be 1:1 with
    /// `parent_receipt_ids` when present.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parent_receipt_hashes: Vec<String>,

    // ── Genesis Bridge (RG-3, 2026-09-12) ──
    /// Constitutional anchor reference. When set, this receipt bridges the
    /// Historical Lineage (receipt DAG) to the Constitutional Origin (RCP-000,
    /// /000, F13 canon). The Genesis Receipt is the root of the Reality Graph
    /// — it has `parent_receipt_ids = []` and `genesis_anchor = Some(...)`.
    ///
    /// Without this field: "what happened?"
    /// With this field: "what happened AND why it has authority to exist."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genesis_anchor: Option<String>,

    // ── Cost ──
    /// Wall-clock duration of this step in nanoseconds
    pub cost_ns: u64,
    /// Total verification cost that led to this step
    pub preceding_verify_cost_ns: Option<u64>,

    // ── Epistemic ──
    /// Truth status of this step's output (F2/F7)
    pub epistemic_label: EpistemicLabel,

    // ── Governance ──
    /// F1–F13 constitutional verdict for this step
    pub floor_verdict: FloorVerdict,
    /// T2-1 (audit 2026-08-10): WHY bridge — first-class intent field.
    /// "Kenapa agent buat benda ni?" — governance without reading source code.
    pub intent_reason: Option<String>,
    /// T2-1: Expected outcome of this action — what the agent expected to happen.
    /// Completes the WHY bridge: intent (why) + expected_outcome (what should happen).
    pub expected_outcome: Option<String>,
    /// Cooling queue action
    pub cooling_decision: CoolingDecision,

    // ── Witness ──
    /// Aggregated witness scores for F3
    pub tri_witness_votes: Option<TriWitnessVotes>,

    // ── Merkle ──
    /// Root hash of Merkle tree this receipt belongs to
    pub merkle_root: Option<String>,
    /// Inclusion proof path (hex-encoded)
    pub merkle_inclusion_proof: Option<String>,

    // ── Payload ──
    /// Flexible JSON payload — step-specific data, errors, intermediates
    pub payload: Option<serde_json::Value>,

    // ── Provenance (Gate 1 Instrument — 2026-08-04) ──
    /// FQ formula version used when this receipt was recorded
    pub formula_version: Option<String>,
    /// SHA3-256 of the formula source
    pub formula_hash: Option<String>,
    /// Organs that witnessed this step
    pub witness_organs: Option<Vec<String>>,
    /// APEX block number — links this receipt to the constitutional G/J evaluation
    /// cycle it belongs to (B6 follow-up, 2026-08-06). None when outside any APEX block.
    pub apex_block: Option<u64>,
    /// QG.v0.3 (2026-08-14): FLOW block number — the metabolic evaluation cycle
    /// this receipt belongs to (vector spec §9 STEP 1). None when unwired.
    pub flow_block: Option<u64>,
    /// QG.v0.3 (2026-08-14): PROJECTION block number — the forward-model/projection
    /// cycle this receipt belongs to. None when unwired.
    pub projection_block: Option<u64>,
}

impl FlowReceipt {
    /// Bind DAG parents with cryptographic causal edges (RG-PH, 2026-09-13).
    /// `hashes` must be the parents' `jcs_body_hash` values, 1:1 with `ids`.
    pub fn with_causal_parents(mut self, ids: Vec<String>, hashes: Vec<String>) -> Self {
        self.parent_receipt_ids = ids;
        self.parent_receipt_hashes = hashes;
        self
    }

    /// RG-PH: canonical JCS SHA3-256 of this receipt, computed over the
    /// receipt with `jcs_body_hash` excluded (self-reference is impossible;
    /// the exclusion is the contract). Cross-language reproducible under
    /// `arifflow-jcs-v1` — see spec/RG_PREV_HASH_SCHEMA_v1.md.
    pub fn compute_jcs_body_hash(&self) -> Result<String, crate::jcs::JcsError> {
        let mut value =
            serde_json::to_value(self).map_err(|_| crate::jcs::JcsError::UnsupportedValue)?;
        if let Some(obj) = value.as_object_mut() {
            obj.remove("jcs_body_hash");
        }
        crate::jcs::jcs_sha3_hex(&value)
    }

    /// Create a new receipt as the **first** in a flow chain (no previous).
    pub fn new_first(
        actor_id: impl Into<String>,
        session_id: impl Into<String>,
        step_type: StepType,
        epistemic_label: EpistemicLabel,
        cost_ns: u64,
    ) -> Self {
        Self {
            receipt_id: Uuid::new_v4(),
            previous_receipt_hash: None,
            created_at: Utc::now(),
            actor_id: actor_id.into(),
            session_id: session_id.into(),
            session_token: None,
            step_type,
            risk_class: RiskClass::default(),
            topology_id: None,
            lane_id: None,
            step_number: 0,
            routed_organ: None,
            parent_receipt_ids: Vec::new(),
            jcs_body_hash: None,
            parent_receipt_hashes: Vec::new(),
            genesis_anchor: None,
            cost_ns,
            preceding_verify_cost_ns: None,
            epistemic_label,
            floor_verdict: FloorVerdict::Pass,
            intent_reason: None,
            expected_outcome: None,
            cooling_decision: CoolingDecision::None,
            tri_witness_votes: None,
            merkle_root: None,
            merkle_inclusion_proof: None,
            payload: None,
            formula_version: Some("qg.v0.2".into()),
            formula_hash: Some("sha256:placeholder".into()),
            witness_organs: None,
            apex_block: None,
            flow_block: None,
            projection_block: None,
        }
    }

    /// Create a new receipt chained to a previous receipt.
    pub fn new_chained(
        previous: &FlowReceipt,
        actor_id: impl Into<String>,
        session_id: impl Into<String>,
        step_type: StepType,
        epistemic_label: EpistemicLabel,
        cost_ns: u64,
    ) -> Self {
        let prev_hash = previous.hash();
        Self {
            receipt_id: Uuid::new_v4(),
            previous_receipt_hash: Some(prev_hash),
            created_at: Utc::now(),
            actor_id: actor_id.into(),
            session_id: session_id.into(),
            session_token: None,
            step_type,
            risk_class: RiskClass::default(),
            topology_id: None,
            lane_id: None,
            step_number: previous.step_number + 1,
            routed_organ: None,
            parent_receipt_ids: Vec::new(),
            jcs_body_hash: None,
            parent_receipt_hashes: Vec::new(),
            genesis_anchor: None,
            cost_ns,
            preceding_verify_cost_ns: None,
            epistemic_label,
            floor_verdict: FloorVerdict::Pass,
            intent_reason: None,
            expected_outcome: None,
            cooling_decision: CoolingDecision::None,
            tri_witness_votes: None,
            merkle_root: None,
            merkle_inclusion_proof: None,
            payload: None,
            formula_version: Some("qg.v0.2".into()),
            formula_hash: Some("sha256:placeholder".into()),
            witness_organs: None,
            apex_block: None,
            flow_block: None,
            projection_block: None,
        }
    }

    /// Compute the SHA3-256 hash of this receipt's canonical JSON.
    pub fn hash(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha3_256::new();
        hasher.update(json.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Set the step type builder-style.
    pub fn with_step_type(mut self, step_type: StepType) -> Self {
        self.step_type = step_type;
        self
    }

    /// Set the risk class builder-style.
    pub fn with_risk_class(mut self, risk_class: RiskClass) -> Self {
        self.risk_class = risk_class;
        self
    }

    /// Set the epistemic label builder-style.
    pub fn with_epistemic(mut self, label: EpistemicLabel) -> Self {
        self.epistemic_label = label;
        self
    }

    /// Set the floor verdict builder-style.
    pub fn with_floor_verdict(mut self, verdict: FloorVerdict) -> Self {
        self.floor_verdict = verdict;
        self
    }

    /// Set the cooling decision builder-style.
    pub fn with_cooling(mut self, decision: CoolingDecision) -> Self {
        self.cooling_decision = decision;
        self
    }

    /// Set witness votes builder-style.
    pub fn with_witness(mut self, votes: TriWitnessVotes) -> Self {
        self.tri_witness_votes = Some(votes);
        self
    }

    /// Set preceding verification cost builder-style.
    pub fn with_preceding_verify_cost(mut self, cost_ns: u64) -> Self {
        self.preceding_verify_cost_ns = Some(cost_ns);
        self
    }

    /// Set topology context builder-style.
    pub fn with_topology(mut self, topology_id: impl Into<String>, lane_id: u32) -> Self {
        self.topology_id = Some(topology_id.into());
        self.lane_id = Some(lane_id);
        self
    }

    /// Set the payload builder-style.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Set the session token builder-style.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.session_token = Some(token.into());
        self
    }

    /// Set the routed organ builder-style (Inspectable Routing — Pattern 6).
    pub fn with_routed_organ(mut self, organ: impl Into<String>) -> Self {
        self.routed_organ = Some(organ.into());
        self
    }

    /// Add a DAG parent edge (Edges as Data Contracts — Pattern 3).
    /// Use for fan-out merge points where a receipt has multiple parents.
    pub fn with_parent(mut self, receipt_hash: impl Into<String>) -> Self {
        self.parent_receipt_ids.push(receipt_hash.into());
        self
    }

    /// Set all DAG parent edges at once.
    pub fn with_parents(mut self, hashes: Vec<String>) -> Self {
        self.parent_receipt_ids = hashes;
        self
    }

    /// Set the genesis anchor — bridges this receipt to a constitutional origin.
    /// Use on the first receipt in a session/graph to establish the Genesis Bridge:
    /// constitutional ancestry ↔ historical ancestry.
    ///
    /// Example: `.with_genesis_anchor("RCP-000")` or `.with_genesis_anchor("/000")`
    pub fn with_genesis_anchor(mut self, anchor: impl Into<String>) -> Self {
        self.genesis_anchor = Some(anchor.into());
        self
    }
}

// ── Chain Verification ───────────────────────────────────────────────────

/// Verify the integrity of a receipt chain.
///
/// Every receipt must have `previous_receipt_hash` matching the SHA3-256
/// of the previous receipt. The first receipt must have `None`.
pub fn verify_chain(receipts: &[FlowReceipt]) -> Result<(), String> {
    if receipts.is_empty() {
        return Err("Empty receipt chain".to_string());
    }

    // First receipt must have no previous hash
    if receipts[0].previous_receipt_hash.is_some() {
        return Err(format!(
            "First receipt must have no previous hash, but got: {}",
            receipts[0].previous_receipt_hash.as_ref().unwrap()
        ));
    }

    for i in 1..receipts.len() {
        let expected_hash = receipts[i - 1].hash();
        let actual_hash = receipts[i]
            .previous_receipt_hash
            .as_ref()
            .ok_or_else(|| {
                format!(
                    "Receipt {} (step {}) has no previous hash, but is not the first receipt (index {})",
                    receipts[i].receipt_id, receipts[i].step_number, i
                )
            })?;

        if *actual_hash != expected_hash {
            return Err(format!(
                "Chain break at receipt {} (step {}): expected hash {}, got {}",
                receipts[i].receipt_id, receipts[i].step_number, expected_hash, actual_hash
            ));
        }
    }

    Ok(())
}

// ── Receipt Store ────────────────────────────────────────────────────────

/// A simple in-memory store for flow receipts in a session.
///
/// Maintains chain order and provides FQ computation.
#[derive(Debug, Clone)]
pub struct ReceiptStore {
    receipts: Vec<FlowReceipt>,
    max_receipts: usize,
}

impl ReceiptStore {
    /// Create a new receipt store with a maximum capacity.
    pub fn new(max_receipts: usize) -> Self {
        Self {
            receipts: Vec::with_capacity(max_receipts.min(1000)),
            max_receipts,
        }
    }

    /// Push a receipt bypassing chain validation — for monitoring/observability.
    pub fn push_force(&mut self, receipt: FlowReceipt) {
        if self.receipts.len() >= self.max_receipts {
            self.receipts.remove(0);
        }
        self.receipts.push(receipt);
    }

    /// Push a receipt with chain-aware validation (multi-session safe).
    ///
    /// [OBS] 2026-08-10 — replaces push_force in daemon ingest to catch malformed hash chains
    /// without requiring all clients to track chains. Acceptance rules:
    ///
    /// 1. If receipt has `previous_receipt_hash`, search store for a receipt whose hash
    ///    equals that value. Found → accept (chain valid). Not found → reject.
    /// 2. If receipt has no `previous_receipt_hash` → accept (new chain start, multi-session compatible).
    pub fn push_chain_aware(&mut self, receipt: FlowReceipt) -> Result<(), String> {
        if let Some(ref prev_hash) = receipt.previous_receipt_hash {
            // Client claims this is chained — verify the predecessor exists in our store.
            let found = self.receipts.iter().any(|r| r.hash() == *prev_hash);
            if !found {
                return Err(format!(
                    "Chain-aware reject: previous_receipt_hash {} not found in store ({} receipts). \
                     Accepting only: (a) receipts with no previous hash (new chain), or \
                     (b) receipts whose previous hash matches a stored receipt.",
                    prev_hash,
                    self.receipts.len()
                ));
            }
        }
        // RG-PH (2026-09-13): cryptographic causal-edge verification. When the
        // client binds parent content hashes, each claim is checked against
        // the stored parent's canonical jcs_body_hash. A mismatch means the
        // edge points at different content than it was created against —
        // reject: a false "because" is worse than no edge.
        if !receipt.parent_receipt_hashes.is_empty() {
            if receipt.parent_receipt_hashes.len() != receipt.parent_receipt_ids.len() {
                return Err(format!(
                    "Causal-edge reject: parent_receipt_hashes has {} entries but \
                     parent_receipt_ids has {} — must be 1:1",
                    receipt.parent_receipt_hashes.len(),
                    receipt.parent_receipt_ids.len()
                ));
            }
            for (pid, claimed) in receipt
                .parent_receipt_ids
                .iter()
                .zip(receipt.parent_receipt_hashes.iter())
            {
                let parent = self
                    .receipts
                    .iter()
                    .find(|r| r.receipt_id.to_string() == *pid);
                let Some(parent) = parent else {
                    // Parent outside the in-memory window — cannot verify.
                    // Accept with a logged gap rather than reject: the edge
                    // stays queryable; verification simply didn't happen.
                    eprintln!(
                        "[arifFlow] WARN: causal-edge parent {} not in store window — \
                         hash claim unverifiable, accepted unverified",
                        pid
                    );
                    continue;
                };
                let actual = parent
                    .jcs_body_hash
                    .clone()
                    .or_else(|| parent.compute_jcs_body_hash().ok());
                match actual {
                    Some(ref h) if h == claimed => {}
                    Some(ref h) => {
                        return Err(format!(
                            "Causal-edge reject: parent {} content hash mismatch — \
                             claimed {}, actual {}. The edge binds content; refusing \
                             to record a false 'because'.",
                            pid, claimed, h
                        ));
                    }
                    None => {
                        eprintln!(
                            "[arifFlow] WARN: causal-edge parent {} unhashable \
                             (schema-discipline violation in parent) — accepted unverified",
                            pid
                        );
                    }
                }
            }
        }
        // Either no hash (new chain) or hash matched → accept.
        if self.receipts.len() >= self.max_receipts {
            self.receipts.remove(0);
        }
        self.receipts.push(receipt);
        Ok(())
    }

    pub fn push(&mut self, receipt: FlowReceipt) -> Result<(), String> {
        // Validate chain continuity
        if let Some(last) = self.receipts.last() {
            let expected_hash = last.hash();
            match &receipt.previous_receipt_hash {
                Some(h) if h == &expected_hash => { /* OK */ }
                Some(h) => {
                    return Err(format!(
                        "Chain continuity violation: expected prev hash {}, got {}",
                        expected_hash, h
                    ));
                }
                None => {
                    return Err(
                        "Chain continuity violation: chained receipt has no previous hash"
                            .to_string(),
                    );
                }
            }
        } else if receipt.previous_receipt_hash.is_some() {
            return Err("First receipt in store must have no previous hash".to_string());
        }

        // Enforce capacity
        if self.receipts.len() >= self.max_receipts {
            self.receipts.remove(0); // drop oldest
        }

        self.receipts.push(receipt);
        Ok(())
    }

    /// Get all receipts in the store.
    pub fn all(&self) -> &[FlowReceipt] {
        &self.receipts
    }

    /// Get the last N receipts (sliding window).
    pub fn last_n(&self, n: usize) -> &[FlowReceipt] {
        let len = self.receipts.len();
        let start = len.saturating_sub(n);
        &self.receipts[start..]
    }

    /// Compute the Flow Quotient over the last N receipts.
    pub fn flow_quotient(&self, window: usize) -> FlowQuotient {
        let window_receipts = self.last_n(window);
        FlowQuotient::compute(window_receipts)
    }

    /// Get the number of receipts stored.
    pub fn len(&self) -> usize {
        self.receipts.len()
    }

    /// Returns true if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.receipts.is_empty()
    }

    /// Verify the entire stored chain.
    pub fn verify_chain(&self) -> Result<(), String> {
        verify_chain(&self.receipts)
    }
}

impl Default for ReceiptStore {
    fn default() -> Self {
        Self::new(1000)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_first_receipt() {
        let r = FlowReceipt::new_first(
            "a-forge",
            "session-123",
            StepType::Execute,
            EpistemicLabel::Observation,
            1_000_000,
        );
        assert_eq!(r.actor_id, "a-forge");
        assert_eq!(r.session_id, "session-123");
        assert_eq!(r.step_type, StepType::Execute);
        assert_eq!(r.epistemic_label, EpistemicLabel::Observation);
        assert!(r.previous_receipt_hash.is_none());
        assert_eq!(r.step_number, 0);
    }

    #[test]
    fn test_create_chained_receipt() {
        let r1 = FlowReceipt::new_first(
            "a-forge",
            "session-123",
            StepType::Execute,
            EpistemicLabel::Observation,
            1_000_000,
        );
        let r2 = FlowReceipt::new_chained(
            &r1,
            "a-forge",
            "session-123",
            StepType::Verify,
            EpistemicLabel::Derivation,
            500_000,
        );

        assert!(r2.previous_receipt_hash.is_some());
        assert_eq!(r2.step_number, 1);
        assert_eq!(r2.step_type, StepType::Verify);

        // Hash matches
        let expected_hash = r1.hash();
        assert_eq!(r2.previous_receipt_hash.unwrap(), expected_hash);
    }

    #[test]
    fn test_receipt_hash_deterministic() {
        let r1 = FlowReceipt::new_first(
            "a-forge",
            "session-123",
            StepType::Execute,
            EpistemicLabel::Observation,
            1_000_000,
        );
        let hash1 = r1.hash();
        let hash2 = r1.hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_verify_chain_valid() {
        let mut receipts = Vec::new();
        let r1 = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );
        receipts.push(r1.clone());

        let r2 = FlowReceipt::new_chained(
            &r1,
            "agent",
            "s1",
            StepType::Verify,
            EpistemicLabel::Derivation,
            50,
        );
        receipts.push(r2.clone());

        let r3 = FlowReceipt::new_chained(
            &r2,
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            200,
        );
        receipts.push(r3);

        assert!(verify_chain(&receipts).is_ok());
    }

    #[test]
    fn test_verify_chain_break() {
        let mut receipts = Vec::new();
        let r1 = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );
        receipts.push(r1);

        // Manually create a receipt with wrong hash
        let broken = FlowReceipt {
            previous_receipt_hash: Some("deadbeef".to_string()),
            ..FlowReceipt::new_chained(
                &receipts[0],
                "agent",
                "s1",
                StepType::Execute,
                EpistemicLabel::Observation,
                100,
            )
        };
        receipts.push(broken);

        assert!(verify_chain(&receipts).is_err());
    }

    #[test]
    fn test_verify_chain_empty() {
        assert!(verify_chain(&[]).is_err());
    }

    #[test]
    fn test_tri_witness_votes() {
        let votes = TriWitnessVotes::new(0.9, 0.8, 0.95).unwrap();
        assert!((votes.nash_score() - 0.684).abs() < 0.001);
        assert!(!votes.meets_f3_threshold()); // 0.684 < 0.75
    }

    #[test]
    fn test_tri_witness_f3_pass() {
        let votes = TriWitnessVotes::new(1.0, 0.9, 0.95).unwrap();
        assert!(votes.meets_f3_threshold()); // 0.855 >= 0.75
    }

    #[test]
    fn test_tri_witness_invalid_range() {
        assert!(TriWitnessVotes::new(1.5, 0.5, 0.5).is_err());
        assert!(TriWitnessVotes::new(0.5, -0.1, 0.5).is_err());
    }

    // ── Flow Quotient v2.1 Tests ────────────────────────────────────────────

    #[test]
    fn test_fq_v21_optimal() {
        // 5 exec + 5 verify → quotient = 5/5 = 1.0 → OPTIMAL
        let mut store = ReceiptStore::new(100);
        for _ in 0..5 {
            store.push_force(FlowReceipt::new_first(
                "agent",
                "s1",
                StepType::Execute,
                EpistemicLabel::Observation,
                1_000_000,
            ));
        }
        for _ in 0..5 {
            store.push_force(FlowReceipt::new_first(
                "agent",
                "s1",
                StepType::Verify,
                EpistemicLabel::Derivation,
                500_000,
            ));
        }
        let fq = store.flow_quotient(20);
        assert_eq!(fq.verdict, FlowVerdict::Optimal);
        assert_eq!(fq.quotient, Some(1.0));
    }

    #[test]
    fn test_fq_v21_flowing() {
        // 4 exec + 3 verify → quotient = 3/4 = 0.75 → FLOWING
        let mut store = ReceiptStore::new(100);
        for _ in 0..4 {
            store.push_force(FlowReceipt::new_first(
                "agent",
                "s1",
                StepType::Execute,
                EpistemicLabel::Observation,
                1_000_000,
            ));
        }
        for _ in 0..3 {
            store.push_force(FlowReceipt::new_first(
                "agent",
                "s1",
                StepType::Verify,
                EpistemicLabel::Derivation,
                500_000,
            ));
        }
        let fq = store.flow_quotient(20);
        assert_eq!(fq.verdict, FlowVerdict::Flowing);
        assert_eq!(fq.quotient, Some(0.75));
    }

    #[test]
    fn test_fq_v21_stuck() {
        // 5 exec + 2 verify → quotient = 2/5 = 0.4 → STUCK
        let mut store = ReceiptStore::new(100);
        for _ in 0..5 {
            store.push_force(FlowReceipt::new_first(
                "agent",
                "s1",
                StepType::Execute,
                EpistemicLabel::Observation,
                1_000_000,
            ));
        }
        for _ in 0..2 {
            store.push_force(FlowReceipt::new_first(
                "agent",
                "s1",
                StepType::Verify,
                EpistemicLabel::Derivation,
                500_000,
            ));
        }
        let fq = store.flow_quotient(20);
        assert_eq!(fq.verdict, FlowVerdict::Stuck);
        assert_eq!(fq.quotient, Some(0.4));
    }

    #[test]
    fn test_fq_v21_burning() {
        // 21 exec + 2 verify → quotient = 2/21 ≈ 0.095 → BURNING
        let mut store = ReceiptStore::new(100);
        for _ in 0..21 {
            store.push_force(FlowReceipt::new_first(
                "agent",
                "s1",
                StepType::Execute,
                EpistemicLabel::Observation,
                1_000_000,
            ));
        }
        for _ in 0..2 {
            store.push_force(FlowReceipt::new_first(
                "agent",
                "s1",
                StepType::Verify,
                EpistemicLabel::Derivation,
                500_000,
            ));
        }
        let fq = store.flow_quotient(50);
        assert_eq!(fq.verdict, FlowVerdict::Burning);
        assert!(fq.quotient.unwrap() < 0.1);
    }

    #[test]
    fn test_fq_v21_unknown_no_verify() {
        // 3 exec + 0 verify → UNKNOWN (undefined quotient)
        let mut store = ReceiptStore::new(100);
        for _ in 0..3 {
            store.push_force(FlowReceipt::new_first(
                "agent",
                "s1",
                StepType::Execute,
                EpistemicLabel::Observation,
                1_000_000,
            ));
        }
        let fq = store.flow_quotient(20);
        assert_eq!(fq.verdict, FlowVerdict::Unknown);
        assert_eq!(fq.quotient, None);
    }

    #[test]
    fn test_fq_v21_caution_single_verify() {
        // 3 exec + 1 verify → CAUTION (verify < 2)
        let mut store = ReceiptStore::new(100);
        for _ in 0..3 {
            store.push_force(FlowReceipt::new_first(
                "agent",
                "s1",
                StepType::Execute,
                EpistemicLabel::Observation,
                1_000_000,
            ));
        }
        store.push_force(FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Verify,
            EpistemicLabel::Derivation,
            500_000,
        ));
        let fq = store.flow_quotient(20);
        assert_eq!(fq.verdict, FlowVerdict::Caution);
        assert_eq!(fq.quotient, None);
    }

    #[test]
    fn test_fq_v21_empty() {
        let store = ReceiptStore::new(100);
        let fq = store.flow_quotient(20);
        assert_eq!(fq.verdict, FlowVerdict::Unknown);
        assert_eq!(fq.quotient, None);
    }

    #[test]
    fn test_receipt_store_push_validates_chain() {
        let mut store = ReceiptStore::new(100);

        let r1 = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );
        assert!(store.push(r1).is_ok());

        let r2 = FlowReceipt::new_chained(
            store.all().last().unwrap(),
            "agent",
            "s1",
            StepType::Verify,
            EpistemicLabel::Derivation,
            50,
        );
        assert!(store.push(r2).is_ok());

        // Push a broken receipt
        let broken = FlowReceipt {
            previous_receipt_hash: Some("badhash".to_string()),
            ..FlowReceipt::new_first(
                "agent",
                "s1",
                StepType::Execute,
                EpistemicLabel::Observation,
                100,
            )
        };
        assert!(store.push(broken).is_err());
    }

    #[test]
    fn test_receipt_store_default_capacity() {
        let store = ReceiptStore::default();
        assert_eq!(store.max_receipts, 1000);
    }

    #[test]
    fn test_receipt_store_enforces_max() {
        let mut store = ReceiptStore::new(3);
        let r1 = FlowReceipt::new_first(
            "a",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            10,
        );
        store.push(r1).unwrap();
        let r2 = FlowReceipt::new_chained(
            store.all().last().unwrap(),
            "a",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            10,
        );
        store.push(r2).unwrap();
        let r3 = FlowReceipt::new_chained(
            store.all().last().unwrap(),
            "a",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            10,
        );
        store.push(r3).unwrap();

        assert_eq!(store.len(), 3);

        // Push 4th — oldest should drop
        let r4 = FlowReceipt::new_chained(
            store.all().last().unwrap(),
            "a",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            10,
        );
        store.push(r4).unwrap();
        assert_eq!(store.len(), 3);
    }

    // ── push_chain_aware tests (Fix 2 — 2026-08-10) ──────────────────────

    // ── RG-PH tests (2026-09-13) — cryptographically-chained causal edges ──

    fn rgph_pair() -> (FlowReceipt, FlowReceipt) {
        let parent = FlowReceipt::new_first(
            "rgph-test",
            "s-rgph",
            StepType::Execute,
            EpistemicLabel::Observation,
            10,
        );
        let child = FlowReceipt::new_first(
            "rgph-test",
            "s-rgph",
            StepType::Verify,
            EpistemicLabel::Observation,
            5,
        );
        (parent, child)
    }

    #[test]
    fn rgph_stamp_roundtrip_is_stable() {
        // Exclusion rule: setting jcs_body_hash must not change its own
        // recomputation (self-reference impossible, exclusion is the contract).
        let (parent, _) = rgph_pair();
        let h1 = parent.compute_jcs_body_hash().unwrap();
        let mut stamped = parent.clone();
        stamped.jcs_body_hash = Some(h1.clone());
        let h2 = stamped.compute_jcs_body_hash().unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
        assert!(h1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn rgph_causal_edge_happy_path() {
        let (mut parent, child) = rgph_pair();
        let pid = parent.receipt_id.to_string();
        let ph = parent.compute_jcs_body_hash().unwrap();
        parent.jcs_body_hash = Some(ph.clone());

        let mut store = ReceiptStore::new(100);
        store.push_chain_aware(parent.clone()).unwrap();

        let bound = child.with_causal_parents(vec![pid], vec![ph.clone()]);
        assert!(store.push_chain_aware(bound.clone()).is_ok());
        let stored = store.receipts.last().unwrap();
        assert_eq!(stored.parent_receipt_hashes, vec![ph]);
    }

    #[test]
    fn rgph_causal_edge_tamper_rejected() {
        let (mut parent, child) = rgph_pair();
        let pid = parent.receipt_id.to_string();
        parent.jcs_body_hash = Some(parent.compute_jcs_body_hash().unwrap());

        let mut store = ReceiptStore::new(100);
        store.push_chain_aware(parent).unwrap();

        let forged = child.with_causal_parents(
            vec![pid],
            vec!["deadbeef".repeat(8)], // claims content that isn't the parent's
        );
        let err = store.push_chain_aware(forged).unwrap_err();
        assert!(err.contains("refusing to record a false"), "{err}");
        assert_eq!(store.len(), 1, "forged child must not be stored");
    }

    #[test]
    fn rgph_causal_edge_length_mismatch_rejected() {
        let (parent, child) = rgph_pair();
        let pid = parent.receipt_id.to_string();
        let mut store = ReceiptStore::new(100);
        store.push_chain_aware(parent).unwrap();
        let bad = child.with_causal_parents(vec![pid, "other".into()], vec!["x".into()]);
        let err = store.push_chain_aware(bad).unwrap_err();
        assert!(err.contains("1:1"), "{err}");
    }

    #[test]
    fn rgph_legacy_ids_only_still_accepted() {
        // Compat path: RG-1.5 emitters send ids without hashes.
        let (parent, child) = rgph_pair();
        let pid = parent.receipt_id.to_string();
        let mut store = ReceiptStore::new(100);
        store.push_chain_aware(parent).unwrap();
        let legacy = child.with_causal_parents(vec![pid], Vec::new());
        assert!(store.push_chain_aware(legacy).is_ok());
    }

    #[test]
    fn rgph_client_stamp_is_recomputed_not_trusted() {
        // Same receipt content, two different lying client stamps → the
        // canonical hash depends only on content, never on the field itself.
        let (parent, _) = rgph_pair();
        let mut liar = parent.clone();
        liar.jcs_body_hash = Some("0".repeat(64));
        assert_eq!(
            liar.compute_jcs_body_hash().unwrap(),
            parent.compute_jcs_body_hash().unwrap()
        );
    }

    #[test]
    fn test_push_chain_aware_accepts_no_previous_hash() {
        // [OBS] Receipt with no previous_receipt_hash → accepted (new chain start, multi-session safe)
        let mut store = ReceiptStore::new(100);
        let r = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );
        assert!(store.push_chain_aware(r).is_ok());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_push_chain_aware_accepts_valid_previous_hash() {
        // [OBS] Receipt with valid previous_receipt_hash pointing to existing receipt → accepted
        let mut store = ReceiptStore::new(100);
        let r1 = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );
        store.push_chain_aware(r1.clone()).unwrap();

        let r2 = FlowReceipt::new_chained(
            &r1,
            "agent",
            "s1",
            StepType::Verify,
            EpistemicLabel::Derivation,
            50,
        );
        assert!(store.push_chain_aware(r2).is_ok());
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn test_push_chain_aware_rejects_invalid_previous_hash() {
        // [OBS] Receipt with previous_receipt_hash not matching any stored receipt → rejected
        let mut store = ReceiptStore::new(100);
        // Pre-populate with a receipt so the store isn't empty
        let r1 = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );
        store.push_chain_aware(r1).unwrap();

        // Create a receipt with a bogus hash
        let broken = FlowReceipt {
            previous_receipt_hash: Some("deadbeef_not_a_real_hash".to_string()),
            ..FlowReceipt::new_first(
                "agent",
                "s1",
                StepType::Execute,
                EpistemicLabel::Observation,
                100,
            )
        };
        assert!(
            store.push_chain_aware(broken).is_err(),
            "Invalid hash must be rejected"
        );
        assert_eq!(store.len(), 1, "Store must not grow on reject");
    }

    #[test]
    fn test_push_chain_aware_multi_session_interleaving() {
        // [OBS] Multiple sessions can coexist — each starts with no previous hash.
        let mut store = ReceiptStore::new(100);

        // Session A: 2 receipts
        let r_a1 = FlowReceipt::new_first(
            "agent-a",
            "session-a",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );
        store.push_chain_aware(r_a1.clone()).unwrap();
        let r_a2 = FlowReceipt::new_chained(
            &r_a1,
            "agent-a",
            "session-a",
            StepType::Verify,
            EpistemicLabel::Derivation,
            50,
        );
        store.push_chain_aware(r_a2).unwrap();

        // Session B: starts fresh (no previous hash)
        let r_b1 = FlowReceipt::new_first(
            "agent-b",
            "session-b",
            StepType::Execute,
            EpistemicLabel::Observation,
            200,
        );
        assert!(
            store.push_chain_aware(r_b1).is_ok(),
            "New session must be accepted"
        );
        assert_eq!(store.len(), 3);
    }

    #[test]
    fn test_builder_pattern() {
        let receipt = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        )
        .with_epistemic(EpistemicLabel::Interpretation)
        .with_floor_verdict(FloorVerdict::Caution)
        .with_cooling(CoolingDecision::Clamp)
        .with_witness(TriWitnessVotes::new(0.9, 0.85, 0.95).unwrap())
        .with_preceding_verify_cost(50_000)
        .with_topology("fan-out:build", 3)
        .with_payload(serde_json::json!({"action": "deploy", "target": "production"}));

        assert_eq!(receipt.epistemic_label, EpistemicLabel::Interpretation);
        assert_eq!(receipt.floor_verdict, FloorVerdict::Caution);
        assert_eq!(receipt.cooling_decision, CoolingDecision::Clamp);
        assert!(receipt.tri_witness_votes.is_some());
        assert!(receipt.preceding_verify_cost_ns.is_some());
        assert_eq!(receipt.topology_id.unwrap(), "fan-out:build");
        assert_eq!(receipt.lane_id.unwrap(), 3);
    }

    #[test]
    fn test_flow_quotient_no_verification() {
        let mut store = ReceiptStore::new(10);
        let r1 = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            1_000_000,
        );
        store.push(r1).unwrap();
        let r2 = FlowReceipt::new_chained(
            store.all().last().unwrap(),
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            2_000_000,
        );
        store.push(r2).unwrap();

        let fq = store.flow_quotient(10);
        assert_eq!(fq.verdict, FlowVerdict::Unknown);
        assert_eq!(fq.quotient, None);
    }

    #[test]
    fn test_step_type_display() {
        assert_eq!(StepType::Execute.to_string(), "Execute");
        assert_eq!(StepType::Verify.to_string(), "Verify");
        assert_eq!(StepType::Cool.to_string(), "Cool");
        assert_eq!(StepType::Seal.to_string(), "Seal");
        assert_eq!(StepType::Barrier.to_string(), "Barrier");
        assert_eq!(StepType::Merge.to_string(), "Merge");
        assert_eq!(StepType::Route.to_string(), "Route");
    }

    #[test]
    fn test_epistemic_label_display() {
        assert_eq!(EpistemicLabel::Observation.to_string(), "OBS");
        assert_eq!(EpistemicLabel::Derivation.to_string(), "DER");
        assert_eq!(EpistemicLabel::Interpretation.to_string(), "INT");
        assert_eq!(EpistemicLabel::Specification.to_string(), "SPEC");
        assert_eq!(EpistemicLabel::Seal.to_string(), "SEAL");
    }

    #[test]
    fn test_step_type_classification() {
        assert!(StepType::Execute.is_execution());
        assert!(StepType::Seal.is_execution());
        assert!(StepType::Merge.is_execution());
        assert!(!StepType::Execute.is_verification());
        assert!(StepType::Verify.is_verification());
        assert!(!StepType::Cool.is_execution());
    }

    #[test]
    fn test_store_verify_chain() {
        let mut store = ReceiptStore::new(100);
        let r1 = FlowReceipt::new_first(
            "a",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            10,
        );
        store.push(r1).unwrap();
        let r2 = FlowReceipt::new_chained(
            store.all().last().unwrap(),
            "a",
            "s1",
            StepType::Verify,
            EpistemicLabel::Derivation,
            5,
        );
        store.push(r2).unwrap();
        assert!(store.verify_chain().is_ok());
    }

    // ── Graph Edge tests (2026-09-12) ────────────────────────────────────────

    #[test]
    fn test_routed_organ_builder() {
        let receipt = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Route,
            EpistemicLabel::Specification,
            100,
        )
        .with_routed_organ("geox");

        assert_eq!(receipt.routed_organ.as_deref(), Some("geox"));
    }

    #[test]
    fn test_routed_organ_none_by_default() {
        let receipt = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );
        assert!(receipt.routed_organ.is_none());
    }

    #[test]
    fn test_routed_organ_serializes_optional() {
        let receipt = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );
        let json = serde_json::to_string(&receipt).unwrap();
        // When None, skip_serializing_if prevents it from appearing
        assert!(!json.contains("routed_organ"));

        let routed = receipt.with_routed_organ("wealth");
        let json2 = serde_json::to_string(&routed).unwrap();
        assert!(json2.contains("routed_organ"));
        assert!(json2.contains("wealth"));
    }

    #[test]
    fn test_routed_organ_backward_compat_deserialize() {
        // Old receipts without routed_organ must deserialize cleanly
        let json = r#"{
            "receipt_id": "00000000-0000-0000-0000-000000000001",
            "previous_receipt_hash": null,
            "created_at": "2026-09-12T00:00:00Z",
            "actor_id": "agent",
            "session_id": "s1",
            "step_type": "Execute",
            "cost_ns": 100,
            "epistemic_label": "Observation",
            "floor_verdict": "Pass",
            "cooling_decision": "None",
            "step_number": 0,
            "risk_class": "T0Observe"
        }"#;
        let receipt: FlowReceipt = serde_json::from_str(json).unwrap();
        assert!(receipt.routed_organ.is_none());
        assert!(receipt.parent_receipt_ids.is_empty());
    }

    #[test]
    fn test_parent_receipt_ids_builder() {
        let r1 = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );
        let r2 = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            200,
        );

        // Merge point: receipt with two parents
        let merge = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Merge,
            EpistemicLabel::Derivation,
            50,
        )
        .with_parent(r1.hash())
        .with_parent(r2.hash());

        assert_eq!(merge.parent_receipt_ids.len(), 2);
        assert_eq!(merge.parent_receipt_ids[0], r1.hash());
        assert_eq!(merge.parent_receipt_ids[1], r2.hash());
    }

    #[test]
    fn test_parent_receipt_ids_with_parents_bulk() {
        let hashes = vec![
            "hash_a".to_string(),
            "hash_b".to_string(),
            "hash_c".to_string(),
        ];
        let receipt = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Merge,
            EpistemicLabel::Derivation,
            50,
        )
        .with_parents(hashes.clone());

        assert_eq!(receipt.parent_receipt_ids, hashes);
    }

    #[test]
    fn test_parent_receipt_ids_empty_by_default() {
        let receipt = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );
        assert!(receipt.parent_receipt_ids.is_empty());
    }

    #[test]
    fn test_parent_receipt_ids_serializes_optional() {
        let receipt = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );
        let json = serde_json::to_string(&receipt).unwrap();
        // When empty vec, skip_serializing_if prevents it from appearing
        assert!(!json.contains("parent_receipt_ids"));

        let with_parents = receipt.with_parent("abc123");
        let json2 = serde_json::to_string(&with_parents).unwrap();
        assert!(json2.contains("parent_receipt_ids"));
        assert!(json2.contains("abc123"));
    }

    #[test]
    fn test_dag_fanout_merge_pattern() {
        // Simulate: Input → [A, B, C] → Merge → Output
        let input = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );

        // Fan-out: 3 parallel lanes
        let a = FlowReceipt::new_chained(
            &input,
            "agent-a",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            50,
        )
        .with_topology("fan-out:research", 0);

        let b = FlowReceipt::new_chained(
            &input,
            "agent-b",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            75,
        )
        .with_topology("fan-out:research", 1);

        let c = FlowReceipt::new_chained(
            &input,
            "agent-c",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            60,
        )
        .with_topology("fan-out:research", 2);

        // Merge point: 3 parents from the fan-out lanes
        let merge = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Merge,
            EpistemicLabel::Derivation,
            30,
        )
        .with_parents(vec![a.hash(), b.hash(), c.hash()])
        .with_topology("fan-out:research", 0);

        assert_eq!(merge.parent_receipt_ids.len(), 3);
        assert_eq!(merge.topology_id.as_deref(), Some("fan-out:research"));
    }

    #[test]
    fn test_receipt_with_both_new_fields() {
        let receipt = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        )
        .with_routed_organ("geox")
        .with_parent("prev_hash_123");

        let json = serde_json::to_string(&receipt).unwrap();
        assert!(json.contains("routed_organ"));
        assert!(json.contains("geox"));
        assert!(json.contains("parent_receipt_ids"));
        assert!(json.contains("prev_hash_123"));

        // Roundtrip
        let deserialized: FlowReceipt = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.routed_organ.as_deref(), Some("geox"));
        assert_eq!(deserialized.parent_receipt_ids, vec!["prev_hash_123"]);
    }

    // ── Genesis Bridge tests (RG-3, 2026-09-12) ──────────────────────────

    #[test]
    fn test_genesis_anchor_none_by_default() {
        let r = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );
        assert!(r.genesis_anchor.is_none());
    }

    #[test]
    fn test_genesis_anchor_builder() {
        let r = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        )
        .with_genesis_anchor("RCP-000");
        assert_eq!(r.genesis_anchor.as_deref(), Some("RCP-000"));
    }

    #[test]
    fn test_genesis_anchor_serializes_optional() {
        let r = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        );
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("genesis_anchor"));

        let anchored = r.with_genesis_anchor("/000");
        let json2 = serde_json::to_string(&anchored).unwrap();
        assert!(json2.contains("genesis_anchor"));
        assert!(json2.contains("/000"));
    }

    #[test]
    fn test_genesis_anchor_backward_compat() {
        let json = r#"{
            "receipt_id": "00000000-0000-0000-0000-000000000001",
            "previous_receipt_hash": null,
            "created_at": "2026-09-12T00:00:00Z",
            "actor_id": "agent", "session_id": "s1",
            "step_type": "Execute", "cost_ns": 100,
            "epistemic_label": "Observation", "floor_verdict": "Pass",
            "cooling_decision": "None", "step_number": 0,
            "risk_class": "T0Observe"
        }"#;
        let r: FlowReceipt = serde_json::from_str(json).unwrap();
        assert!(r.genesis_anchor.is_none());
    }

    #[test]
    fn test_genesis_receipt_pattern() {
        // Genesis Receipt: root of the Reality Graph
        let genesis = FlowReceipt::new_first(
            "arifOS",
            "genesis-session",
            StepType::Seal,
            EpistemicLabel::Seal,
            0,
        )
        .with_genesis_anchor("RCP-000")
        .with_floor_verdict(FloorVerdict::Pass);

        assert!(genesis.parent_receipt_ids.is_empty());
        assert!(genesis.previous_receipt_hash.is_none());
        assert_eq!(genesis.genesis_anchor.as_deref(), Some("RCP-000"));

        // First action chains from genesis
        let first_action = FlowReceipt::new_chained(
            &genesis,
            "333-AGI",
            "genesis-session",
            StepType::Execute,
            EpistemicLabel::Observation,
            500,
        )
        .with_routed_organ("geox")
        .with_parent(genesis.hash());

        assert_eq!(first_action.parent_receipt_ids.len(), 1);
        assert!(first_action.genesis_anchor.is_none());

        // Serialize genesis — genesis_anchor appears
        let json = serde_json::to_string(&genesis).unwrap();
        assert!(json.contains("genesis_anchor"));
        assert!(json.contains("RCP-000"));

        // Serialize first action — genesis_anchor absent
        let json2 = serde_json::to_string(&first_action).unwrap();
        assert!(!json2.contains("genesis_anchor"));
    }

    #[test]
    fn test_genesis_bridge_full_lineage() {
        // Genesis → Observe → Interpret → Verify
        let genesis = FlowReceipt::new_first(
            "arifOS",
            "bridge-test",
            StepType::Seal,
            EpistemicLabel::Seal,
            0,
        )
        .with_genesis_anchor("RCP-000");

        let observe = FlowReceipt::new_chained(
            &genesis,
            "333-AGI",
            "bridge-test",
            StepType::Execute,
            EpistemicLabel::Observation,
            100,
        )
        .with_routed_organ("GEOX")
        .with_parent(genesis.hash());

        let interpret = FlowReceipt::new_chained(
            &observe,
            "333-AGI",
            "bridge-test",
            StepType::Execute,
            EpistemicLabel::Interpretation,
            200,
        )
        .with_parent(observe.hash());

        let verify = FlowReceipt::new_chained(
            &interpret,
            "555-ASI",
            "bridge-test",
            StepType::Verify,
            EpistemicLabel::Derivation,
            150,
        )
        .with_parent(interpret.hash());

        // Lineage reconstructable: verify → interpret → observe → genesis → RCP-000
        assert!(verify.parent_receipt_ids.contains(&interpret.hash()));
        assert!(interpret.parent_receipt_ids.contains(&observe.hash()));
        assert!(observe.parent_receipt_ids.contains(&genesis.hash()));
        assert_eq!(genesis.genesis_anchor.as_deref(), Some("RCP-000"));
        assert!(observe.genesis_anchor.is_none());
        assert!(interpret.genesis_anchor.is_none());
        assert!(verify.genesis_anchor.is_none());
    }
}

// ── Backward Compatibility Types ─────────────────────────────────────────
// These bridge the sibling subagent's channel.rs/scheduler.rs code
// (written in parallel with the old FlowReceipt API) to the new v1 API.

/// Legacy alias — maps to EpistemicLabel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EpistemicTag {
    Observation,
    Derivation,
    Interpretation,
    Specification,
    Unclassified,
}

impl From<EpistemicTag> for EpistemicLabel {
    fn from(tag: EpistemicTag) -> Self {
        match tag {
            EpistemicTag::Observation => EpistemicLabel::Observation,
            EpistemicTag::Derivation => EpistemicLabel::Derivation,
            EpistemicTag::Interpretation => EpistemicLabel::Interpretation,
            EpistemicTag::Specification => EpistemicLabel::Specification,
            EpistemicTag::Unclassified => EpistemicLabel::Observation, // default
        }
    }
}

/// Legacy governance overlay — carried alongside FlowReceipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceOverlay {
    pub floor_verdict: FloorVerdict,
    pub cooling: CoolingDecision,
}

impl GovernanceOverlay {
    pub fn none() -> Self {
        Self {
            floor_verdict: FloorVerdict::Pass,
            cooling: CoolingDecision::None,
        }
    }
}

/// Legacy Agentic Flow Quotient metric — replaced by FlowQuotient.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AFQMetric {
    pub flow_quotient: f64,
    pub execution_steps: u64,
    pub governance_steps: u64,
    pub execute_count: u64,
    pub verify_count: u64,
    pub afq: f64,
    pub verdict: String,
}

impl AFQMetric {
    pub fn compute(receipts: &[FlowReceipt]) -> Self {
        let fq = FlowQuotient::compute(receipts);
        Self {
            flow_quotient: fq.quotient.unwrap_or(0.0),
            execution_steps: fq.execute_count as u64,
            governance_steps: fq.verify_count as u64,
            execute_count: fq.execute_count as u64,
            verify_count: fq.verify_count as u64,
            afq: fq.quotient.unwrap_or(0.0),
            verdict: fq.verdict.to_string(),
        }
    }

    /// Legacy constructor — bridges old scheduler.rs call site.
    pub fn new(execution_steps: u64, governance_steps: u64) -> Self {
        let quotient = if governance_steps == 0 {
            f64::MAX
        } else {
            execution_steps as f64 / governance_steps as f64
        };
        Self {
            flow_quotient: quotient,
            execution_steps,
            governance_steps,
            execute_count: execution_steps,
            verify_count: governance_steps,
            afq: quotient,
            verdict: if quotient > 3.0 {
                "OPTIMAL"
            } else if quotient > 1.0 {
                "BALANCED"
            } else if quotient > 0.5 {
                "WATCHING"
            } else {
                "STUCK"
            }
            .to_string(),
        }
    }

    /// Legacy diagnosis — returns the verdict string.
    pub fn diagnosis(&self) -> &str {
        &self.verdict
    }
}

impl Default for AFQMetric {
    fn default() -> Self {
        Self {
            flow_quotient: 0.0,
            execution_steps: 0,
            governance_steps: 0,
            execute_count: 0,
            verify_count: 0,
            afq: 0.0,
            verdict: "UNKNOWN".to_string(),
        }
    }
}

/// Legacy receipt chain alias — ordered list of receipts.
pub type ReceiptChain = Vec<FlowReceipt>;

impl FlowReceipt {
    /// Legacy constructor — bridges old channel.rs API to new FlowReceipt v1.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        _payload_bytes: &[u8],
        _epoch: u64,
        _lease_id: uuid::Uuid,
        actor_id: &str,
        _cc_id: uuid::Uuid,
        _parent_receipt_id: Option<uuid::Uuid>,
        epistemic_tag: EpistemicTag,
        _state_root: MerkleRoot,
        _governance: GovernanceOverlay,
        _afq: AFQMetric,
    ) -> Self {
        Self::new_first(
            actor_id,
            "legacy-session",
            StepType::Execute,
            epistemic_tag.into(),
            0,
        )
    }
}
