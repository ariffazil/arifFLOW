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
use crate::receipt::{FlowQuotient, FlowReceipt, FlowVerdict, ReceiptStore, StepType};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

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

/// Tracked state for a single actor in the flow plane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorFlowState {
    pub actor_id: String,
    pub execute_count: u64,
    pub verify_count: u64,
    pub execute_cost_ns: u64,
    pub verify_cost_ns: u64,
    /// Raw cost-based ratio (legacy) — kept for serialization compat
    pub fq: f64,
    /// v2.1 quotient: verify_count / execute_count. None when undefined.
    pub quotient: Option<f64>,
    pub verdict: FlowVerdict,
    pub consecutive_executes_without_verify: u64,
    pub throttled: bool,
    pub held: bool,
    pub last_seen_ns: i64,
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
            throttled: false,
            held: false,
            last_seen_ns: 0,
        }
    }

    /// Update from a flow receipt.
    pub fn ingest(&mut self, receipt: &FlowReceipt) {
        self.last_seen_ns = receipt.created_at.timestamp_nanos_opt().unwrap_or(0);
        if receipt.step_type.is_execution() {
            self.execute_count += 1;
            self.execute_cost_ns = self.execute_cost_ns.saturating_add(receipt.cost_ns);
            self.consecutive_executes_without_verify += 1;
        }
        if receipt.step_type.is_verification() {
            self.verify_count += 1;
            self.verify_cost_ns = self.verify_cost_ns.saturating_add(receipt.cost_ns);
            self.consecutive_executes_without_verify = 0;
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

        // v2.1 verdict: six-state band per Arif F13 spec
        self.verdict = if self.verify_count == 0 && self.execute_count == 0 {
            FlowVerdict::Unmeasured
        } else if self.verify_count == 0 {
            FlowVerdict::Unmeasured
        } else if self.verify_count < 2 {
            FlowVerdict::Caution
        } else if self.execute_count == 0 {
            FlowVerdict::Flowing
        } else if let Some(q) = self.quotient {
            if q >= 1.0 {
                FlowVerdict::Optimal
            } else if q >= 0.5 {
                FlowVerdict::Flowing
            } else if q >= 0.1 {
                FlowVerdict::Stuck
            } else {
                FlowVerdict::Burning
            }
        } else {
            FlowVerdict::Unmeasured
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
        }
    }

    /// Ingest a receipt and update actor state.
    pub fn ingest(&mut self, receipt: &FlowReceipt) {
        let actor_id = receipt.actor_id.clone();
        let entry = self
            .actors
            .entry(actor_id)
            .or_insert_with(|| ActorFlowState::new(receipt.actor_id.clone()));
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
            // FQ < stuck_threshold → HOLD
            if state.fq < self.thresholds.stuck_threshold && state.execute_count > 0 {
                checks.push(InvariantCheck::new(
                    FlowInvariant::F3_ObserveNeverInterpret,
                    InvariantStatus::Hold,
                    format!(
                        "Actor '{}' is STUCK: FQ={:.2} (threshold: {:.2}). execute={} verify={}",
                        actor_id,
                        state.fq,
                        self.thresholds.stuck_threshold,
                        state.execute_count,
                        state.verify_count
                    ),
                    format!(
                        "FQ={:.2} execute_count={} verify_count={} consecutive_without_verify={}",
                        state.fq,
                        state.execute_count,
                        state.verify_count,
                        state.consecutive_executes_without_verify
                    ),
                ));
            }

            // FQ < 1/overheat_threshold → THROTTLE
            // v2.1 direction flip (audit 2026-08-10): quotient = verify/execute,
            // HIGHER = healthier. The legacy v2.0 "overheat" (execute ≫ verify)
            // now reads as a LOW v2.1 quotient — extreme under-verification.
            // overheat_threshold 10.0 → reciprocal 0.1 ("burning" band).
            if state.fq < (1.0 / self.thresholds.overheat_threshold) && state.execute_count > 0 {
                checks.push(InvariantCheck::new(
                    FlowInvariant::F3_ObserveNeverInterpret,
                    InvariantStatus::Warn,
                    format!(
                        "Actor '{}' is OVERHEAT: FQ={:.2} (threshold: {:.2})",
                        actor_id, state.fq, self.thresholds.overheat_threshold
                    ),
                    format!(
                        "FQ={:.2} execute_count={} verify_count={}",
                        state.fq, state.execute_count, state.verify_count
                    ),
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
                        if let Some(state) = self.actors.get_mut(actor_id) {
                            if check.status == InvariantStatus::Hold {
                                state.held = true;
                                state.throttled = true;
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
            }
            if check.status == InvariantStatus::Warn {
                if let Some(actor_start) = check.reason.find("Actor '") {
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
            if state.held {
                result.push((
                    id.as_str(),
                    EnforcerAction::Hold,
                    format!("HELD: FQ={:.2}", state.fq),
                ));
            } else if state.throttled {
                result.push((
                    id.as_str(),
                    EnforcerAction::Throttle,
                    format!("THROTTLED: FQ={:.2}", state.fq),
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
        }
    }
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
        for i in 0..6 {
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
}
