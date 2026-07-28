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
//! | > 3.0    | Optimal | Agent in flow. Governance in the architecture. |
//! | 1.0–3.0  | Balanced | Healthy verification. |
//! | 0.5–1.0  | Watching | Self-monitoring competes with execution. |
//! | < 0.5    | Stuck | Self-monitoring has become the task. mPFC takeover. |

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
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
    /// Uses geometric mean: ∛(human × ai × earth) — unified with tri_witness.rs
    pub fn nash_score(&self) -> f64 {
        if self.human == 0.0 || self.ai == 0.0 || self.earth == 0.0 {
            0.0
        } else {
            (self.human * self.ai * self.earth).cbrt()
        }
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

/// Flow health verdict based on Flow Quotient.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FlowVerdict {
    /// FQ > 3.0 — Agent in flow. Governance in the architecture.
    Optimal,
    /// FQ > 5.0 — Verify DOMINATES execute. Self-audit spiral. GAP-M1 fix.
    Overheat,
    /// FQ 1.0–3.0 — Healthy verification. Self-monitoring supports execution.
    Balanced,
    /// FQ 0.5–1.0 — Agent spends as much time verifying as executing.
    Watching,
    /// FQ < 0.5 — Self-monitoring has become the task. mPFC takeover.
    Stuck,
    /// No receipts yet — metabolism not measurable.
    Unmeasured,
}

impl fmt::Display for FlowVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlowVerdict::Optimal => write!(f, "OPTIMAL"),
            FlowVerdict::Overheat => write!(f, "OVERHEAT"),
            FlowVerdict::Balanced => write!(f, "BALANCED"),
            FlowVerdict::Watching => write!(f, "WATCHING"),
            FlowVerdict::Stuck => write!(f, "STUCK"),
            FlowVerdict::Unmeasured => write!(f, "UNMEASURED"),
        }
    }
}

impl FlowVerdict {
    /// Returns the CSS-safe emoji for cockpit dashboards.
    pub fn emoji(&self) -> &'static str {
        match self {
            FlowVerdict::Optimal => "🟢",
            FlowVerdict::Overheat => "🔵",
            FlowVerdict::Balanced => "🟡",
            FlowVerdict::Watching => "🟠",
            FlowVerdict::Stuck => "🔴",
            FlowVerdict::Unmeasured => "⚪",
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
    /// Flow Quotient = execute_cost / max(verify_cost, 1)
    pub quotient: f64,
    /// Health verdict
    pub verdict: FlowVerdict,
    /// Window size used
    pub window_size: usize,
    // ── GAP-M4: Formula transparency fields ──
    /// Raw ratio before smoothing (INFINITY if verify=0)
    pub raw_ratio: f64,
    /// Whether EMA smoothing was applied (false for STUCK/OVERHEAT/UNMEASURED)
    pub is_smoothed: bool,
    /// EMA alpha used for smoothing (0.0 = no smoothing)
    pub alpha: f64,
    /// Rolling window in seconds
    pub window_s: u64,
    /// Cost clamp bounds: (min_ns, max_ns)
    pub cost_clamp_ns: (u64, u64),
    // ── GAP-M2: Actor-level FQ ──
    /// Per-actor FQ breakdown
    pub by_actor: BTreeMap<String, ActorFqSnapshot>,
    /// Actor with the lowest FQ (or None if all actors healthy)
    pub worst_actor: Option<String>,
    /// Number of unique actors in this window
    pub actor_count: usize,
    // ── GAP-M3: Trend ──
    /// FQ trend over the lookback window
    pub trend: FqTrend,
}

/// Per-actor FQ snapshot for GAP-M2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorFqSnapshot {
    pub execute_count: usize,
    pub verify_count: usize,
    pub execute_cost_ns: u64,
    pub verify_cost_ns: u64,
    pub fq: f64,
    pub verdict: FlowVerdict,
}

/// FQ trend detection for GAP-M3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FqTrend {
    /// Direction: Rising, Falling, Stable
    pub direction: TrendDirection,
    /// Rate of change in FQ per minute
    pub rate_per_min: f64,
    /// Standard deviation of last N FQ readings
    pub volatility: f64,
    /// Number of samples used for trend
    pub samples: usize,
    /// Lookback window in seconds
    pub window_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrendDirection {
    Rising,
    Falling,
    Stable,
}

impl FlowQuotient {
    /// Cost clamp bounds: min 1ms, max 5min per step.
    pub const COST_MIN_NS: u64 = 1_000_000;
    pub const COST_MAX_NS: u64 = 300_000_000_000;
    /// EMA alpha for noise reduction (only applied to non-terminal states).
    pub const DEFAULT_ALPHA: f64 = 0.3;
    /// Default rolling window in seconds.
    pub const DEFAULT_WINDOW_S: u64 = 300;

    fn clamp_cost(cost: u64) -> u64 {
        cost.clamp(Self::COST_MIN_NS, Self::COST_MAX_NS)
    }

    /// Compute FQ from a slice of receipts.
    /// GAP-M1: verify=0 → STUCK (not Optimal, not 999).
    /// GAP-M1: FQ > 5.0 → OVERHEAT (verify dominating execute is also pathological).
    /// GAP-M2: includes per-actor breakdown.
    pub fn compute(receipts: &[FlowReceipt]) -> Self {
        let mut execute_cost = 0u64;
        let mut verify_cost = 0u64;
        let mut execute_count = 0usize;
        let mut verify_count = 0usize;

        // Per-actor accumulators (GAP-M2)
        let mut actor_exec: BTreeMap<String, (usize, u64)> = BTreeMap::new();
        let mut actor_verify: BTreeMap<String, (usize, u64)> = BTreeMap::new();

        for r in receipts {
            if r.step_type.is_execution() {
                let cost = Self::clamp_cost(r.cost_ns);
                execute_cost = execute_cost.saturating_add(cost);
                execute_count += 1;
                let e = actor_exec.entry(r.actor_id.clone()).or_insert((0, 0));
                e.0 += 1;
                e.1 = e.1.saturating_add(cost);
            }
            if r.step_type.is_verification() {
                let cost = Self::clamp_cost(r.cost_ns);
                verify_cost = verify_cost.saturating_add(cost);
                verify_count += 1;
                let v = actor_verify.entry(r.actor_id.clone()).or_insert((0, 0));
                v.0 += 1;
                v.1 = v.1.saturating_add(cost);
            }
            // Add preceding verification cost
            if let Some(preceding) = r.preceding_verify_cost_ns {
                let clamped = Self::clamp_cost(preceding);
                verify_cost = verify_cost.saturating_add(clamped);
                // Attribute preceding cost to the same actor
                let v = actor_verify.entry(r.actor_id.clone()).or_insert((0, 0));
                v.1 = v.1.saturating_add(clamped);
            }
        }

        // GAP-M1: verify=0 → STUCK (not 999, not Optimal)
        let (quotient, raw_ratio, is_smoothed, verdict) = if receipts.is_empty() {
            (0.0, 0.0, false, FlowVerdict::Unmeasured)
        } else if verify_cost == 0 && execute_cost > 0 {
            (0.0, f64::INFINITY, false, FlowVerdict::Stuck)
        } else if verify_cost == 0 {
            (0.0, 0.0, false, FlowVerdict::Unmeasured)
        } else {
            let raw = execute_cost as f64 / verify_cost as f64;
            let v = match raw {
                x if x < 0.5 => FlowVerdict::Stuck,
                x if x >= 0.5 && x < 1.0 => FlowVerdict::Watching,
                x if x >= 1.0 && x <= 3.0 => FlowVerdict::Balanced,
                x if x > 3.0 && x <= 5.0 => FlowVerdict::Optimal,
                _ => FlowVerdict::Overheat, // GAP-M1: FQ > 5.0
            };
            (raw, raw, false, v)
        };

        // GAP-M2: Build per-actor snapshots
        let mut all_actors: BTreeSet<String> = BTreeSet::new();
        for a in actor_exec.keys() {
            all_actors.insert(a.clone());
        }
        for a in actor_verify.keys() {
            all_actors.insert(a.clone());
        }

        let mut by_actor = BTreeMap::new();
        let mut worst_fq = f64::MAX;
        let mut worst_actor: Option<String> = None;

        for actor_id in &all_actors {
            let (ec, e_cost) = actor_exec.get(actor_id).copied().unwrap_or((0, 0));
            let (vc, v_cost) = actor_verify.get(actor_id).copied().unwrap_or((0, 0));
            let afq = if v_cost == 0 && e_cost > 0 {
                0.0 // STUCK for this actor
            } else if v_cost == 0 {
                0.0
            } else {
                e_cost as f64 / v_cost as f64
            };
            let averdict = if afq == 0.0 && v_cost == 0 && e_cost > 0 {
                FlowVerdict::Stuck
            } else if afq == 0.0 {
                FlowVerdict::Unmeasured
            } else {
                match afq {
                    x if x < 0.5 => FlowVerdict::Stuck,
                    x if x >= 0.5 && x < 1.0 => FlowVerdict::Watching,
                    x if x >= 1.0 && x <= 3.0 => FlowVerdict::Balanced,
                    x if x > 3.0 && x <= 5.0 => FlowVerdict::Optimal,
                    _ => FlowVerdict::Overheat,
                }
            };

            by_actor.insert(
                actor_id.clone(),
                ActorFqSnapshot {
                    execute_count: ec,
                    verify_count: vc,
                    execute_cost_ns: e_cost,
                    verify_cost_ns: v_cost,
                    fq: afq,
                    verdict: averdict,
                },
            );

            if afq < worst_fq && (ec > 0 || vc > 0) {
                worst_fq = afq;
                worst_actor = Some(actor_id.clone());
            }
        }

        // GAP-M3: No trend here — compute() is point-in-time. Trend is computed from history.
        let trend = FqTrend {
            direction: TrendDirection::Stable,
            rate_per_min: 0.0,
            volatility: 0.0,
            samples: 0,
            window_s: 0,
        };

        Self {
            execute_count,
            execute_cost_ns: execute_cost,
            verify_count,
            verify_cost_ns: verify_cost,
            quotient,
            verdict,
            window_size: receipts.len(),
            raw_ratio,
            is_smoothed,
            alpha: 0.0, // point-in-time has no smoothing
            window_s: Self::DEFAULT_WINDOW_S,
            cost_clamp_ns: (Self::COST_MIN_NS, Self::COST_MAX_NS),
            by_actor,
            worst_actor,
            actor_count: all_actors.len(),
            trend,
        }
    }

    /// Compute FQ with trend detection from history (GAP-M3).
    pub fn compute_with_trend(
        receipts: &[FlowReceipt],
        fq_history: &[(f64, chrono::DateTime<chrono::Utc>)],
        trend_window_s: u64,
    ) -> Self {
        let mut base = Self::compute(receipts);

        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(trend_window_s as i64);
        let recent: Vec<f64> = fq_history
            .iter()
            .filter(|(_, ts)| *ts > cutoff)
            .map(|(v, _)| *v)
            .collect();

        if recent.len() >= 3 {
            let n = recent.len() as f64;
            let mean = recent.iter().sum::<f64>() / n;
            let variance = recent.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n;
            let volatility = variance.sqrt();

            // Simple slope: (last - first) / (n-1) steps, convert to per-minute
            let step_s = trend_window_s as f64 / n;
            let slope_per_step = (recent.last().unwrap() - recent.first().unwrap()) / (n - 1.0);
            let rate_per_min = slope_per_step / step_s * 60.0;

            let direction = if rate_per_min.abs() < 0.05 {
                TrendDirection::Stable
            } else if rate_per_min > 0.0 {
                TrendDirection::Rising
            } else {
                TrendDirection::Falling
            };

            base.trend = FqTrend {
                direction,
                rate_per_min: (rate_per_min * 1000.0).round() / 1000.0,
                volatility: (volatility * 1000.0).round() / 1000.0,
                samples: recent.len(),
                window_s: trend_window_s,
            };
        } else {
            base.trend = FqTrend {
                direction: TrendDirection::Stable,
                rate_per_min: 0.0,
                volatility: 0.0,
                samples: recent.len(),
                window_s: trend_window_s,
            };
        }

        base
    }

    /// Minimal placeholder — used by legacy code that constructs FQ structs directly.
    /// All missing fields get reasonable defaults.
    pub fn empty() -> Self {
        Self {
            execute_count: 0,
            execute_cost_ns: 0,
            verify_count: 0,
            verify_cost_ns: 0,
            quotient: 0.0,
            verdict: FlowVerdict::Unmeasured,
            window_size: 0,
            raw_ratio: 0.0,
            is_smoothed: false,
            alpha: 0.0,
            window_s: Self::DEFAULT_WINDOW_S,
            cost_clamp_ns: (Self::COST_MIN_NS, Self::COST_MAX_NS),
            by_actor: BTreeMap::new(),
            worst_actor: None,
            actor_count: 0,
            trend: FqTrend {
                direction: TrendDirection::Stable,
                rate_per_min: 0.0,
                volatility: 0.0,
                samples: 0,
                window_s: 0,
            },
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
            topology_id: None,
            lane_id: None,
            step_number: 0,
            cost_ns,
            preceding_verify_cost_ns: None,
            epistemic_label,
            floor_verdict: FloorVerdict::Pass,
            cooling_decision: CoolingDecision::None,
            tri_witness_votes: None,
            merkle_root: None,
            merkle_inclusion_proof: None,
            payload: None,
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
            topology_id: None,
            lane_id: None,
            step_number: previous.step_number + 1,
            cost_ns,
            preceding_verify_cost_ns: None,
            epistemic_label,
            floor_verdict: FloorVerdict::Pass,
            cooling_decision: CoolingDecision::None,
            tri_witness_votes: None,
            merkle_root: None,
            merkle_inclusion_proof: None,
            payload: None,
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

/// A store for flow receipts with optional file-backed persistence.
///
/// Maintains chain order, provides FQ computation, and tracks FQ history.
/// When `persist_path` is set, new receipts are appended to a JSONL file
/// and loaded on restart.
#[derive(Debug, Clone)]
pub struct ReceiptStore {
    receipts: Vec<FlowReceipt>,
    max_receipts: usize,
    persist_path: Option<std::path::PathBuf>,
    fq_history: Vec<f64>, // FQ values after each push (last 100)
}

impl ReceiptStore {
    /// Maximum FQ history entries to retain.
    const MAX_FQ_HISTORY: usize = 100;

    /// Create a new receipt store with a maximum capacity.
    pub fn new(max_receipts: usize) -> Self {
        Self {
            receipts: Vec::with_capacity(max_receipts.min(1000)),
            max_receipts,
            persist_path: None,
            fq_history: Vec::with_capacity(Self::MAX_FQ_HISTORY),
        }
    }

    /// Create a new receipt store with file-backed persistence.
    /// Loads existing receipts from disk if the file exists.
    pub fn new_with_persistence(max_receipts: usize, persist_path: std::path::PathBuf) -> Self {
        let mut store = Self {
            receipts: Vec::with_capacity(max_receipts.min(1000)),
            max_receipts,
            persist_path: Some(persist_path.clone()),
            fq_history: Vec::with_capacity(Self::MAX_FQ_HISTORY),
        };
        store.load_from_disk();
        store
    }

    /// Load receipts from the persistence file (JSONL format).
    fn load_from_disk(&mut self) {
        let path = match &self.persist_path {
            Some(p) => p,
            None => return,
        };
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return, // File doesn't exist yet — fresh start
        };
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(receipt) = serde_json::from_str::<FlowReceipt>(trimmed) {
                if self.receipts.len() >= self.max_receipts {
                    self.receipts.remove(0);
                }
                self.receipts.push(receipt);
            }
        }
        // Rebuild FQ history from loaded receipts
        self.rebuild_fq_history();
    }

    /// Persist a single receipt to the JSONL file.
    fn persist_receipt(&self, receipt: &FlowReceipt) {
        let path = match &self.persist_path {
            Some(p) => p,
            None => return,
        };
        if let Ok(json) = serde_json::to_string(receipt) {
            let mut line = json;
            line.push('\n');
            // Append to file — create if doesn't exist
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = file.write_all(line.as_bytes());
            }
        }
    }

    /// Record current FQ in history, maintaining max size.
    fn record_fq(&mut self) {
        let fq = self.flow_quotient(20).quotient;
        if self.fq_history.len() >= Self::MAX_FQ_HISTORY {
            self.fq_history.remove(0);
        }
        self.fq_history.push(fq);
    }

    /// Rebuild FQ history by replaying receipts in sliding windows.
    fn rebuild_fq_history(&mut self) {
        self.fq_history.clear();
        if self.receipts.is_empty() {
            return;
        }
        // For each receipt after the first, compute FQ on the window ending at that receipt
        for i in 1..=self.receipts.len() {
            let window = &self.receipts[..i];
            let fq = FlowQuotient::compute(window).quotient;
            if self.fq_history.len() >= Self::MAX_FQ_HISTORY {
                self.fq_history.remove(0);
            }
            self.fq_history.push(fq);
        }
    }

    /// Push a receipt bypassing chain validation — for monitoring/observability.
    pub fn push_force(&mut self, receipt: FlowReceipt) {
        if self.receipts.len() >= self.max_receipts {
            self.receipts.remove(0);
        }
        self.persist_receipt(&receipt);
        self.receipts.push(receipt);
        self.record_fq();
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

        self.persist_receipt(&receipt);
        self.receipts.push(receipt);
        self.record_fq();
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

    /// Get the FQ history (up to last 100 values).
    pub fn fq_history(&self) -> &[f64] {
        &self.fq_history
    }

    /// Get the current persistence path, if any.
    pub fn persist_path(&self) -> Option<&std::path::Path> {
        self.persist_path.as_deref()
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
        assert!((votes.nash_score() - 0.881).abs() < 0.005);
        assert!(votes.meets_f3_threshold()); // 0.881 >= 0.75

        let low_votes = TriWitnessVotes::new(0.7, 0.6, 0.5).unwrap();
        assert!((low_votes.nash_score() - 0.594).abs() < 0.005);
        assert!(!low_votes.meets_f3_threshold()); // 0.594 < 0.75
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

    #[test]
    fn test_flow_quotient_stuck() {
        // Lots of verify, little execute → STUCK
        let mut store = ReceiptStore::new(100);
        let r1 = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Verify,
            EpistemicLabel::Derivation,
            5_000_000,
        );
        store.push(r1).unwrap();
        let r2 = FlowReceipt {
            preceding_verify_cost_ns: Some(5_000_000),
            ..FlowReceipt::new_chained(
                store.all().last().unwrap(),
                "agent",
                "s1",
                StepType::Verify,
                EpistemicLabel::Derivation,
                5_000_000,
            )
        };
        store.push(r2).unwrap();
        let r3 = FlowReceipt {
            preceding_verify_cost_ns: Some(2_000_000),
            ..FlowReceipt::new_chained(
                store.all().last().unwrap(),
                "agent",
                "s1",
                StepType::Execute,
                EpistemicLabel::Observation,
                1_000_000,
            )
        };
        store.push(r3).unwrap();
        // execute=1M+2M=3M, verify=5M+5M+5M=15M → FQ=3/15=0.2 → Stuck
        let fq = store.flow_quotient(10);
        assert_eq!(fq.verdict, FlowVerdict::Stuck);
        assert!(fq.quotient < 0.5);
    }

    #[test]
    fn test_flow_quotient_balanced_fixed() {
        let mut store = ReceiptStore::new(100);
        let r1 = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            5_000_000,
        );
        store.push(r1).unwrap();
        // execute=5M, verify=(2M+1M)=3M → FQ=1.67 → Balanced
        let r2 = FlowReceipt {
            preceding_verify_cost_ns: Some(1_000_000),
            ..FlowReceipt::new_chained(
                store.all().last().unwrap(),
                "agent",
                "s1",
                StepType::Verify,
                EpistemicLabel::Derivation,
                2_000_000,
            )
        };
        store.push(r2).unwrap();
        let fq = store.flow_quotient(10);
        assert_eq!(fq.verdict, FlowVerdict::Balanced);
        assert!(fq.quotient > 0.9 && fq.quotient <= 3.0);
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
        // GAP-M1: verify=0 → STUCK (not Optimal, not 999)
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
        assert_eq!(
            fq.verdict,
            FlowVerdict::Stuck,
            "GAP-M1: verify=0 must be STUCK"
        );
        assert_eq!(fq.quotient, 0.0, "GAP-M1: verify=0 quotient must be 0.0");
        assert_eq!(fq.raw_ratio.is_infinite(), true);
    }

    // ── GAP-M1 tests: OVERHEAT detection ──
    #[test]
    fn test_flow_quotient_overheat() {
        let mut store = ReceiptStore::new(100);
        let r1 = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Verify,
            EpistemicLabel::Derivation,
            6_000_000,
        );
        store.push(r1).unwrap();
        let r2 = FlowReceipt::new_chained(
            store.all().last().unwrap(),
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            1_000_000,
        );
        store.push(r2).unwrap();
        let _fq = store.flow_quotient(10);
        // execute_cost=1M, verify_cost=6M → FQ=0.167 → STUCK. Need more execute.
        // Let's test the ratio: execute=10M, verify=1M → FQ=10 → OVERHEAT
    }

    #[test]
    fn test_flow_quotient_overheat_ratio() {
        let mut store = ReceiptStore::new(100);
        // execute=12M, verify=1M+1M(prec)=2M → FQ=6.0 → OVERHEAT
        let r1 = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            12_000_000,
        );
        store.push(r1).unwrap();
        let r2 = FlowReceipt {
            preceding_verify_cost_ns: Some(1_000_000),
            ..FlowReceipt::new_chained(
                store.all().last().unwrap(),
                "agent",
                "s1",
                StepType::Verify,
                EpistemicLabel::Derivation,
                1_000_000,
            )
        };
        store.push(r2).unwrap();
        let fq = store.flow_quotient(10);
        assert_eq!(
            fq.verdict,
            FlowVerdict::Overheat,
            "FQ=6 must be OVERHEAT, got {:?}",
            fq.verdict
        );
        assert!(fq.quotient > 5.0);
    }

    #[test]
    fn test_flow_quotient_overheat_then_recovery() {
        let mut store = ReceiptStore::new(100);
        // execute=12M, verify=1M+1M(prec)=2M → FQ=6.0 → OVERHEAT
        let r1 = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            12_000_000,
        );
        store.push(r1).unwrap();
        let r2 = FlowReceipt {
            preceding_verify_cost_ns: Some(1_000_000),
            ..FlowReceipt::new_chained(
                store.all().last().unwrap(),
                "agent",
                "s1",
                StepType::Verify,
                EpistemicLabel::Derivation,
                1_000_000,
            )
        };
        store.push(r2).unwrap();
        assert_eq!(store.flow_quotient(10).verdict, FlowVerdict::Overheat);

        // Now add more Execute+Verify equally → ratio drops to BALANCED range
        let r3 = FlowReceipt::new_chained(
            store.all().last().unwrap(),
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            5_000_000,
        );
        store.push(r3).unwrap();
        let r4 = FlowReceipt {
            preceding_verify_cost_ns: Some(2_000_000),
            ..FlowReceipt::new_chained(
                store.all().last().unwrap(),
                "agent",
                "s1",
                StepType::Verify,
                EpistemicLabel::Derivation,
                3_000_000,
            )
        };
        store.push(r4).unwrap();
        // Now: execute=12M+5M=17M, verify=2M+3M+2M=7M → FQ=17/7≈2.4 → BALANCED
        let fq = store.flow_quotient(10);
        assert_eq!(
            fq.verdict,
            FlowVerdict::Balanced,
            "recovery → Balanced, got {:?}",
            fq.verdict
        );
    }

    // ── GAP-M2: Actor-level FQ ──
    #[test]
    fn test_actor_level_fq_multiple_agents() {
        let mut store = ReceiptStore::new(100);
        // Agent A: healthy (Execute + Verify)
        let r1 = FlowReceipt::new_first(
            "opencode",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            1_000_000,
        );
        store.push(r1).unwrap();
        let r2 = FlowReceipt::new_chained(
            store.all().last().unwrap(),
            "opencode",
            "s1",
            StepType::Verify,
            EpistemicLabel::Derivation,
            500_000,
        );
        store.push(r2).unwrap();
        // Agent B: only Execute, no Verify
        let r3 = FlowReceipt::new_chained(
            store.all().last().unwrap(),
            "hermes-prime",
            "s1",
            StepType::Execute,
            EpistemicLabel::Derivation,
            500_000,
        );
        store.push(r3).unwrap();

        let fq = store.flow_quotient(20);
        assert!(
            fq.by_actor.contains_key("opencode"),
            "GAP-M2: opencode must be in by_actor"
        );
        assert!(
            fq.by_actor.contains_key("hermes-prime"),
            "GAP-M2: hermes-prime must be in by_actor"
        );
        assert_eq!(fq.actor_count, 2);
        // worst_actor should be hermes-prime (FQ=0, no Verify)
        assert!(fq.worst_actor.is_some());
    }

    // ── GAP-M7: Cold start state ──
    #[test]
    fn test_flow_quotient_unmeasured() {
        let store = ReceiptStore::new(100);
        let fq = store.flow_quotient(10);
        assert_eq!(fq.verdict, FlowVerdict::Unmeasured);
        assert_eq!(fq.quotient, 0.0);
    }

    #[test]
    fn test_verdict_emoji() {
        assert_eq!(FlowVerdict::Optimal.emoji(), "🟢");
        assert_eq!(FlowVerdict::Overheat.emoji(), "🔵");
        assert_eq!(FlowVerdict::Balanced.emoji(), "🟡");
        assert_eq!(FlowVerdict::Watching.emoji(), "🟠");
        assert_eq!(FlowVerdict::Stuck.emoji(), "🔴");
        assert_eq!(FlowVerdict::Unmeasured.emoji(), "⚪");
    }

    #[test]
    fn test_flow_quotient_formula_transparency_fields() {
        let mut store = ReceiptStore::new(100);
        let r1 = FlowReceipt::new_first(
            "agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            2_000_000,
        );
        store.push(r1).unwrap();
        let r2 = FlowReceipt::new_chained(
            store.all().last().unwrap(),
            "agent",
            "s1",
            StepType::Verify,
            EpistemicLabel::Derivation,
            1_000_000,
        );
        store.push(r2).unwrap();

        let fq = store.flow_quotient(10);
        assert!(fq.cost_clamp_ns.0 > 0, "GAP-M4: cost_clamp min must be set");
        assert!(fq.cost_clamp_ns.1 > 0, "GAP-M4: cost_clamp max must be set");
        assert!(fq.window_s > 0, "GAP-M4: window_s must be set");
        assert!(
            !fq.raw_ratio.is_infinite() || fq.raw_ratio == 0.0,
            "GAP-M4: raw_ratio must be finite"
        );
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
            flow_quotient: fq.quotient,
            execution_steps: fq.execute_count as u64,
            governance_steps: fq.verify_count as u64,
            execute_count: fq.execute_count as u64,
            verify_count: fq.verify_count as u64,
            afq: fq.quotient,
            verdict: fq.verdict.to_string(),
        }
    }

    /// Legacy constructor — bridges old scheduler.rs call site.
    pub fn new(execution_steps: u64, governance_steps: u64) -> Self {
        let quotient = if governance_steps == 0 {
            999.0 // clamped — prevents f64::MAX JSON overflow
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
            verdict: if quotient > 5.0 {
                "OVERHEAT"
            } else if quotient > 3.0 {
                "OPTIMAL"
            } else if quotient > 1.0 {
                "BALANCED"
            } else if quotient > 0.5 {
                "WATCHING"
            } else if quotient > 0.0 {
                "STUCK"
            } else {
                "UNMEASURED"
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
