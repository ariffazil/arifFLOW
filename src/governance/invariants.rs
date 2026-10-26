// arifFlow governance/invariants.rs
// Flow-Plane Invariant Enforcement — F0-F6 automated gate
// DITEMPA BUKAN DIBERI
//
// This module is the MISSING PIECE. The invariants were declared in canon
// (ARIFLOW_KERNEL_CANON.md) but never enforced. This module:
//
//   1. Defines each invariant as a checkable rule
//   2. Runs an enforcement loop that gates execution
//   3. Auto-throttles actors that violate FQ thresholds
//   4. Emits HOLD signals when invariants are breached
//   5. Writes enforcement receipts to the cooling ledger
//
// The invariants operate at the FLOW PLANE (F0-F6), distinct from the
// execution invariants (A1-A6) enforced in scheduler.rs.

use crate::governance::cooling::{Convergence, CoolingEntry, CoolingLedger, DriftSeverity};
use crate::governance::entity_class::{ActorClass, ClassRegistry};
use crate::receipt::{
    ExplanationClass, ExplanationGate, FLOW_CODE_EXPLANATION_CLASS_INELIGIBLE,
    FLOW_INVARIANT_EXPLANATION_CLASS, FlowQuotient, FlowReceipt, FlowVerdict, ReceiptStore,
    RiskClass,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::time::Instant;

// ── Flow-Plane Invariants ────────────────────────────────────────────────

/// The seven flow-plane invariants from ARIFLOW_KERNEL_CANON.md §Flow-Plane.
/// These are the constitutional laws of movement. They define what arifFlow
/// IS PERMITTED TO BE — distinct from the A-series execution invariants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowInvariant {
    /// F0: Flow transmits, never owns.
    /// arifFlow transmits governed intelligence. It does not originate intent
    /// and does not claim ownership of what it routes.
    F0_TransmitNeverOwn,

    /// F1: Flow schedules, never authorizes.
    /// arifFlow determines execution order. Authorization comes from arifOS
    /// (PLANE 1). Scheduling ≠ permission.
    F1_ScheduleNeverAuthorize,

    /// F2: Flow checkpoints, never judges.
    /// arifFlow records Merkle-anchored state at every super-step. Verdict
    /// grammar (SEAL/HOLD/SABAR/VOID) belongs to arifOS.
    F2_CheckpointNeverJudge,

    /// F3: Flow observes, never interprets.
    /// arifFlow measures FQ, detects drift, emits cooling receipts, and reports
    /// divergence. What drift means belongs to ATLAS333/arifOS.
    F3_ObserveNeverInterpret,

    /// F4: Flow routes execution, never becomes execution authority.
    /// arifFlow dispatches lanes to A-FORGE. A-FORGE owns the execution
    /// decision within its governed scope.
    F4_RouteNeverExecute,

    /// F5: Flow writes receipts, never owns memory.
    /// arifFlow appends checkpoint receipts to VAULT999. VAULT999 sovereignty
    /// belongs to ARIFFAZIL/arifOS.
    F5_ReceiptNeverOwn,

    /// F6: Flow connects organs, never collapses organs.
    /// arifFlow schedules GEOX, WEALTH, WELL, HERMES. It does not merge them,
    /// does not own them, does not understand their domain realities.
    /// F6 is the boundary that prevents the nervous system from becoming a mind.
    F6_ConnectNeverCollapse,
}

impl FlowInvariant {
    /// Human-readable invariant name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::F0_TransmitNeverOwn => "F0: Flow transmits, never owns",
            Self::F1_ScheduleNeverAuthorize => "F1: Flow schedules, never authorizes",
            Self::F2_CheckpointNeverJudge => "F2: Flow checkpoints, never judges",
            Self::F3_ObserveNeverInterpret => "F3: Flow observes, never interprets",
            Self::F4_RouteNeverExecute => {
                "F4: Flow routes execution, never becomes execution authority"
            }
            Self::F5_ReceiptNeverOwn => "F5: Flow writes receipts, never owns memory",
            Self::F6_ConnectNeverCollapse => "F6: Flow connects organs, never collapses organs",
        }
    }

    /// Short code for logging.
    pub fn code(&self) -> &'static str {
        match self {
            Self::F0_TransmitNeverOwn => "F0",
            Self::F1_ScheduleNeverAuthorize => "F1",
            Self::F2_CheckpointNeverJudge => "F2",
            Self::F3_ObserveNeverInterpret => "F3",
            Self::F4_RouteNeverExecute => "F4",
            Self::F5_ReceiptNeverOwn => "F5",
            Self::F6_ConnectNeverCollapse => "F6",
        }
    }

    /// The list of all flow-plane invariants in order.
    pub fn all() -> Vec<FlowInvariant> {
        vec![
            Self::F0_TransmitNeverOwn,
            Self::F1_ScheduleNeverAuthorize,
            Self::F2_CheckpointNeverJudge,
            Self::F3_ObserveNeverInterpret,
            Self::F4_RouteNeverExecute,
            Self::F5_ReceiptNeverOwn,
            Self::F6_ConnectNeverCollapse,
        ]
    }
}

// ── Invariant Status ─────────────────────────────────────────────────────

/// The result of checking a single invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvariantStatus {
    /// Invariant is satisfied. No action needed.
    Pass,
    /// Soft tension detected. Proceed with awareness.
    Warn,
    /// Hard violation. Execution must HOLD.
    Hold,
    /// Critical violation. System is permanently blocked on this invariant.
    Void,
}

impl InvariantStatus {
    pub fn is_blocking(&self) -> bool {
        matches!(self, InvariantStatus::Hold | InvariantStatus::Void)
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            InvariantStatus::Pass => "✅",
            InvariantStatus::Warn => "⚠️",
            InvariantStatus::Hold => "🔴",
            InvariantStatus::Void => "💀",
        }
    }
}

// ── Invariant Check Result ───────────────────────────────────────────────

/// Result of checking a single invariant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantCheck {
    pub invariant: FlowInvariant,
    pub status: InvariantStatus,
    pub reason: String,
    pub evidence: String,
    pub timestamp_ns: i64,
}

impl InvariantCheck {
    pub fn new(
        invariant: FlowInvariant,
        status: InvariantStatus,
        reason: impl Into<String>,
        evidence: impl Into<String>,
    ) -> Self {
        Self {
            invariant,
            status,
            reason: reason.into(),
            evidence: evidence.into(),
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as i64,
        }
    }
}

// ── Full Enforcement Report ──────────────────────────────────────────────

/// Result of checking ALL flow-plane invariants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantReport {
    pub checks: Vec<InvariantCheck>,
    pub overall_status: InvariantStatus,
    pub blocking_count: usize,
    pub warn_count: usize,
    pub fq: Option<FlowQuotient>,
    pub timestamp_ns: i64,
}

impl InvariantReport {
    pub fn new(checks: Vec<InvariantCheck>, fq: Option<FlowQuotient>) -> Self {
        let blocking_count = checks.iter().filter(|c| c.status.is_blocking()).count();
        let warn_count = checks
            .iter()
            .filter(|c| c.status == InvariantStatus::Warn)
            .count();
        let overall_status = if checks.iter().any(|c| c.status == InvariantStatus::Void) {
            InvariantStatus::Void
        } else if checks.iter().any(|c| c.status == InvariantStatus::Hold) {
            InvariantStatus::Hold
        } else if checks.iter().any(|c| c.status == InvariantStatus::Warn) {
            InvariantStatus::Warn
        } else {
            InvariantStatus::Pass
        };

        Self {
            checks,
            overall_status,
            blocking_count,
            warn_count,
            fq,
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as i64,
        }
    }
}

// ── Per-Actor FQ State ───────────────────────────────────────────────────

/// Number of most-recent receipts that count toward an actor's FQ.
/// FQ measures current behaviour, not lifetime age; older receipts age out.
const ACTOR_FQ_WINDOW: usize = 50;

/// Wall-clock nanoseconds since UNIX epoch. Used for durable hold timestamps
/// (held_since_ns) so a hold's first-application time survives a restart —
/// Instant alone is process-local and useless for persistence.
pub fn now_ns() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

/// Tracked state for a single actor in the flow plane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorFlowState {
    pub actor_id: String,
    pub execute_count: u64,
    pub verify_count: u64,
    pub execute_cost_ns: u64,
    pub verify_cost_ns: u64,
    /// [DEPRECATED] Legacy cost-based ratio. Use `quotient` (v2.1 count-based) instead.
    /// Kept for serialization backward compat only. Will be removed when all consumers migrate.
    #[deprecated(note = "use quotient — fq is a legacy alias with cost-based semantics")]
    pub fq: f64,
    /// v2.1 quotient: verify_count / execute_count. None when undefined.
    pub quotient: Option<f64>,
    pub verdict: FlowVerdict,
    pub consecutive_executes_without_verify: u64,
    /// Last risk_class seen from this actor — used for risk-weighted FQ thresholds
    pub last_risk_class: RiskClass,
    pub throttled: bool,
    pub held: bool,
    pub last_seen_ns: i64,
    /// P3 (2026-10-15): Verify receipts the daemon stamped as mechanical
    /// dual-emission (same actor, same session, ≤1s after that actor's
    /// Execute, zero cost). Exposed for measurement; NOT yet excluded from
    /// verify_count / quotient — that enforcement change waits for data.
    #[serde(default)]
    pub mechanical_verify_count: u64,
    /// Last ACTOR_FQ_WINDOW step kinds (1=execution, 2=verification, 0=other).
    /// Counts are decremented when an old step ages out.
    #[serde(default)]
    pub window: VecDeque<u8>,
    /// [F13 2026-10-20] When this hold was first applied. 0 = never held.
    /// Durable timestamp carried into HoldRecord so a restart cannot silently
    /// clear a hold by re-deriving it as under-threshold.
    #[serde(default)]
    pub held_since_ns: i64,
    /// [F-4 2026-10-25] Classification recorded at registration from the
    /// validated ClassRegistry. NEVER settable by a receipt or request payload.
    /// Default (and any unknown token on disk) is `Unclassified`, which COUNTS
    /// toward the federation FQ aggregate — fail-closed.
    #[serde(default)]
    pub actor_class: ActorClass,
}

impl ActorFlowState {
    pub fn new(actor_id: impl Into<String>) -> Self {
        Self {
            actor_id: actor_id.into(),
            execute_count: 0,
            verify_count: 0,
            execute_cost_ns: 0,
            verify_cost_ns: 0,
            fq: 0.0,
            quotient: None,
            verdict: FlowVerdict::Balanced,
            consecutive_executes_without_verify: 0,
            last_risk_class: RiskClass::default(),
            throttled: false,
            held: false,
            last_seen_ns: 0,
            mechanical_verify_count: 0,
            window: VecDeque::new(),
            held_since_ns: 0,
            actor_class: ActorClass::Unclassified,
        }
    }

    /// Update from a flow receipt.
    pub fn ingest(&mut self, receipt: &FlowReceipt) {
        self.last_seen_ns = receipt.created_at.timestamp_nanos_opt().unwrap_or(0);
        self.last_risk_class = receipt.risk_class.clone();

        // Windowed FQ (2026-10-19): count only the most recent ACTOR_FQ_WINDOW
        // steps. A lifetime counter measures age, not current behaviour; one
        // old execute can pin an actor (and the min-gated federation) forever.
        let kind: u8 = if receipt.step_type.is_work_execution() {
            1
        } else if receipt.step_type.is_verification() {
            2
        } else {
            0
        };
        self.window.push_back(kind);
        if self.window.len() > ACTOR_FQ_WINDOW {
            if let Some(aged_out) = self.window.pop_front() {
                match aged_out {
                    1 => self.execute_count = self.execute_count.saturating_sub(1),
                    2 => self.verify_count = self.verify_count.saturating_sub(1),
                    _ => {}
                }
            }
        }
        match kind {
            1 => {
                self.execute_count += 1;
                self.execute_cost_ns = self.execute_cost_ns.saturating_add(receipt.cost_ns);
                self.consecutive_executes_without_verify += 1;
            }
            2 => {
                self.verify_count += 1;
                self.verify_cost_ns = self.verify_cost_ns.saturating_add(receipt.cost_ns);
                self.consecutive_executes_without_verify = 0;
                // P3 (2026-10-15): track mechanical dual-emissions separately.
                // Measure-first: still counted in verify_count above; the split is
                // exposed so the enforcement change can be made on data, not vibes.
                if receipt.mechanical_self_verify {
                    self.mechanical_verify_count += 1;
                }
            }
            _ => {}
        }
        // ── FQ dual-formula fix (audit 2026-08-10) ──
        // Legacy cost-based ratio (execute_cost_ns / verify_cost_ns) is DEPRECATED.
        // It diverged from v2.1 semantics and miscalibrated HOLD/THROTTLE gates
        // relative to /health which reports v2.1 (verify_count / execute_count).
        // We now alias `self.fq` to the v2.1 count-based quotient so enforcement
        // T1-2 (audit 2026-08-10): FQ canonicalization.
        // `self.quotient` is the SINGLE CANONICAL field for governance decisions.
        // `self.fq` is a DEPRECATED alias — kept only for serialized backward compat.
        // All enforcement and health reporting MUST use `self.quotient`.
        // Do NOT read `self.fq` for governance — it is a mirror, not a source.
        //
        // Canonical semantics: quotient = verify_count / execute_count — HIGHER = healthier.
        // Thresholds: stuck < 0.5, overheat > 10.0 (inverted to 1/overheat for v2.1 direction).

        // v2.1 quotient: verify_count / execute_count (inverted, count-based)
        self.quotient = if self.verify_count == 0 || self.execute_count == 0 {
            None
        } else {
            Some(self.verify_count as f64 / self.execute_count as f64)
        };

        // Alias: fq mirrors quotient for back-compat. None → 0.0 (no measurement yet).
        self.fq = self.quotient.unwrap_or(0.0);

        // v2.2 verdict: seven-state band per Arif F13 spec + Helix Codex Lock 2
        // Both ratio poles are Calhoun sink:
        //   verify:execute > 3:1 → FOSSILIZED (contact, no motion)
        //   execute:verify > 3:1 → BURNING (motion, no witness)
        self.verdict = if self.verify_count == 0 && self.execute_count == 0 {
            FlowVerdict::Unknown
        } else if self.verify_count == 0 {
            FlowVerdict::Unknown
        } else if self.verify_count < 2 {
            FlowVerdict::Caution
        } else if self.execute_count == 0 {
            FlowVerdict::Flowing
        } else if let Some(q) = self.quotient {
            if q > 3.0 {
                // Helix Codex Lock 2: fossilisation pole
                FlowVerdict::Fossilized
            } else if q >= 1.0 {
                FlowVerdict::Optimal
            } else if q >= 0.5 {
                FlowVerdict::Flowing
            } else if q >= 0.1 {
                FlowVerdict::Stuck
            } else {
                // Helix Codex Lock 2: burn pole
                FlowVerdict::Burning
            }
        } else {
            FlowVerdict::Unknown
        };
    }
}

// ── Throttle / HOLD State ────────────────────────────────────────────────

/// What action the enforcer takes on a violating actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnforcerAction {
    /// No restriction — execute freely.
    Allow,
    /// Throttle — reduce execution rate (max 1 per cooldown window).
    Throttle,
    /// HOLD — block all execution until verification happens.
    Hold,
    /// VOID — permanently block. Requires F13 override.
    Void,
}

// ── FQ Threshold Config ──────────────────────────────────────────────────

/// Configuration for FQ-based invariant enforcement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FqThresholds {
    /// FQ below this → STUCK → HOLD
    pub stuck_threshold: f64,
    /// FQ above this → OVERHEAT → THROTTLE
    pub overheat_threshold: f64,
    /// FQ above this → FOSSILIZED → HOLD (Helix Codex Lock 2: fossilisation pole)
    pub fossilisation_threshold: f64,
    /// Max consecutive executes without verify before THROTTLE
    pub max_consecutive_executes: u64,
    /// Cooldown window in seconds (throttled actors wait this long between executes)
    pub throttle_cooldown_s: u64,
    /// How often the enforcement loop runs (seconds)
    pub enforcement_interval_s: u64,
}

impl Default for FqThresholds {
    fn default() -> Self {
        Self {
            stuck_threshold: 0.5,
            overheat_threshold: 10.0,
            fossilisation_threshold: 3.0,
            max_consecutive_executes: 5,
            throttle_cooldown_s: 30,
            enforcement_interval_s: 10,
        }
    }
}

// ── Invariant Enforcer ───────────────────────────────────────────────────

/// The invariant enforcement engine.
///
/// Runs alongside the arifFlow daemon. Every enforcement_interval_s:
///   1. Scans all tracked actors' FQ
///   2. Checks each F0-F6 invariant
///   3. Applies throttle/HOLD/VOID actions
///   4. Writes enforcement receipts
///   5. Emits cooling entries for violations
pub struct InvariantEnforcer {
    /// Per-actor flow state
    pub actors: BTreeMap<String, ActorFlowState>,
    /// FQ threshold configuration
    pub thresholds: FqThresholds,
    /// Cooling ledger for drift tracking
    pub cooling_ledger: CoolingLedger,
    /// Receipt store for enforcement receipts
    pub receipt_store: ReceiptStore,
    /// Last enforcement time
    pub last_enforcement: Instant,
    /// Count of enforcement cycles
    pub cycle_count: u64,
    /// Count of HOLD signals emitted
    pub hold_count: u64,
    /// Count of THROTTLE signals emitted
    pub throttle_count: u64,
    /// CM-1 (2026-09-19): execution-class receipts REFUSED by the
    /// explanatory-class gate (F2/F3). arifFlow reports the refusal; it does
    /// not classify the claim (that is claim_kernel authority, on disk at
    /// /root/AAA/lib/claim_kernel/claim_kernel.py) and it does not hold the
    /// actor for it. Surfaced by the daemon status surface reported in
    /// /root/arifFlow/src/main.rs.
    pub explanation_refusals: u64,
    /// Most recent explanatory-class refusal, for the daemon status surface
    /// and the enforcement report (see /root/arifFlow/src/main.rs).
    pub last_explanation_violation: Option<InvariantCheck>,
    /// Refusals recorded since the last enforcement cycle — surfaced in the
    /// next report, then drained.
    pub pending_explanation_violations: Vec<InvariantCheck>,
    /// [F-4 2026-10-25] Validated classification declarations. Consulted ONCE
    /// per actor, at registration (first ingest), to stamp `actor_class`.
    /// Nothing on the wire can write to this registry; a receipt's actor_id
    /// only selects a key, it never grants a class by itself.
    pub class_registry: ClassRegistry,
}

/// Shared F3 hold derivation — the ONE place the three Calhoun poles are
/// evaluated. Consumed by both `enforce()` (report) and `check_actor()` (gate),
/// so a report and a gate cannot disagree about the same actor in the same
/// instant. Returns (reason, detail) when the actor is held, None when clear.
///
/// [A 2026-10-20] Before this, enforce() derived from live counters while
/// check_actor() trusted a sticky `held` flag it never re-derived — measured
/// live as /enforce "not blocking" vs /check "HOLD, 403" for fi-002.
fn f3_hold_condition(
    actor_id: &str,
    state: &ActorFlowState,
    thresholds: &FqThresholds,
) -> Option<(String, String)> {
    // FQ gate: the policy floor (stuck_threshold, 0.5) is the binding gate;
    // the risk-class floor can only raise it, never lower it. Canonical
    // quantity is `quotient` (count-based); `fq` is the deprecated alias.
    let fq_value = state.quotient.unwrap_or(0.0);
    let fq_required = state
        .last_risk_class
        .fq_required()
        .max(thresholds.stuck_threshold);

    // STUCK — below the floor.
    if fq_value < fq_required && state.execute_count > 0 {
        return Some((
            format!(
                "Actor '{}' is STUCK: FQ={:.2} < fq_required={:.2} (risk_class={}). execute={} verify={}",
                actor_id,
                fq_value,
                fq_required,
                state.last_risk_class.code(),
                state.execute_count,
                state.verify_count
            ),
            format!(
                "FQ={:.2} execute_count={} verify_count={} consecutive_without_verify={}",
                fq_value, state.execute_count, state.verify_count,
                state.consecutive_executes_without_verify
            ),
        ));
    }

    // BURN pole — execute:verify > 10:1, motion without witness.
    // v2.2: both Calhoun poles are equal-weight holds (Codex requirement).
    if state.fq < (1.0 / thresholds.overheat_threshold) && state.execute_count > 0 {
        return Some((
            format!(
                "Actor '{}' is BURNING: FQ={:.2} (execute:verify > 10:1). Motion without witness.",
                actor_id, state.fq
            ),
            format!(
                "FQ={:.2} execute_count={} verify_count={} — Helix Codex Lock 2 burn pole",
                state.fq, state.execute_count, state.verify_count
            ),
        ));
    }

    // FOSSILISATION pole — verify:execute > 3:1, contact exists, nothing moves.
    if let Some(q) = state.quotient
        && q > thresholds.fossilisation_threshold
        && state.execute_count > 0
    {
        // [F13 2026-10-26 — option C, ratified by Arif] VERIFICATION-NATIVE ROLE
        // EXEMPTION. A role whose assignment IS verification cannot be
        // "fossilised" for verifying: the pole would punish the job it was given
        // (measured case: grok-build archived 3271 Verify / 38 Execute across
        // months — 86:1 is its nature, not a stall).
        //
        // Scope of the exemption — deliberately narrow:
        //   * the HOLD is suppressed; the FOSSILIZED label still prints (it is a
        //     true statement about the window) and the actor still counts toward
        //     the federation aggregate;
        //   * declaration is explicit and auditable, never a silent
        //     reclassification (entity_class.rs §register: first declaration wins):
        //         ARIFLOW_VERIFY_NATIVE_ACTORS="grok-build"
        //   * an executor is not eligible: claude-fi-002 carries 1527 lifetime
        //     Execute receipts, so declaring it verification-native would be
        //     misclassification, not relief.
        let verify_native = std::env::var("ARIFLOW_VERIFY_NATIVE_ACTORS")
            .map(|v| {
                v.split(',')
                    .map(|t| t.trim())
                    .any(|t| !t.is_empty() && t.eq_ignore_ascii_case(actor_id))
            })
            .unwrap_or(false);
        if verify_native {
            return None;
        }
        return Some((
            format!(
                "Actor '{}' is FOSSILIZED: FQ={:.2} > {:.2} (verify:execute > 3:1). Contact exists, nothing moves.",
                actor_id, q, thresholds.fossilisation_threshold
            ),
            format!(
                "FQ={:.2} execute_count={} verify_count={} — Helix Codex Lock 2 fossilisation pole",
                q, state.execute_count, state.verify_count
            ),
        ));
    }

    None
}

/// Federation FQ aggregate — Minimum-Gated Harmonic Mean (Tugas 2.2 / Gate 2):
/// if any active executing actor has FQ < 0.50, the aggregate is gated to
/// min_i(FQ_i); otherwise the harmonic mean over all active executing actors.
///
/// [F-4 2026-10-25] The ONE place this constitutional aggregate is computed.
/// Exclusion is by validated `actor_class == Fixture` ONLY — a name never
/// exempts. An actor with no validated classification counts (fail-closed).
/// An actor is "active" exactly as before: execute_count > 0 and a measured
/// quotient (the pre-existing window semantics are preserved, not loosened).
///
/// Returns `None` when no actor qualifies (the caller falls back to the pool
/// quotient, preserving the historical shape).
pub fn federation_fq_aggregate(actors: &BTreeMap<String, ActorFlowState>) -> Option<f64> {
    let active_fqs: Vec<f64> = actors
        .iter()
        .filter(|(_, state)| {
            state.actor_class.counts_toward_federation_aggregate() && state.execute_count > 0
        })
        .filter_map(|(_, state)| state.quotient)
        .collect();
    let &min_fq = active_fqs
        .iter()
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))?;
    if min_fq < 0.50 {
        Some(min_fq)
    } else {
        let sum_inv: f64 = active_fqs.iter().map(|&q| 1.0 / q.max(0.001)).sum();
        Some(active_fqs.len() as f64 / sum_inv)
    }
}

impl InvariantEnforcer {
    pub fn new(thresholds: FqThresholds) -> Self {
        Self {
            actors: BTreeMap::new(),
            thresholds,
            cooling_ledger: CoolingLedger::default(),
            receipt_store: ReceiptStore::default(),
            last_enforcement: Instant::now(),
            cycle_count: 0,
            hold_count: 0,
            throttle_count: 0,
            explanation_refusals: 0,
            last_explanation_violation: None,
            pending_explanation_violations: Vec::new(),
            class_registry: ClassRegistry::default(),
        }
    }

    /// [F-4 2026-10-25] Install the validated classification registry.
    /// Call this BEFORE seeding/ingest so every actor state is stamped at
    /// registration. Returns the rejection log from the registry so the
    /// caller can surface it (rejected declarations never exempt anyone).
    pub fn set_class_registry(&mut self, registry: ClassRegistry) -> Vec<String> {
        let rejects = registry.rejects().to_vec();
        self.class_registry = registry;
        rejects
    }

    /// CM-1 (2026-09-19) — record a REFUSED execution-class receipt as a
    /// flow-plane violation under the named code
    /// `F3_EXPLANATION_CLASS_INELIGIBLE` (F2 + F3).
    ///
    /// The actor is deliberately NOT held here. Holding an actor because of
    /// the class of its justification would be judging the claim — exactly
    /// what F2 forbids. The refusal IS the whole enforcement: the receipt does
    /// not transit. Classification authority stays with the claim_kernel.
    pub fn note_explanation_refusal(
        &mut self,
        receipt: &FlowReceipt,
        gate: &ExplanationGate,
    ) -> InvariantCheck {
        let class = gate
            .explanation_class
            .unwrap_or(ExplanationClass::Unclassified);
        let code = gate
            .code
            .clone()
            .unwrap_or_else(|| FLOW_CODE_EXPLANATION_CLASS_INELIGIBLE.to_string());
        let schema = gate.schema.clone().unwrap_or_else(|| "none".to_string());

        // NOTE: the reason string intentionally avoids the `Actor '<id>'`
        // token used by Step 3 of `enforce()` — refusals must not be
        // converted into actor HOLDs (F2: checkpoint, never judge).
        let check = InvariantCheck::new(
            FlowInvariant::F3_ObserveNeverInterpret,
            InvariantStatus::Hold,
            format!(
                "{code}: receipt {} actor={} step={} class={} schema={} — execution-class \
                 justification is not action-eligible; transmission refused",
                receipt.receipt_id, receipt.actor_id, receipt.step_type, class, schema
            ),
            format!(
                "gate=REFUSED invariant={} verdict_owner=claim_kernel arifFlow=transmit-only \
                 step_type={} receipt={}",
                FLOW_INVARIANT_EXPLANATION_CLASS, receipt.step_type, receipt.receipt_id
            ),
        );

        self.explanation_refusals += 1;
        self.last_explanation_violation = Some(check.clone());
        self.pending_explanation_violations.push(check.clone());

        self.cooling_ledger.record(CoolingEntry::new(
            self.cycle_count,
            format!("REFUSED {code}: {class} justification on execution-class step"),
            format!(
                "arifFlow refused to transmit receipt {} from actor {}",
                receipt.receipt_id, receipt.actor_id
            ),
            Convergence::Diverging,
            DriftSeverity::High,
            "arifFlow/invariants",
        ));

        check
    }

    /// Ingest a receipt and update actor state.
    pub fn ingest(&mut self, receipt: &FlowReceipt) {
        // RED-002: canonicalize actor identity — case-variant IDs are ONE actor
        // ("A-FORGE" ≡ "a-forge"). Original case is preserved on the receipt;
        // the FQ state key is canonical.
        //
        // [B3/A3 identity-canon 2026-10-22] Also collapse a lane-qualified handle
        // ("kimi-code/FI-008") onto an existing canonical key ("fi-008") so one
        // principal does not accrue two FQ states. Collapse-to-existing only —
        // a first-seen handle is never re-keyed on a guess.
        let raw = receipt.actor_id.to_ascii_lowercase();
        let actor_id = self.resolve_actor_key(&raw).unwrap_or(raw);
        // [F-4 2026-10-25] Stamp class at REGISTRATION (first ingest) from the
        // validated registry — the receipt payload never sets it.
        let actor_class = self.class_registry.classify(&actor_id);
        let entry = self
            .actors
            .entry(actor_id.clone())
            .or_insert_with(|| {
                let mut state = ActorFlowState::new(actor_id);
                state.actor_class = actor_class;
                state
            });
        entry.ingest(receipt);
    }

    /// Run the full enforcement cycle. Returns the invariant report.
    pub fn enforce(&mut self) -> InvariantReport {
        self.cycle_count += 1;
        let now = Instant::now();

        // ── Step 1: FQ Gate ──
        // Check every actor's FQ against thresholds.
        let mut checks = Vec::new();

        for (actor_id, state) in &self.actors {
            // [A 2026-10-20] F3 poles come from the shared derivation below —
            // the same one check_actor() uses. Before this, enforce() derived
            // from live counters while check_actor() trusted a sticky `held`
            // flag, so /enforce and /check could report opposite verdicts for
            // the same actor in the same instant (measured live: fi-002
            // allowed by /enforce, 403 by /check).
            if let Some((reason, detail)) = f3_hold_condition(actor_id, state, &self.thresholds) {
                checks.push(InvariantCheck::new(
                    FlowInvariant::F3_ObserveNeverInterpret,
                    InvariantStatus::Hold,
                    reason,
                    detail,
                ));
            }

            // Consecutive executes without verify → THROTTLE → HOLD
            if state.consecutive_executes_without_verify > self.thresholds.max_consecutive_executes
            {
                checks.push(InvariantCheck::new(
                    FlowInvariant::F1_ScheduleNeverAuthorize,
                    InvariantStatus::Hold,
                    format!(
                        "Actor '{}' has {} consecutive executes without verification (max: {})",
                        actor_id,
                        state.consecutive_executes_without_verify,
                        self.thresholds.max_consecutive_executes
                    ),
                    format!(
                        "consecutive_executes_without_verify={}",
                        state.consecutive_executes_without_verify
                    ),
                ));
            }
        }

        // ── Step 2: Structural invariants (F0, F2, F4, F5, F6) ──
        // These are always PASS unless the system is misconfigured.
        // F0: arifFlow never originates intent — checked by architecture (no intent API)
        checks.push(InvariantCheck::new(
            FlowInvariant::F0_TransmitNeverOwn,
            InvariantStatus::Pass,
            "Architecture: arifFlow has no intent-origination surface",
            "No POST /intent endpoint exists",
        ));

        // F2: arifFlow never judges — checked by architecture (no verdict API)
        checks.push(InvariantCheck::new(
            FlowInvariant::F2_CheckpointNeverJudge,
            InvariantStatus::Pass,
            "Architecture: arifFlow has no verdict-generation surface",
            "Verdict grammar belongs to arifOS :8088",
        ));

        // F4: arifFlow never executes — checked by capability boundary
        checks.push(InvariantCheck::new(
            FlowInvariant::F4_RouteNeverExecute,
            InvariantStatus::Pass,
            "Architecture: arifFlow has no execution surface",
            "Execution belongs to A-FORGE :7071",
        ));

        // F5: arifFlow never owns memory — checked by VAULT999 boundary
        checks.push(InvariantCheck::new(
            FlowInvariant::F5_ReceiptNeverOwn,
            InvariantStatus::Pass,
            "Architecture: arifFlow writes receipts, VAULT999 owns memory",
            "VAULT999 authority: ARIFFAZIL/arifOS",
        ));

        // F6: arifFlow never collapses organs — checked by boundary
        checks.push(InvariantCheck::new(
            FlowInvariant::F6_ConnectNeverCollapse,
            InvariantStatus::Pass,
            "Architecture: arifFlow schedules organs, never merges them",
            "Organs are independent: GEOX, WEALTH, WELL, HERMES",
        ));

        // ── Step 2b: Explanation-class refusals since the last cycle (CM-1) ──
        // Reported, never used to hold the actor: these checks carry no
        // `Actor '<id>'` token, so Step 3 cannot convert them into HOLDs
        // (F2: arifFlow checkpoints, it does not judge the claim).
        if !self.pending_explanation_violations.is_empty() {
            let drained: Vec<InvariantCheck> =
                std::mem::take(&mut self.pending_explanation_violations);
            checks.extend(drained);
        }

        // ── Step 3: Apply actions ──
        // Update actor throttle/hold state based on checks.
        for check in &checks {
            if check.status.is_blocking() {
                // Find the actor and apply HOLD
                // The actor_id is embedded in the reason string — extract it
                if let Some(actor_start) = check.reason.find("Actor '") {
                    let rest = &check.reason[actor_start + 7..];
                    if let Some(actor_end) = rest.find('\'') {
                        let actor_id = &rest[..actor_end];
                        if let Some(state) = self.actors.get_mut(actor_id)
                            && check.status == InvariantStatus::Hold
                        {
                            state.held = true;
                            state.throttled = true;
                            // [F13 2026-10-20] Stamp first-application time so the hold
                            // can be persisted durably and survive a restart instead of
                            // being silently re-derived away. Only set on first hold.
                            if state.held_since_ns == 0 {
                                state.held_since_ns = now_ns();
                            }
                            self.hold_count += 1;

                            // Record cooling entry for HOLD
                            self.cooling_ledger.record(CoolingEntry::new(
                                self.cycle_count,
                                format!("HOLD: {}", check.reason),
                                format!("Actor {} held by invariant enforcement", actor_id),
                                Convergence::Diverging,
                                DriftSeverity::Critical,
                                "arifFlow/invariants",
                            ));
                        }
                    }
                }
            }
            if check.status == InvariantStatus::Warn
                && let Some(actor_start) = check.reason.find("Actor '")
            {
                let rest = &check.reason[actor_start + 7..];
                if let Some(actor_end) = rest.find('\'') {
                    let actor_id = &rest[..actor_end];
                    if let Some(state) = self.actors.get_mut(actor_id) {
                        state.throttled = true;
                        self.throttle_count += 1;
                    }
                }
            }
        }

        // ── Step 4: Compute FQ ──
        let fq = self.receipt_store.flow_quotient(100);

        self.last_enforcement = now;

        InvariantReport::new(checks, Some(fq))
    }

    /// Resolve a raw actor handle to the registered canonical key.
    ///
    /// [B3/A3 identity-canon 2026-10-22] A caller may present a lane-qualified
    /// handle (`kimi-code/FI-008`) while the actor registry is keyed by the
    /// canonical id (`fi-008`). Raw-string lookup then misses and the same
    /// principal is judged on a phantom state. Resolution order mirrors
    /// `contracts/identity_canon.py` comparison_key: exact, then separators
    /// stripped (`- _ space .`), then slash head/tail for `name/FI-nnn` forms.
    fn resolve_actor_key(&self, actor_id: &str) -> Option<String> {
        if self.actors.contains_key(actor_id) {
            return Some(actor_id.to_string());
        }
        fn key_norm(s: &str) -> String {
            s.to_lowercase()
                .chars()
                .filter(|c| !matches!(c, '-' | '_' | ' ' | '.'))
                .collect()
        }
        let n = key_norm(actor_id);
        if !n.is_empty()
            && let Some(k) = self.actors.keys().find(|k| key_norm(k) == n)
        {
            return Some(k.clone());
        }
        if let Some((head, tail)) = actor_id.split_once('/') {
            for part in [head.trim(), tail.trim()] {
                let np = key_norm(part);
                if np.is_empty() {
                    continue;
                }
                if let Some(k) = self.actors.keys().find(|k| key_norm(k) == np) {
                    return Some(k.clone());
                }
            }
        }
        None
    }

    /// Check if a specific actor is allowed to execute.
    /// Returns (allowed, reason, action).
    pub fn check_actor(&self, actor_id: &str) -> (bool, String, EnforcerAction) {
        // Resolve the raw handle to the registered canonical key before lookup.
        let resolved = self.resolve_actor_key(actor_id);
        let lookup = resolved.as_deref().unwrap_or(actor_id);
        if let Some(state) = self.actors.get(lookup) {
            // [A 2026-10-20] RE-DERIVE from live counters first — the sticky
            // `held` flag is a latch, not the source of truth. This is the
            // check_actor-side half of the shared-derivation fix: the same
            // f3_hold_condition() that drives enforce() answers here, so
            // /check and /enforce cannot report opposite verdicts for one actor.
            if let Some((reason, _detail)) =
                f3_hold_condition(actor_id, state, &self.thresholds)
            {
                return (false, reason, EnforcerAction::Hold);
            }
            if state.held {
                return (
                    false,
                    format!("Actor '{}' is HELD — verify before executing", actor_id),
                    EnforcerAction::Hold,
                );
            }
            if state.throttled {
                let elapsed = self.last_enforcement.elapsed().as_secs();
                if elapsed < self.thresholds.throttle_cooldown_s {
                    return (
                        false,
                        format!(
                            "Actor '{}' is THROTTLED — wait {}s (elapsed: {}s)",
                            actor_id, self.thresholds.throttle_cooldown_s, elapsed
                        ),
                        EnforcerAction::Throttle,
                    );
                }
            }
            if state.consecutive_executes_without_verify >= self.thresholds.max_consecutive_executes
            {
                return (
                    false,
                    format!(
                        "Actor '{}' needs verification: {} consecutive executes without verify",
                        actor_id, state.consecutive_executes_without_verify
                    ),
                    EnforcerAction::Hold,
                );
            }
        }
        (true, "OK".to_string(), EnforcerAction::Allow)
    }

    /// Get all actors that are currently held or throttled.
    pub fn restricted_actors(&self) -> Vec<(&str, EnforcerAction, String)> {
        let mut result = Vec::new();
        for (id, state) in &self.actors {
            // F13 2026-10-20: `state.fq` is an alias that maps `None → 0.0`, so an actor
            // with no measured quotient in the window (either side zero) reported
            // "HELD: FQ=0.00" — the same string as an actor that executed and verified
            // nothing. Measured case: `chron` held at "FQ=0.00" while its own window read
            // 2 Execute / 10 Verify (FQ=5.00, the opposite pole). The reason must say what
            // was actually measured; the hold itself is unchanged.
            let reason = match state.quotient {
                Some(q) => format!(
                    "HELD: FQ={:.2} (execute={} verify={})",
                    q, state.execute_count, state.verify_count
                ),
                None => format!(
                    "HELD: FQ=UNMEASURED — window has execute={} verify={}; a quotient needs both sides",
                    state.execute_count, state.verify_count
                ),
            };
            if state.held {
                result.push((id.as_str(), EnforcerAction::Hold, reason));
            } else if state.throttled {
                result.push((
                    id.as_str(),
                    EnforcerAction::Throttle,
                    reason.replacen("HELD:", "THROTTLED:", 1),
                ));
            }
        }
        result
    }

    /// Release a hold on an actor (called after verification receipt).
    pub fn release_hold(&mut self, actor_id: &str) {
        if let Some(state) = self.actors.get_mut(actor_id) {
            state.held = false;
            state.throttled = false;
            state.consecutive_executes_without_verify = 0;
            // [F13 2026-10-20] Cleared with the hold — a released hold must not
            // resurrect on the next boot from a stale timestamp.
            state.held_since_ns = 0;
        }
    }

    // ── F13 2026-10-20: durable hold ledger ────────────────────────────
    //
    // A hold is a constitutional verdict, not a derived statistic. Before this
    // it existed only in memory: on boot the daemon re-ingested receipts to seed
    // per-actor FQ, which recomputed `held` from CURRENT counters. An actor that
    // was no longer over threshold therefore came back un-held after every
    // restart, deploy or crash — with no audit entry. Observed live on
    // 2026-10-20: `fi-002` lost its hold across a deploy, 8 → 7 holds, no
    // RELEASED record anywhere.
    //
    // These two methods make the verdict durable. Persist/restore is deliberately
    // SEPARATE from enforce()/release_hold() so the enforcer stays a pure
    // in-memory state machine and the daemon owns all file IO.

    /// Snapshot every currently-held actor for durable persistence.
    pub fn held_snapshot(&self) -> Vec<HoldRecord> {
        self.actors
            .values()
            .filter(|s| s.held)
            .map(|s| HoldRecord {
                actor_id: s.actor_id.clone(),
                held_since_ns: s.held_since_ns,
            })
            .collect()
    }

    /// Re-apply persisted holds after boot seeding.
    ///
    /// Restored holds are returned so the caller can WITNESS each one — a hold
    /// that is restored rather than re-derived must be visible in the audit
    /// trail, otherwise durability silently resurrects ghosts with no witness.
    ///
    /// ## Evidence test (2026-10-26) — HOLD_LATCH_OUTLIVES_EVIDENCE
    ///
    /// A hold is re-applied ONLY when the actor exists in the state seeded from
    /// the live receipt window. An actor with zero receipts cannot justify a
    /// hold, and — worse — cannot clear one: `/check` blocks its execution, so
    /// it never runs, so it never emits the session-backed Verify that the
    /// auto-release path requires. A block no legitimate action can lift is a
    /// permanent BAN, and F2 says arifFlow checkpoints, never judges. Those
    /// records are returned as `unsupported`; the caller witnesses the drop.
    ///
    /// Measured case (2026-10-26): `anonymous`, `codex`, `fi-008-cron` and
    /// `grok-build` were resurrected as shell actors on every boot while holding
    /// 0 receipts between them — `grok-build`'s 6,577 receipts (including Verify
    /// entries) had been archived out of the live ledger on 2026-09-14.
    pub fn restore_holds(&mut self, records: &[HoldRecord]) -> RestoreOutcome {
        let mut outcome = RestoreOutcome::default();
        for rec in records {
            // Canonicalize: seeding canonicalizes actor ids to lowercase, so a
            // persisted mixed-case id must fold to the same key.
            let key = rec.actor_id.to_ascii_lowercase();

            // Evidence test — no receipts in the live window means no hold.
            if !self.actors.contains_key(&key) {
                outcome.unsupported.push(rec.actor_id.clone());
                continue;
            }

            let already = self
                .actors
                .get(&key)
                .map(|s| s.held)
                .unwrap_or(false);
            if already {
                // Already held from re-derivation — keep the EARLIER timestamp
                // (it is the true first-application time).
                if let Some(state) = self.actors.get_mut(&key)
                    && state.held_since_ns == 0
                {
                    state.held_since_ns = rec.held_since_ns;
                }
                continue;
            }
            // Live evidence exists and re-derivation left the actor un-held →
            // re-apply the durable verdict on top of the re-derived state.
            // [F-4 2026-10-25] The state keeps the same registration-time class
            // stamp as an ingested actor (fail-closed when undeclared).
            let shell_class = self.class_registry.classify(&key);
            let entry = self
                .actors
                .entry(key)
                .or_insert_with(|| {
                    let mut state = ActorFlowState::new(rec.actor_id.clone());
                    state.actor_class = shell_class;
                    state
                });
            entry.held = true;
            entry.throttled = true;
            entry.held_since_ns = if rec.held_since_ns != 0 {
                rec.held_since_ns
            } else {
                now_ns()
            };
            outcome.restored.push(rec.actor_id.clone());
        }
        outcome
    }
}

/// Outcome of re-applying the durable hold ledger after boot seeding.
///
/// `restored`  — holds re-applied on top of a state that exists in the live
///               receipt window (the actor is real and still under floor).
/// `unsupported` — hold records dropped because the actor has ZERO receipts in
///               the live window. The evidence that produced the hold is gone,
///               so the hold has no basis. See `restore_holds` evidence test.
#[derive(Debug, Default, Clone)]
pub struct RestoreOutcome {
    pub restored: Vec<String>,
    pub unsupported: Vec<String>,
}

/// A durable record of one held actor.
///
/// [F13 2026-10-20] A hold is a constitutional verdict with a lifetime; it is
/// not a function of the current receipt window. This is the on-disk form that
/// makes that true across restarts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldRecord {
    pub actor_id: String,
    /// When the hold was first applied (wall-clock ns). 0 = unknown.
    pub held_since_ns: i64,
}

impl Default for InvariantEnforcer {
    fn default() -> Self {
        Self::new(FqThresholds::default())
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receipt::{EpistemicLabel, FlowReceipt, StepType};

    #[test]
    fn test_all_invariants_defined() {
        let all = FlowInvariant::all();
        assert_eq!(all.len(), 7);
        assert_eq!(all[0].code(), "F0");
        assert_eq!(all[6].code(), "F6");
    }

    #[test]
    fn test_invariant_names() {
        let f0 = FlowInvariant::F0_TransmitNeverOwn;
        assert!(f0.name().contains("F0"));
        assert!(f0.name().contains("transmits"));
    }

    #[test]
    fn test_actor_ingest_updates_fq() {
        let mut state = ActorFlowState::new("test-agent");
        assert_eq!(state.fq, 0.0);

        let r1 = FlowReceipt::new_first(
            "test-agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            1_000_000,
        );
        state.ingest(&r1);
        assert_eq!(state.execute_count, 1);
        assert_eq!(state.verify_count, 0);
        assert_eq!(state.consecutive_executes_without_verify, 1);
        // v2.1: 1 exec, 0 verify → quotient None → fq aliased to 0.0
        assert_eq!(state.fq, 0.0);

        let r2 = FlowReceipt::new_first(
            "test-agent",
            "s1",
            StepType::Verify,
            EpistemicLabel::Derivation,
            500_000,
        );
        state.ingest(&r2);
        assert_eq!(state.verify_count, 1);
        assert_eq!(state.consecutive_executes_without_verify, 0);
        // v2.1: 1 exec + 1 verify → quotient = 1/1 = 1.0 (verify_count < 2 → Caution verdict)
        assert!((state.fq - 1.0).abs() < 0.01);
        assert_eq!(state.verdict, FlowVerdict::Caution);
    }

    // ── 2026-10-26: a durable hold must not outlive its evidence ─────────
    //
    // Regression class HOLD_LATCH_OUTLIVES_EVIDENCE. Measured live: four actors
    // were resurrected as held shell actors on every boot while holding zero
    // receipts between them. /check blocked them, so they could never Execute,
    // so they could never emit the Verify that releases them — a permanent ban.

    #[test]
    fn restore_holds_drops_hold_for_actor_with_no_live_evidence() {
        let mut enf = InvariantEnforcer::default();
        // No receipts seeded → the actor does not exist in the live window.
        let records = vec![HoldRecord {
            actor_id: "ghost-actor".to_string(),
            held_since_ns: 1_700_000_000_000_000_000,
        }];
        let outcome = enf.restore_holds(&records);
        assert!(
            outcome.restored.is_empty(),
            "a hold with no live evidence must not be restored"
        );
        assert_eq!(outcome.unsupported, vec!["ghost-actor".to_string()]);
        assert!(
            !enf.actors.contains_key("ghost-actor"),
            "no shell actor may be created for a ghost hold"
        );
        assert!(enf.held_snapshot().is_empty());
    }

    #[test]
    fn restore_holds_reapplies_hold_for_actor_with_live_evidence() {
        let mut enf = InvariantEnforcer::default();
        // Live evidence: one Execute, no Verify → under the FQ floor.
        let r = FlowReceipt::new_first(
            "live-actor",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            1_000_000,
        );
        enf.ingest(&r);
        let records = vec![HoldRecord {
            actor_id: "live-actor".to_string(),
            held_since_ns: 1_700_000_000_000_000_000,
        }];
        let outcome = enf.restore_holds(&records);
        assert_eq!(outcome.restored, vec!["live-actor".to_string()]);
        assert!(outcome.unsupported.is_empty());
        let state = enf.actors.get("live-actor").expect("actor must exist");
        assert!(state.held, "hold must be re-applied on top of live evidence");
        assert_eq!(
            state.held_since_ns, 1_700_000_000_000_000_000,
            "first-application time must survive the restore"
        );
    }

    #[test]
    fn test_stuck_detection() {
        let mut state = ActorFlowState::new("stuck-agent");
        // Lots of verify, little execute
        let r1 = FlowReceipt::new_first(
            "stuck-agent",
            "s1",
            StepType::Verify,
            EpistemicLabel::Derivation,
            1_000_000,
        );
        state.ingest(&r1);
        let r2 = FlowReceipt::new_first(
            "stuck-agent",
            "s1",
            StepType::Verify,
            EpistemicLabel::Derivation,
            1_000_000,
        );
        state.ingest(&r2);
        let r3 = FlowReceipt::new_first(
            "stuck-agent",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            100_000,
        );
        state.ingest(&r3);
        // v2.1: 1 exec + 2 verify → quotient = 2/1 = 2.0 → Optimal (verify leads execution)
        assert!(state.fq > 1.0);
        assert_eq!(state.verdict, FlowVerdict::Optimal);
    }

    #[test]
    fn test_enforcer_ingest_and_check() {
        let mut enforcer = InvariantEnforcer::default();

        // Simulate a stuck actor: 5 executes, 0 verifies
        for _i in 0..6 {
            let r = FlowReceipt::new_first(
                "bad-actor",
                "s1",
                StepType::Execute,
                EpistemicLabel::Observation,
                1_000_000,
            );
            enforcer.ingest(&r);
        }

        let report = enforcer.enforce();

        // Should have at least one HOLD check
        let holds: Vec<_> = report
            .checks
            .iter()
            .filter(|c| c.status.is_blocking())
            .collect();
        assert!(
            !holds.is_empty(),
            "Expected at least one HOLD for stuck actor"
        );

        // Check that bad-actor is blocked
        let (allowed, reason, action) = enforcer.check_actor("bad-actor");
        assert!(!allowed, "Stuck actor should be blocked: {}", reason);
        assert_eq!(action, EnforcerAction::Hold);
    }

    #[test]
    fn test_enforcer_allows_healthy_actor() {
        let mut enforcer = InvariantEnforcer::default();

        // Simulate balanced actor: 1 execute, 1 verify
        let r1 = FlowReceipt::new_first(
            "healthy",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            1_000_000,
        );
        enforcer.ingest(&r1);
        let r2 = FlowReceipt::new_first(
            "healthy",
            "s1",
            StepType::Verify,
            EpistemicLabel::Derivation,
            500_000,
        );
        enforcer.ingest(&r2);

        let _report = enforcer.enforce();
        let (allowed, reason, action) = enforcer.check_actor("healthy");
        assert!(allowed, "Healthy actor should be allowed: {}", reason);
        assert_eq!(action, EnforcerAction::Allow);
    }

    #[test]
    fn test_check_actor_resolves_lane_qualified_handle() {
        // B3/A3 identity-canon (2026-10-22): a lane-qualified handle
        // ("kimi-code/FI-008") must resolve to the canonical registered key
        // ("fi-008") — one principal, one key. Before the resolver, the raw
        // lookup missed and the actor was judged on a phantom state.
        let mut enforcer = InvariantEnforcer::default();
        for (i, st) in [(1u64, StepType::Execute), (2u64, StepType::Verify)] {
            let r = FlowReceipt::new_first(
                "fi-008",
                &format!("s{}", i),
                st,
                EpistemicLabel::Observation,
                1_000_000,
            );
            enforcer.ingest(&r);
        }
        let canonical = enforcer.check_actor("fi-008");
        let qualified = enforcer.check_actor("kimi-code/FI-008");
        assert_eq!(
            canonical.0, qualified.0,
            "lane-qualified handle must resolve to the same principal state"
        );
        assert!(
            qualified.0,
            "balanced fi-008 seen through a lane-qualified handle must be allowed: {}",
            qualified.1
        );
    }

    #[test]
    fn test_ingest_collapses_lane_qualified_handle() {
        // B3/A3 identity-canon (2026-10-22): two handles for one principal must
        // accrue ONE FQ state. Execute via "fi-008", verify via
        // "kimi-code/FI-008" — if they collapse, the principal is balanced.
        let mut enforcer = InvariantEnforcer::default();
        let r1 = FlowReceipt::new_first(
            "fi-008",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            1_000_000,
        );
        enforcer.ingest(&r1);
        let r2 = FlowReceipt::new_first(
            "kimi-code/FI-008",
            "s2",
            StepType::Verify,
            EpistemicLabel::Derivation,
            500_000,
        );
        enforcer.ingest(&r2);
        let (allowed, reason, _) = enforcer.check_actor("fi-008");
        assert!(
            allowed,
            "one principal must be balanced after exec+verify across two handles: {}",
            reason
        );
    }

    #[test]
    fn test_release_hold() {
        let mut enforcer = InvariantEnforcer::default();

        // Make actor stuck: 6 executes, 0 verifies → quotient 0.0 < 0.5 floor.
        for _ in 0..6 {
            let r = FlowReceipt::new_first(
                "test",
                "s1",
                StepType::Execute,
                EpistemicLabel::Observation,
                1_000_000,
            );
            enforcer.ingest(&r);
        }
        enforcer.enforce();

        let (allowed, _, _) = enforcer.check_actor("test");
        assert!(!allowed);

        // [6f7ee12 doctrine] release_hold clears the LATCH; the gate
        // re-derives the hold from live counters ([A 2026-10-20] shared F3
        // derivation) and /release is arifOS-arbitrated fail-closed. A
        // release without verification evidence must NOT free a measured
        // STUCK actor — the quotient governs, not the flag. This test
        // previously asserted flag-era semantics and went red the day the
        // re-derive commit landed; it now pins the doctrine instead.
        enforcer.release_hold("test");
        let (allowed2, reason2, _) = enforcer.check_actor("test");
        assert!(
            !allowed2,
            "release without verify evidence must not clear a measured STUCK hold: {}",
            reason2
        );

        // The legitimate path (mirrors /ingest auto-release): verify up to
        // the metabolic floor. 6 executes need quotient ≥ 0.5 → 3 verifies
        // (3/6 = 0.5, not < floor) — then the re-derived condition lifts.
        for _ in 0..3 {
            let v = FlowReceipt::new_first(
                "test",
                "s1",
                StepType::Verify,
                EpistemicLabel::Derivation,
                1_000_000,
            );
            enforcer.ingest(&v);
        }
        enforcer.release_hold("test");
        let (allowed3, reason3, _) = enforcer.check_actor("test");
        assert!(
            allowed3,
            "actor at the quotient floor must pass after verify + release: {}",
            reason3
        );
    }

    #[test]
    fn test_restricted_actors() {
        let mut enforcer = InvariantEnforcer::default();

        for _ in 0..6 {
            let r = FlowReceipt::new_first(
                "bad",
                "s1",
                StepType::Execute,
                EpistemicLabel::Observation,
                1_000_000,
            );
            enforcer.ingest(&r);
        }
        enforcer.enforce();

        let restricted = enforcer.restricted_actors();
        assert!(!restricted.is_empty());
        assert_eq!(restricted[0].0, "bad");
        assert_eq!(restricted[0].1, EnforcerAction::Hold);
    }

    #[test]
    fn test_invariant_status_helpers() {
        assert!(InvariantStatus::Hold.is_blocking());
        assert!(InvariantStatus::Void.is_blocking());
        assert!(!InvariantStatus::Pass.is_blocking());
        assert!(!InvariantStatus::Warn.is_blocking());
    }

    #[test]
    fn test_default_thresholds() {
        let t = FqThresholds::default();
        assert_eq!(t.stuck_threshold, 0.5);
        assert_eq!(t.overheat_threshold, 10.0);
        assert_eq!(t.max_consecutive_executes, 5);
    }

    // ── F-4 (2026-10-25): fixture exclusion by validated CLASS, never name ──

    use crate::governance::entity_class::{ClassDeclaration, DeclarationSource};

    /// 4 Executes + 1 Verify → quotient 0.25 (below the 0.50 min-gate).
    fn ingest_low_fq(enf: &mut InvariantEnforcer, actor: &str) {
        for _ in 0..4 {
            enf.ingest(&FlowReceipt::new_first(
                actor,
                "s-low",
                StepType::Execute,
                EpistemicLabel::Observation,
                1_000_000,
            ));
        }
        enf.ingest(&FlowReceipt::new_first(
            actor,
            "s-low",
            StepType::Verify,
            EpistemicLabel::Derivation,
            500_000,
        ));
    }

    /// 2 Executes + 2 Verifies → quotient 1.0.
    fn ingest_healthy(enf: &mut InvariantEnforcer, actor: &str) {
        for _ in 0..2 {
            enf.ingest(&FlowReceipt::new_first(
                actor,
                "s-ok",
                StepType::Execute,
                EpistemicLabel::Observation,
                1_000_000,
            ));
            enf.ingest(&FlowReceipt::new_first(
                actor,
                "s-ok",
                StepType::Verify,
                EpistemicLabel::Derivation,
                500_000,
            ));
        }
    }

    /// (a) An ordinary actor literally named "test" with low FQ IS included:
    /// its 0.25 gates the aggregate. A name is not a classification.
    #[test]
    fn test_aggregate_includes_ordinary_actor_named_test() {
        let mut enf = InvariantEnforcer::default();
        ingest_low_fq(&mut enf, "test");
        ingest_healthy(&mut enf, "healthy");
        assert_eq!(
            enf.actors.get("test").unwrap().actor_class,
            ActorClass::Unclassified
        );
        let agg = federation_fq_aggregate(&enf.actors).unwrap();
        assert!(
            (agg - 0.25).abs() < 1e-9,
            "actor named 'test' with no validated declaration must gate the aggregate, got {}",
            agg
        );
    }

    /// (b) A genuine fixture with VALIDATED fixture provenance IS excluded
    /// from the aggregate (raw per-actor data stays visible).
    #[test]
    fn test_aggregate_excludes_validated_fixture() {
        let mut enf = InvariantEnforcer::default();
        let mut reg = ClassRegistry::new();
        reg.register(
            ClassDeclaration::validate(
                "test",
                ActorClass::Fixture,
                "operator:scenario-harness-2026-10-25",
                DeclarationSource::TestHarness,
            )
            .expect("validated fixture declaration"),
        )
        .expect("registration");
        enf.set_class_registry(reg);

        ingest_low_fq(&mut enf, "test");
        ingest_healthy(&mut enf, "healthy");
        assert_eq!(
            enf.actors.get("test").unwrap().actor_class,
            ActorClass::Fixture
        );
        let agg = federation_fq_aggregate(&enf.actors).unwrap();
        assert!(
            (agg - 1.0).abs() < 1e-9,
            "validated fixture must be excluded from the aggregate, got {}",
            agg
        );
        // The fixture's own numbers remain visible per-actor.
        assert_eq!(enf.actors.get("test").unwrap().execute_count, 4);
    }

    /// (c) Unknown or invalid classification is NOT exempt — fail-closed to
    /// counting the actor. Every rejected declaration leaves it Unclassified.
    #[test]
    fn test_aggregate_fail_closed_on_invalid_classification() {
        let mut enf = InvariantEnforcer::default();
        let mut reg = ClassRegistry::new();
        // Self-declaration.
        assert!(ClassDeclaration::validate(
            "test",
            ActorClass::Fixture,
            "test",
            DeclarationSource::TestHarness
        )
        .is_err());
        // Empty declarer.
        assert!(ClassDeclaration::validate(
            "test",
            ActorClass::Fixture,
            "   ",
            DeclarationSource::OperatorConfig
        )
        .is_err());
        // Unclassified is not a declarable exemption.
        assert!(ClassDeclaration::validate(
            "test",
            ActorClass::Unclassified,
            "operator",
            DeclarationSource::OperatorConfig
        )
        .is_err());
        // Caller-supplied labels are not a declaration source.
        assert!(DeclarationSource::parse("fixture").is_err());
        assert!(DeclarationSource::parse("self").is_err());
        assert!(reg.is_empty());

        enf.set_class_registry(reg);
        ingest_low_fq(&mut enf, "test");
        ingest_healthy(&mut enf, "healthy");
        let agg = federation_fq_aggregate(&enf.actors).unwrap();
        assert!(
            (agg - 0.25).abs() < 1e-9,
            "invalid classification must not exempt: the actor counts, got {}",
            agg
        );
    }

    /// A "fixture" label on the wire cannot self-exempt: ingestion is the
    /// only wire path into actor state and it never stamps a class.
    #[test]
    fn test_caller_supplied_fixture_label_does_not_self_exempt() {
        let mut enf = InvariantEnforcer::default();
        ingest_low_fq(&mut enf, "staged_agent");
        ingest_healthy(&mut enf, "healthy");
        assert_eq!(
            enf.actors.get("staged_agent").unwrap().actor_class,
            ActorClass::Unclassified,
            "receipts cannot classify themselves"
        );
        let agg = federation_fq_aggregate(&enf.actors).unwrap();
        assert!((agg - 0.25).abs() < 1e-9, "got {}", agg);
    }

    /// Strictness pin: the constitutional formula is preserved exactly —
    /// min < 0.50 gates to the min; otherwise harmonic mean. Fixture
    /// exclusion may only REMOVE declared fixtures, never loosen the gate.
    #[test]
    fn test_aggregate_min_gate_and_harmonic_mean_preserved() {
        let mut enf = InvariantEnforcer::default();
        // low: 5 exec / 2 verify → 0.4 ; high: 1 exec / 2 verify → 2.0
        for _ in 0..5 {
            enf.ingest(&FlowReceipt::new_first(
                "low",
                "s1",
                StepType::Execute,
                EpistemicLabel::Observation,
                1_000_000,
            ));
        }
        for _ in 0..2 {
            enf.ingest(&FlowReceipt::new_first(
                "low",
                "s1",
                StepType::Verify,
                EpistemicLabel::Derivation,
                500_000,
            ));
        }
        enf.ingest(&FlowReceipt::new_first(
            "high",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            1_000_000,
        ));
        for _ in 0..2 {
            enf.ingest(&FlowReceipt::new_first(
                "high",
                "s1",
                StepType::Verify,
                EpistemicLabel::Derivation,
                500_000,
            ));
        }
        // min 0.4 < 0.50 → gate to the min.
        assert!((federation_fq_aggregate(&enf.actors).unwrap() - 0.4).abs() < 1e-9);

        // Both actors ≥ 0.50 (mid 5 exec / 3 verify → 0.6, high 1 / 2 → 2.0):
        // h = 2 / (1/0.6 + 1/2.0) — harmonic mean, no min-gate.
        let mut enf2 = InvariantEnforcer::default();
        for _ in 0..5 {
            enf2.ingest(&FlowReceipt::new_first(
                "mid",
                "s2",
                StepType::Execute,
                EpistemicLabel::Observation,
                1_000_000,
            ));
        }
        for _ in 0..3 {
            enf2.ingest(&FlowReceipt::new_first(
                "mid",
                "s2",
                StepType::Verify,
                EpistemicLabel::Derivation,
                500_000,
            ));
        }
        enf2.ingest(&FlowReceipt::new_first(
            "high",
            "s1",
            StepType::Execute,
            EpistemicLabel::Observation,
            1_000_000,
        ));
        for _ in 0..2 {
            enf2.ingest(&FlowReceipt::new_first(
                "high",
                "s1",
                StepType::Verify,
                EpistemicLabel::Derivation,
                500_000,
            ));
        }
        let agg = federation_fq_aggregate(&enf2.actors).unwrap();
        let expected = 2.0 / (1.0 / 0.6 + 1.0 / 2.0);
        assert!(
            (agg - expected).abs() < 1e-9,
            "harmonic mean must be preserved, got {} expected {}",
            agg,
            expected
        );
    }

    // ── CM-1 explanatory-class refusals (2026-09-19) ─────────────────────

    fn narrative_execute_receipt() -> FlowReceipt {
        FlowReceipt::new_first(
            "333-AGI",
            "s-cm1",
            StepType::Execute,
            EpistemicLabel::Specification,
            1_000,
        )
        .with_explanation_class(ExplanationClass::Narrative)
        .with_explanation_schema(crate::receipt::EXPLANATION_SCHEMA_V1)
    }

    /// A refused execution-class receipt is reported as an F3 flow-plane
    /// violation under the named code — and the ACTOR IS NOT HELD, because
    /// holding an actor for the class of its justification would be judging.
    #[test]
    fn test_explanation_refusal_reported_as_flow_plane_violation() {
        let mut enforcer = InvariantEnforcer::default();
        let receipt = narrative_execute_receipt();
        let gate = receipt.explanation_gate();
        assert!(gate.is_refused());

        let check = enforcer.note_explanation_refusal(&receipt, &gate);
        assert_eq!(check.invariant, FlowInvariant::F3_ObserveNeverInterpret);
        assert_eq!(check.status, InvariantStatus::Hold);
        assert!(
            check
                .reason
                .contains(crate::receipt::FLOW_CODE_EXPLANATION_CLASS_INELIGIBLE),
            "reason must carry the named code: {}",
            check.reason
        );
        assert!(check.reason.contains("NARRATIVE"));
        assert!(
            check.evidence.contains("verdict_owner=claim_kernel"),
            "evidence must state who owns the verdict: {}",
            check.evidence
        );
        assert_eq!(enforcer.explanation_refusals, 1);

        // F2: no actor HOLD is manufactured from a refusal.
        let (allowed, _, action) = enforcer.check_actor("333-agi");
        assert!(allowed, "refusal must not hold the actor");
        assert_eq!(action, EnforcerAction::Allow);
        assert!(!enforcer.actors.contains_key("333-agi"));
    }

    /// The refusal surfaces in the next enforcement report, then drains.
    #[test]
    fn test_explanation_refusal_surfaces_in_enforce_report() {
        let mut enforcer = InvariantEnforcer::default();
        let receipt = narrative_execute_receipt();
        let gate = receipt.explanation_gate();
        enforcer.note_explanation_refusal(&receipt, &gate);

        let report = enforcer.enforce();
        let surfaced = report.checks.iter().any(|c| {
            c.reason
                .contains(crate::receipt::FLOW_CODE_EXPLANATION_CLASS_INELIGIBLE)
        });
        assert!(
            surfaced,
            "enforcement report must carry the refusal under the named code"
        );
        assert!(enforcer.pending_explanation_violations.is_empty());
        assert_eq!(report.overall_status, InvariantStatus::Hold);

        // Second cycle: nothing pending, so the refusal is not double-counted.
        let report_2 = enforcer.enforce();
        assert!(!report_2.checks.iter().any(|c| {
            c.reason
                .contains(crate::receipt::FLOW_CODE_EXPLANATION_CLASS_INELIGIBLE)
        }));
        assert_eq!(enforcer.explanation_refusals, 1);
    }

    /// Eligible classes never produce a refusal record.
    #[test]
    fn test_eligible_class_produces_no_refusal() {
        let enforcer = InvariantEnforcer::default();
        let receipt = FlowReceipt::new_first(
            "A-FORGE",
            "s-cm1",
            StepType::Execute,
            EpistemicLabel::Specification,
            1_000,
        )
        .with_explanation_class(ExplanationClass::Mechanism)
        .with_explanation_schema(crate::receipt::EXPLANATION_SCHEMA_V1);
        let gate = receipt.explanation_gate();
        assert_eq!(gate.outcome_code, "ELIGIBLE");
        assert!(gate.is_accepted());
        assert_eq!(enforcer.explanation_refusals, 0);
        assert!(enforcer.last_explanation_violation.is_none());
    }
}
