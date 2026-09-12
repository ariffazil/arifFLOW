// arifFlow topology/controlled_cycle.rs
// Controlled Cycle Topology — convergent WORK→VERIFY→[PASS|FAIL] loops
//
// The 4th governed topology, completing the base shapes from
// Graph Engineering Patterns (Pattern 4: Controlled Cycle).
//
// Topology:
//   WORK → VERIFY → [PASS → EXIT]
//                  → [FAIL → FEEDBACK → WORK]
//
// Every cycle needs: hard stop, convergence metric, cost budget, escalation.
// See Pattern 9 (Convergent Cycles) and Pattern 10 (Local Failure).
//
// Invariant F4: "Too many paths = untestable governance."
//   ControlledCycle is justified because:
//   1. It is the only missing base shape from the 12-pattern taxonomy
//   2. It already exists conceptually in Pipeline's review_every_n
//   3. Every convergent workflow needs it
//   4. It is governable: max_rounds + convergence_threshold + budget = testable
//
// DITEMPA BUKAN DIBERI — Forged, Not Given.

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ── Configuration ────────────────────────────────────────────────────────

/// Configuration for a controlled cycle run.
///
/// Defines the convergence criteria and safety bounds for the
/// WORK→VERIFY→[PASS|FAIL] loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlledCycleConfig {
    /// Maximum number of WORK→VERIFY rounds before forced exit.
    /// Hard stop — prevents infinite loops (Pattern 9: every cycle needs hard stop).
    pub max_rounds: u32,

    /// Minimum FQ improvement between rounds to count as "progress".
    /// If the FQ delta between consecutive rounds is below this threshold,
    /// the round is considered a "dry round" (no meaningful improvement).
    pub convergence_threshold: f64,

    /// Number of consecutive dry rounds before forced exit.
    /// If convergence_threshold is not met for this many rounds, the cycle
    /// is considered stalled and exits with ConvergenceStalled.
    pub dry_rounds_limit: u32,

    /// Total cost budget in nanoseconds.
    /// Sum of all step cost_ns across all rounds must not exceed this.
    /// 0 = unlimited (not recommended for production).
    pub budget_ns: u64,

    /// Default escalation target when the cycle exits abnormally.
    /// Overridden by exit-condition-specific targets in CycleSummary.
    pub default_escalation_target: EscalationTarget,
}

impl Default for ControlledCycleConfig {
    fn default() -> Self {
        Self {
            max_rounds: 6,
            convergence_threshold: 0.05,
            dry_rounds_limit: 2,
            budget_ns: 300_000_000_000, // 5 minutes
            default_escalation_target: EscalationTarget::Sovereign888,
        }
    }
}

// ── Convergence State ────────────────────────────────────────────────────

/// Result of a single round's convergence check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConvergenceState {
    /// FQ improved beyond convergence_threshold → genuine progress
    Progressing,
    /// FQ improved but below threshold → dry round (no meaningful gain)
    DryRound,
    /// FQ degraded → the cycle is making things worse
    Diverging,
    /// First round — no prior FQ to compare against
    FirstRound,
}

impl ConvergenceState {
    pub fn is_dry(&self) -> bool {
        matches!(self, ConvergenceState::DryRound)
    }

    pub fn is_diverging(&self) -> bool {
        matches!(self, ConvergenceState::Diverging)
    }
}

impl std::fmt::Display for ConvergenceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Progressing => write!(f, "PROGRESSING"),
            Self::DryRound => write!(f, "DRY_ROUND"),
            Self::Diverging => write!(f, "DIVERGING"),
            Self::FirstRound => write!(f, "FIRST_ROUND"),
        }
    }
}

// ── Exit Condition ───────────────────────────────────────────────────────

/// Why the controlled cycle exited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CycleExitCondition {
    /// All verification passed → natural exit
    Passed,
    /// Hit max_rounds without convergence → escalation needed
    MaxRoundsExceeded,
    /// Hit budget_ns → cost limit reached
    BudgetExhausted,
    /// Dry rounds exceeded → stalled, no progress
    ConvergenceStalled,
    /// FQ diverged → making things worse, abort
    DivergenceDetected,
    /// Manual cancellation
    Cancelled,
}

impl std::fmt::Display for CycleExitCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Passed => write!(f, "PASSED"),
            Self::MaxRoundsExceeded => write!(f, "MAX_ROUNDS_EXCEEDED"),
            Self::BudgetExhausted => write!(f, "BUDGET_EXHAUSTED"),
            Self::ConvergenceStalled => write!(f, "CONVERGENCE_STALLED"),
            Self::DivergenceDetected => write!(f, "DIVERGENCE_DETECTED"),
            Self::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

/// Who receives the escalation when a cycle exits abnormally.
///
/// Not all non-pass exits have the same escalation target. A user cancellation
/// is not necessarily an F13 escalation. A budget cap may belong to an
/// operational owner. A divergence event may require an independent verifier.
/// Only canonical mutation gaps require the sovereign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscalationTarget {
    /// No escalation needed — cycle succeeded
    None,
    /// Operator-level cancellation or manual intervention
    Operator,
    /// Budget owner — cost extension changes authority/risk
    BudgetOwner,
    /// Evidence owner — truth remains unresolved
    EvidenceOwner,
    /// Independent verifier — needs third-party audit
    IndependentVerifier,
    /// Organ owner — the organ that owns the topology
    OrganOwner,
    /// Sovereign 888 — canonical mutation or authority gap
    Sovereign888,
}

impl EscalationTarget {
    /// Returns the default escalation target for a given exit condition.
    pub fn for_exit(exit: CycleExitCondition) -> Self {
        match exit {
            CycleExitCondition::Passed => Self::None,
            CycleExitCondition::Cancelled => Self::Operator,
            CycleExitCondition::BudgetExhausted => Self::BudgetOwner,
            CycleExitCondition::MaxRoundsExceeded => Self::OrganOwner,
            CycleExitCondition::ConvergenceStalled => Self::EvidenceOwner,
            CycleExitCondition::DivergenceDetected => Self::Sovereign888,
        }
    }

    /// Returns true if this escalation target maps to 888_HOLD.
    pub fn requires_888_hold(&self) -> bool {
        matches!(self, Self::Sovereign888)
    }
}

impl std::fmt::Display for EscalationTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "NONE"),
            Self::Operator => write!(f, "OPERATOR"),
            Self::BudgetOwner => write!(f, "BUDGET_OWNER"),
            Self::EvidenceOwner => write!(f, "EVIDENCE_OWNER"),
            Self::IndependentVerifier => write!(f, "INDEPENDENT_VERIFIER"),
            Self::OrganOwner => write!(f, "ORGAN_OWNER"),
            Self::Sovereign888 => write!(f, "SOVEREIGN_888"),
        }
    }
}

// ── Cycle Errors ─────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum CycleError {
    #[error("Cycle already complete — exit condition: {0}")]
    AlreadyComplete(CycleExitCondition),
    #[error("Budget exhausted after {rounds} rounds: {cost_ns} ns / {budget_ns} ns")]
    BudgetExhausted {
        rounds: u32,
        cost_ns: u64,
        budget_ns: u64,
    },
}

// ── Round Record ─────────────────────────────────────────────────────────

/// Record of one WORK→VERIFY round within the cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleRound {
    /// Round number (0-indexed)
    pub round: u32,
    /// FQ at the start of this round
    pub fq_before: Option<f64>,
    /// FQ at the end of this round
    pub fq_after: Option<f64>,
    /// Convergence state for this round
    pub convergence: ConvergenceState,
    /// Cost of this round in nanoseconds
    pub cost_ns: u64,
    /// Did verification pass in this round?
    pub verification_passed: bool,
    /// Number of consecutive dry rounds at the END of this round
    pub consecutive_dry: u32,
}

// ── Cycle Summary ────────────────────────────────────────────────────────

/// Final summary of a completed controlled cycle.
///
/// This is the most important object — it makes it impossible for the
/// federation to confuse success, safe stop, failure, and sovereign decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleSummary {
    /// Total rounds executed
    pub total_rounds: u32,
    /// Total cost across all rounds in nanoseconds
    pub total_cost_ns: u64,
    /// FQ at cycle start
    pub fq_start: Option<f64>,
    /// FQ at cycle end
    pub fq_end: Option<f64>,
    /// FQ delta across the cycle
    pub fq_delta: Option<f64>,
    /// Why the cycle exited
    pub exit_condition: CycleExitCondition,
    /// Who should receive the escalation
    pub escalation_target: EscalationTarget,
    /// Per-round records
    pub rounds: Vec<CycleRound>,
    /// Budget limit (None = unlimited)
    pub budget_limit: Option<u64>,
}

impl CycleSummary {
    /// Returns true if the cycle exited because verification passed.
    pub fn succeeded(&self) -> bool {
        self.exit_condition == CycleExitCondition::Passed
    }

    /// Returns true if the cycle needs escalation (human/888 intervention).
    pub fn needs_escalation(&self) -> bool {
        self.escalation_target != EscalationTarget::None
    }

    /// Returns true if this exit requires 888_HOLD (sovereign gate).
    pub fn requires_888_hold(&self) -> bool {
        self.escalation_target.requires_888_hold()
    }

    /// Returns true if the cycle cannot be resumed without explicit authorization.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.exit_condition,
            CycleExitCondition::DivergenceDetected
                | CycleExitCondition::BudgetExhausted
                | CycleExitCondition::MaxRoundsExceeded
        )
    }
}

// ── Controlled Cycle Engine ──────────────────────────────────────────────

/// The Controlled Cycle topology engine.
///
/// Manages the WORK→VERIFY→[PASS|FAIL] loop with convergence tracking,
/// budget enforcement, and escalation. This is the 4th governed topology
/// in arifFlow, completing the base shapes from graph engineering patterns.
///
/// Usage:
/// ```ignore
/// let mut cycle = ControlledCycle::new(config);
///
/// // Each round:
/// let round_result = cycle.start_round(fq_before)?;
/// // ... execute WORK + VERIFY ...
/// cycle.end_round(round_result, fq_after, verification_passed)?;
///
/// // Check if we should continue
/// if let Some(summary) = cycle.check_exit() {
///     // Cycle is done — use summary
/// }
/// ```
pub struct ControlledCycle {
    config: ControlledCycleConfig,
    current_round: u32,
    total_cost_ns: u64,
    consecutive_dry: u32,
    rounds: Vec<CycleRound>,
    fq_history: Vec<Option<f64>>,
    complete: bool,
    exit_condition: Option<CycleExitCondition>,
}

impl ControlledCycle {
    /// Create a new controlled cycle with the given configuration.
    pub fn new(config: ControlledCycleConfig) -> Self {
        Self {
            config,
            current_round: 0,
            total_cost_ns: 0,
            consecutive_dry: 0,
            rounds: Vec::new(),
            fq_history: Vec::new(),
            complete: false,
            exit_condition: None,
        }
    }

    /// Start a new round. Records the FQ before execution.
    /// Returns the round number.
    pub fn start_round(&mut self, fq_before: Option<f64>) -> Result<u32, CycleError> {
        if self.complete {
            return Err(CycleError::AlreadyComplete(
                self.exit_condition
                    .unwrap_or(CycleExitCondition::Cancelled),
            ));
        }

        let round = self.current_round;
        self.fq_history.push(fq_before);
        Ok(round)
    }

    /// End a round. Computes convergence state and checks exit conditions.
    pub fn end_round(
        &mut self,
        round: u32,
        cost_ns: u64,
        fq_after: Option<f64>,
        verification_passed: bool,
    ) -> Result<(), CycleError> {
        if self.complete {
            return Err(CycleError::AlreadyComplete(
                self.exit_condition
                    .unwrap_or(CycleExitCondition::Cancelled),
            ));
        }

        // Budget check
        self.total_cost_ns = self.total_cost_ns.saturating_add(cost_ns);
        if self.config.budget_ns > 0 && self.total_cost_ns > self.config.budget_ns {
            self.complete = true;
            self.exit_condition = Some(CycleExitCondition::BudgetExhausted);
            return Ok(());
        }

        // Convergence computation
        let fq_before = self.fq_history.last().copied().flatten();
        let convergence = Self::compute_convergence(fq_before, fq_after);

        // Dry rounds tracking
        match convergence {
            ConvergenceState::DryRound => {
                self.consecutive_dry += 1;
            }
            ConvergenceState::Diverging => {
                // Divergence is worse than dry — count as 2 dry rounds
                self.consecutive_dry += 2;
            }
            _ => {
                self.consecutive_dry = 0;
            }
        }

        // Record round
        self.rounds.push(CycleRound {
            round,
            fq_before,
            fq_after,
            convergence,
            cost_ns,
            verification_passed,
            consecutive_dry: self.consecutive_dry,
        });

        // Exit condition checks — ORDER MATTERS:
        // 1. Passed (highest priority — success is success)
        // 2. Divergence (quality degradation — must stop immediately)
        // 3. Stall (no progress — but quality hasn't worsened)
        // 4. Budget (resource exhaustion)
        // 5. Max rounds (iteration limit)
        if verification_passed {
            self.complete = true;
            self.exit_condition = Some(CycleExitCondition::Passed);
            return Ok(());
        }

        if convergence.is_diverging() {
            self.complete = true;
            self.exit_condition = Some(CycleExitCondition::DivergenceDetected);
            return Ok(());
        }

        if self.consecutive_dry >= self.config.dry_rounds_limit {
            self.complete = true;
            self.exit_condition = Some(CycleExitCondition::ConvergenceStalled);
            return Ok(());
        }

        self.current_round = round + 1;
        if self.current_round >= self.config.max_rounds {
            self.complete = true;
            self.exit_condition = Some(CycleExitCondition::MaxRoundsExceeded);
        }

        Ok(())
    }

    /// Check if the cycle should exit. Returns the summary if complete.
    pub fn check_exit(&self) -> Option<CycleSummary> {
        if !self.complete {
            return None;
        }
        Some(self.summary())
    }

    /// Force-cancel the cycle.
    pub fn cancel(&mut self) {
        self.complete = true;
        self.exit_condition = Some(CycleExitCondition::Cancelled);
    }

    /// Build the final summary.
    pub fn summary(&self) -> CycleSummary {
        let fq_start = self.fq_history.first().copied().flatten();
        let fq_end = self.rounds.last().and_then(|r| r.fq_after);
        let fq_delta = match (fq_start, fq_end) {
            (Some(a), Some(b)) => Some(b - a),
            _ => None,
        };
        let exit = self.exit_condition.unwrap_or(CycleExitCondition::Cancelled);

        CycleSummary {
            total_rounds: self.current_round,
            total_cost_ns: self.total_cost_ns,
            fq_start,
            fq_end,
            fq_delta,
            exit_condition: exit,
            escalation_target: EscalationTarget::for_exit(exit),
            rounds: self.rounds.clone(),
            budget_limit: if self.config.budget_ns > 0 {
                Some(self.config.budget_ns)
            } else {
                None
            },
        }
    }

    /// Current round number (0-indexed).
    pub fn current_round(&self) -> u32 {
        self.current_round
    }

    /// Total cost so far.
    pub fn total_cost_ns(&self) -> u64 {
        self.total_cost_ns
    }

    /// Whether the cycle is complete.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Exit condition, if known.
    pub fn exit_condition(&self) -> Option<CycleExitCondition> {
        self.exit_condition
    }

    /// Number of consecutive dry rounds.
    pub fn consecutive_dry(&self) -> u32 {
        self.consecutive_dry
    }

    /// Compute convergence state between two FQ measurements.
    fn compute_convergence(fq_before: Option<f64>, fq_after: Option<f64>) -> ConvergenceState {
        match (fq_before, fq_after) {
            (None, _) | (_, None) => ConvergenceState::FirstRound,
            (Some(before), Some(after)) => {
                let delta = after - before;
                if delta > 0.0 && delta >= after.abs() * 0.05 {
                    // At least 5% relative improvement
                    ConvergenceState::Progressing
                } else if delta >= 0.0 {
                    ConvergenceState::DryRound
                } else {
                    ConvergenceState::Diverging
                }
            }
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> ControlledCycleConfig {
        ControlledCycleConfig {
            max_rounds: 4,
            convergence_threshold: 0.05,
            dry_rounds_limit: 2,
            budget_ns: 1_000_000_000, // 1 second
            default_escalation_target: EscalationTarget::Sovereign888,
        }
    }

    #[test]
    fn test_cycle_passes_on_first_round() {
        let mut cycle = ControlledCycle::new(default_config());
        let round = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(round, 100, Some(1.5), true).unwrap();

        let summary = cycle.summary();
        assert!(summary.succeeded());
        assert_eq!(summary.exit_condition, CycleExitCondition::Passed);
        assert_eq!(summary.total_rounds, 0);
        assert_eq!(summary.escalation_target, EscalationTarget::None);
    }

    #[test]
    fn test_cycle_exits_on_max_rounds() {
        let config = ControlledCycleConfig {
            max_rounds: 2,
            dry_rounds_limit: 10, // high enough that max_rounds triggers first
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);

        // Round 0: dry
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(1.01), false).unwrap();
        assert!(!cycle.is_complete());

        // Round 1: dry again
        let r = cycle.start_round(Some(1.01)).unwrap();
        cycle.end_round(r, 100, Some(1.02), false).unwrap();
        // max_rounds=2, current_round now 2 → should be complete
        assert!(cycle.is_complete());
        assert_eq!(
            cycle.exit_condition(),
            Some(CycleExitCondition::MaxRoundsExceeded)
        );
    }

    #[test]
    fn test_cycle_exits_on_budget() {
        let config = ControlledCycleConfig {
            max_rounds: 10,
            budget_ns: 500,
            dry_rounds_limit: 10,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);

        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 600, Some(1.0), false).unwrap(); // 600 > 500 budget

        assert!(cycle.is_complete());
        assert_eq!(
            cycle.exit_condition(),
            Some(CycleExitCondition::BudgetExhausted)
        );
    }

    #[test]
    fn test_cycle_exits_on_convergence_stall() {
        let config = ControlledCycleConfig {
            max_rounds: 10,
            dry_rounds_limit: 2,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);

        // Round 0: dry
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(1.005), false).unwrap();
        assert!(!cycle.is_complete());
        assert_eq!(cycle.consecutive_dry(), 1);

        // Round 1: dry again → consecutive_dry = 2 = dry_rounds_limit
        let r = cycle.start_round(Some(1.005)).unwrap();
        cycle.end_round(r, 100, Some(1.01), false).unwrap();

        assert!(cycle.is_complete());
        assert_eq!(
            cycle.exit_condition(),
            Some(CycleExitCondition::ConvergenceStalled)
        );
    }

    #[test]
    fn test_cycle_exits_on_divergence() {
        let config = ControlledCycleConfig {
            max_rounds: 10,
            dry_rounds_limit: 10,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);

        // Round 0: FQ drops → divergence
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(0.5), false).unwrap();

        assert!(cycle.is_complete());
        assert_eq!(
            cycle.exit_condition(),
            Some(CycleExitCondition::DivergenceDetected)
        );
    }

    #[test]
    fn test_convergence_computation() {
        // Progressing: significant improvement
        assert_eq!(
            ControlledCycle::compute_convergence(Some(1.0), Some(2.0)),
            ConvergenceState::Progressing
        );

        // DryRound: marginal improvement
        assert_eq!(
            ControlledCycle::compute_convergence(Some(1.0), Some(1.01)),
            ConvergenceState::DryRound
        );

        // Diverging: FQ dropped
        assert_eq!(
            ControlledCycle::compute_convergence(Some(1.0), Some(0.5)),
            ConvergenceState::Diverging
        );

        // FirstRound: no before value
        assert_eq!(
            ControlledCycle::compute_convergence(None, Some(1.0)),
            ConvergenceState::FirstRound
        );
    }

    #[test]
    fn test_dry_rounds_reset_on_progress() {
        let config = ControlledCycleConfig {
            max_rounds: 10,
            dry_rounds_limit: 3,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);

        // Round 0: dry
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(1.005), false).unwrap();
        assert_eq!(cycle.consecutive_dry(), 1);

        // Round 1: dry
        let r = cycle.start_round(Some(1.005)).unwrap();
        cycle.end_round(r, 100, Some(1.01), false).unwrap();
        assert_eq!(cycle.consecutive_dry(), 2);

        // Round 2: progressing → reset dry counter
        let r = cycle.start_round(Some(1.01)).unwrap();
        cycle.end_round(r, 100, Some(2.0), false).unwrap();
        assert_eq!(cycle.consecutive_dry(), 0);

        // Round 3: dry again → starts from 1
        let r = cycle.start_round(Some(2.0)).unwrap();
        cycle.end_round(r, 100, Some(2.01), false).unwrap();
        assert_eq!(cycle.consecutive_dry(), 1);
    }

    #[test]
    fn test_cycle_needs_escalation() {
        let config = ControlledCycleConfig {
            max_rounds: 1,
            dry_rounds_limit: 10,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);

        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(1.005), false).unwrap();

        let summary = cycle.summary();
        assert!(!summary.succeeded());
        assert!(summary.needs_escalation());
        // MaxRoundsExceeded → OrganOwner, not Sovereign888
        assert_eq!(
            summary.escalation_target,
            EscalationTarget::OrganOwner
        );
        assert!(!summary.requires_888_hold());
    }

    #[test]
    fn test_cancel() {
        let mut cycle = ControlledCycle::new(default_config());
        let _ = cycle.start_round(Some(1.0));
        cycle.cancel();

        assert!(cycle.is_complete());
        assert_eq!(
            cycle.exit_condition(),
            Some(CycleExitCondition::Cancelled)
        );
    }

    #[test]
    fn test_cannot_start_after_complete() {
        let mut cycle = ControlledCycle::new(default_config());
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(1.5), true).unwrap(); // passes

        assert!(cycle.start_round(Some(1.5)).is_err());
    }

    #[test]
    fn test_summary_fq_delta() {
        let mut cycle = ControlledCycle::new(default_config());

        let r = cycle.start_round(Some(0.5)).unwrap();
        cycle.end_round(r, 100, Some(1.5), true).unwrap();

        let summary = cycle.summary();
        assert_eq!(summary.fq_start, Some(0.5));
        assert_eq!(summary.fq_end, Some(1.5));
        assert!((summary.fq_delta.unwrap() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_zero_budget_means_unlimited() {
        let config = ControlledCycleConfig {
            budget_ns: 0,
            max_rounds: 3,
            dry_rounds_limit: 10,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);

        // Huge cost but budget is 0 (unlimited)
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, u64::MAX / 2, Some(1.005), false).unwrap();

        // Should NOT be budget-exhausted (budget=0 means unlimited)
        assert!(!cycle.is_complete());
    }

    // ── Governance-grade tests (Arif SEAL 2026-09-12) ─────────────────────

    #[test]
    fn test_passed_cycle_has_no_escalation_target() {
        let mut cycle = ControlledCycle::new(default_config());
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(1.5), true).unwrap();

        let s = cycle.summary();
        assert_eq!(s.escalation_target, EscalationTarget::None);
        assert!(!s.needs_escalation());
        assert!(!s.requires_888_hold());
    }

    #[test]
    fn test_cancelled_routes_to_operator_not_sovereign() {
        let mut cycle = ControlledCycle::new(default_config());
        let _ = cycle.start_round(Some(1.0));
        cycle.cancel();

        let s = cycle.summary();
        assert_eq!(s.escalation_target, EscalationTarget::Operator);
        assert!(!s.requires_888_hold());
    }

    #[test]
    fn test_divergence_routes_to_sovereign() {
        let mut cycle = ControlledCycle::new(default_config());
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(0.3), false).unwrap(); // FQ drops

        let s = cycle.summary();
        assert_eq!(s.exit_condition, CycleExitCondition::DivergenceDetected);
        assert_eq!(s.escalation_target, EscalationTarget::Sovereign888);
        assert!(s.requires_888_hold());
    }

    #[test]
    fn test_budget_exhaustion_routes_to_budget_owner() {
        let config = ControlledCycleConfig {
            budget_ns: 100,
            max_rounds: 10,
            dry_rounds_limit: 10,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 200, Some(1.005), false).unwrap(); // 200 > 100 budget

        let s = cycle.summary();
        assert_eq!(s.exit_condition, CycleExitCondition::BudgetExhausted);
        assert_eq!(s.escalation_target, EscalationTarget::BudgetOwner);
        assert!(!s.requires_888_hold());
    }

    #[test]
    fn test_convergence_stalled_routes_to_evidence_owner() {
        let config = ControlledCycleConfig {
            max_rounds: 10,
            dry_rounds_limit: 2,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);

        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(1.005), false).unwrap();
        let r = cycle.start_round(Some(1.005)).unwrap();
        cycle.end_round(r, 100, Some(1.01), false).unwrap();

        let s = cycle.summary();
        assert_eq!(s.exit_condition, CycleExitCondition::ConvergenceStalled);
        assert_eq!(s.escalation_target, EscalationTarget::EvidenceOwner);
        assert!(!s.requires_888_hold());
    }

    #[test]
    fn test_non_pass_cannot_mark_as_sealed_success() {
        // A diverged cycle must not be confused with success
        let mut cycle = ControlledCycle::new(default_config());
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(0.5), false).unwrap();

        let s = cycle.summary();
        assert!(!s.succeeded());
        assert!(s.needs_escalation());
        assert!(s.requires_888_hold());
    }

    #[test]
    fn test_cannot_resume_after_divergence() {
        let mut cycle = ControlledCycle::new(default_config());
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(0.5), false).unwrap();

        assert!(cycle.is_complete());
        assert!(cycle.start_round(Some(0.5)).is_err());
    }

    #[test]
    fn test_cannot_resume_after_budget_exhaustion() {
        let config = ControlledCycleConfig {
            budget_ns: 100,
            max_rounds: 10,
            dry_rounds_limit: 10,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 200, Some(1.005), false).unwrap();

        assert!(cycle.is_complete());
        assert!(cycle.start_round(Some(1.005)).is_err());
    }

    #[test]
    fn test_round_number_is_monotonic() {
        let config = ControlledCycleConfig {
            max_rounds: 5,
            dry_rounds_limit: 10,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);

        for expected in 0..3u32 {
            let r = cycle.start_round(Some(1.0 + expected as f64 * 0.1)).unwrap();
            assert_eq!(r, expected);
            cycle
                .end_round(r, 100, Some(1.0 + (expected + 1) as f64 * 0.1), false)
                .unwrap();
        }
    }

    #[test]
    fn test_convergence_state_recorded_per_round() {
        let config = ControlledCycleConfig {
            max_rounds: 4,
            dry_rounds_limit: 10,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);

        // Round 0: FirstRound
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(1.5), false).unwrap();

        // Round 1: Progressing
        let r = cycle.start_round(Some(1.5)).unwrap();
        cycle.end_round(r, 100, Some(3.0), false).unwrap();

        // Round 2: DryRound
        let r = cycle.start_round(Some(3.0)).unwrap();
        cycle.end_round(r, 100, Some(3.01), false).unwrap();

        // Round 3: Diverging
        let r = cycle.start_round(Some(3.01)).unwrap();
        cycle.end_round(r, 100, Some(2.0), false).unwrap();

        let s = cycle.summary();
        assert_eq!(s.rounds.len(), 4);
        assert_eq!(s.rounds[0].convergence, ConvergenceState::Progressing); // 1.0→1.5 = +50%
        assert_eq!(s.rounds[1].convergence, ConvergenceState::Progressing);
        assert_eq!(s.rounds[2].convergence, ConvergenceState::DryRound);
        assert_eq!(s.rounds[3].convergence, ConvergenceState::Diverging);
    }

    #[test]
    fn test_summary_links_all_round_receipts() {
        let config = ControlledCycleConfig {
            max_rounds: 3,
            dry_rounds_limit: 10,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);

        for i in 0..3u32 {
            let r = cycle.start_round(Some(1.0 + i as f64)).unwrap();
            cycle.end_round(r, 100 * (i + 1) as u64, Some(1.0 + i as f64), false)
                .unwrap();
        }

        let s = cycle.summary();
        assert_eq!(s.rounds.len(), 3);
        // Each round has a unique round number
        assert_eq!(s.rounds[0].round, 0);
        assert_eq!(s.rounds[1].round, 1);
        assert_eq!(s.rounds[2].round, 2);
        // Total cost is sum of all rounds
        assert_eq!(s.total_cost_ns, 100 + 200 + 300);
    }

    #[test]
    fn test_held_cycle_emits_not_false_completion() {
        let config = ControlledCycleConfig {
            max_rounds: 1,
            dry_rounds_limit: 10,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);

        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(1.005), false).unwrap();

        let s = cycle.summary();
        assert!(!s.succeeded());
        assert!(s.is_terminal());
        assert!(s.needs_escalation());
    }

    #[test]
    fn test_budget_limit_recorded_in_summary() {
        let config = ControlledCycleConfig {
            budget_ns: 500,
            max_rounds: 10,
            dry_rounds_limit: 10,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(1.5), true).unwrap();

        let s = cycle.summary();
        assert_eq!(s.budget_limit, Some(500));
    }

    #[test]
    fn test_budget_zero_limit_recorded_as_none() {
        let config = ControlledCycleConfig {
            budget_ns: 0,
            max_rounds: 3,
            dry_rounds_limit: 10,
            ..default_config()
        };
        let mut cycle = ControlledCycle::new(config);
        let r = cycle.start_round(Some(1.0)).unwrap();
        cycle.end_round(r, 100, Some(1.5), true).unwrap();

        let s = cycle.summary();
        assert_eq!(s.budget_limit, None);
    }

    #[test]
    fn test_escalation_target_display() {
        assert_eq!(EscalationTarget::None.to_string(), "NONE");
        assert_eq!(EscalationTarget::Operator.to_string(), "OPERATOR");
        assert_eq!(EscalationTarget::BudgetOwner.to_string(), "BUDGET_OWNER");
        assert_eq!(EscalationTarget::EvidenceOwner.to_string(), "EVIDENCE_OWNER");
        assert_eq!(
            EscalationTarget::IndependentVerifier.to_string(),
            "INDEPENDENT_VERIFIER"
        );
        assert_eq!(EscalationTarget::OrganOwner.to_string(), "ORGAN_OWNER");
        assert_eq!(
            EscalationTarget::Sovereign888.to_string(),
            "SOVEREIGN_888"
        );
    }

    #[test]
    fn test_escalation_target_requires_888_hold() {
        assert!(!EscalationTarget::None.requires_888_hold());
        assert!(!EscalationTarget::Operator.requires_888_hold());
        assert!(!EscalationTarget::BudgetOwner.requires_888_hold());
        assert!(!EscalationTarget::EvidenceOwner.requires_888_hold());
        assert!(!EscalationTarget::IndependentVerifier.requires_888_hold());
        assert!(!EscalationTarget::OrganOwner.requires_888_hold());
        assert!(EscalationTarget::Sovereign888.requires_888_hold());
    }
}
