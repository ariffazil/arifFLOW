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
        }
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
        let actor_id = receipt.actor_id.to_ascii_lowercase();
        let entry = self
            .actors
            .entry(actor_id.clone())
            .or_insert_with(|| ActorFlowState::new(actor_id));
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

    /// Check if a specific actor is allowed to execute.
    /// Returns (allowed, reason, action).
    pub fn check_actor(&self, actor_id: &str) -> (bool, String, EnforcerAction) {
        if let Some(state) = self.actors.get(actor_id) {
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
    /// Returns the records that were restored (i.e. were absent after seeding).
    pub fn restore_holds(&mut self, records: &[HoldRecord]) -> Vec<String> {
        let mut restored = Vec::new();
        for rec in records {
            // Canonicalize: seeding canonicalizes actor ids to lowercase, so a
            // persisted mixed-case id must fold to the same key.
            let key = rec.actor_id.to_ascii_lowercase();
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
            // Restoring a hold for an actor with no state at all: create a shell
            // so the verdict is not lost just because the receipt window aged out.
            let entry = self
                .actors
                .entry(key)
                .or_insert_with(|| ActorFlowState::new(rec.actor_id.clone()));
            entry.held = true;
            entry.throttled = true;
            entry.held_since_ns = if rec.held_since_ns != 0 {
                rec.held_since_ns
            } else {
                now_ns()
            };
            restored.push(rec.actor_id.clone());
        }
        restored
    }
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
    fn test_release_hold() {
        let mut enforcer = InvariantEnforcer::default();

        // Make actor stuck
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

        // Release hold
        enforcer.release_hold("test");
        let (allowed2, _, _) = enforcer.check_actor("test");
        assert!(allowed2, "Actor should be released after hold release");
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
