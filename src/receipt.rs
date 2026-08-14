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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskClass {
    /// T0: Read, probe, observe — minimal risk. FQ floor: 0.1
    T0Observe,
    /// T1: Edit, test, commit, lint — moderate risk. FQ floor: 0.3
    T1Mutate,
    /// T2: Deploy, restart, multi-file — elevated risk. FQ floor: 0.5
    T2Deploy,
    /// T3: Irreversible, credential rotation, production — critical risk. FQ floor: 1.0
    T3Irreversible,
}

impl Default for RiskClass {
    fn default() -> Self {
        Self::T0Observe
    }
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

/// Flow health verdict based on Flow Quotient (v2.1 — 2026-08-05).
///
/// Six-state band per Arif F13 spec:
///   UNKNOWN  — verify_count == 0 (missing data)
///   CAUTION  — verify_count < 2 (insufficient pattern)
///   OPTIMAL  — quotient >= 1.0 (verification leads execution)
///   FLOWING  — quotient >= 0.5 (healthy metabolism)
///   STUCK    — quotient >= 0.1 (verification lagging)
///   BURNING  — quotient < 0.1 (execution outruns verification severely)
///
/// Legacy variants (retained for backward compat, not produced by v2.1):
///   Overheat, Balanced, Watching
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FlowVerdict {
    /// verify_count == 0 — no data to ratio against
    Unmeasured,
    /// verify_count < 2 — single verification is coincidence, not pattern
    Caution,
    /// quotient >= 1.0 — verification leads execution (v2.1)
    Optimal,
    /// quotient >= 0.5 — healthy metabolism
    Flowing,
    /// quotient >= 0.1 — verification lagging execution
    Stuck,
    /// quotient < 0.1 — execution far outruns verification
    Burning,
    // ── Legacy variants (retained for backward compat) ──
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
            FlowVerdict::Unmeasured => write!(f, "UNKNOWN"),
            FlowVerdict::Caution => write!(f, "CAUTION"),
            FlowVerdict::Optimal => write!(f, "OPTIMAL"),
            FlowVerdict::Flowing => write!(f, "FLOWING"),
            FlowVerdict::Stuck => write!(f, "STUCK"),
            FlowVerdict::Burning => write!(f, "BURNING"),
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
    ///   - formula_hash: sha256:arifflow-fq-v2.1-2026-08-05
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

        // v2.1: six-state band per Arif F13 spec
        let verdict = if verify_count == 0 {
            // No verification at all — we don't know the state.
            // Prior v2.0 returned OPTIMAL here (f64::MAX). That was misleading.
            FlowVerdict::Unmeasured
        } else if verify_count < 2 {
            // Single verification is coincidence, not pattern.
            FlowVerdict::Caution
        } else if execute_count == 0 {
            // Observe-only window — not stuck, not optimal.
            // Neutral baseline until execution data arrives.
            FlowVerdict::Flowing
        } else {
            let q = raw_quotient.unwrap_or(0.0);
            if q >= 1.0 {
                FlowVerdict::Optimal
            } else if q >= 0.5 {
                FlowVerdict::Flowing
            } else if q >= 0.1 {
                FlowVerdict::Stuck
            } else {
                FlowVerdict::Burning
            }
        };

        // v2.1: quotient is None for Unmeasured/Caution (undefined or insufficient data)
        let quotient = match verdict {
            FlowVerdict::Unmeasured | FlowVerdict::Caution => None,
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
        assert_eq!(fq.verdict, FlowVerdict::Unmeasured);
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
        assert_eq!(fq.verdict, FlowVerdict::Unmeasured);
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
        assert_eq!(fq.verdict, FlowVerdict::Unmeasured);
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
            flow_quotient: quotient.clone(),
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
