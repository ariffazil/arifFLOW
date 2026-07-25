// arifFlow governance/cooling.rs
// Cooling Ledger — drift detection between plan and execution reality
//
// GAP P1-4: Cooling queue for post-execution drift analysis.
// Every super-step plan vs reality delta is recorded as a CoolingEntry.
// Convergent: plan matches reality. Divergent: plan ≠ reality → governance signal.
//
// Invariant C1: Cooling is OBSERVE-only — never blocks execution.
// Invariant C2: Cooling entries are append-only.
// Invariant C3: Threshold-based alerting on sustained divergence.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Convergence state of a cooling entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Convergence {
    /// Plan matched reality
    Converging,
    /// Plan is approaching reality
    Approaching,
    /// Plan diverged from reality
    Diverging,
    /// No plan was declared — pure observation
    Unplanned,
}

impl Convergence {
    pub fn is_diverging(&self) -> bool {
        matches!(self, Convergence::Diverging)
    }

    pub fn should_alert(&self) -> bool {
        matches!(self, Convergence::Diverging)
    }
}

/// Severity of drift
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftSeverity {
    /// Minor gap — within tolerance
    Low,
    /// Notable gap — warrants review
    Medium,
    /// Significant gap — governance attention needed
    High,
    /// Critical gap — constitutional concern
    Critical,
}

/// A single cooling entry — one plan-vs-reality observation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoolingEntry {
    /// Step number this entry refers to
    pub super_step: u64,
    /// What the plan predicted
    pub plan_description: String,
    /// What reality actually produced
    pub reality_delta: String,
    /// Convergence verdict
    pub convergence: Convergence,
    /// Drift severity
    pub severity: DriftSeverity,
    /// Which organ witnessed the drift (GEOX/WEALTH/WELL)
    pub witness_organ: String,
    /// Which floor is implicated (F1-F13)
    pub governance_floor: Option<String>,
    /// Hypothesis for why drift occurred
    pub hypothesis: Option<String>,
    /// Timestamp (epoch millis)
    pub timestamp_ms: u64,
}

impl CoolingEntry {
    pub fn new(
        super_step: u64,
        plan: impl Into<String>,
        reality: impl Into<String>,
        convergence: Convergence,
        severity: DriftSeverity,
        witness_organ: impl Into<String>,
    ) -> Self {
        Self {
            super_step,
            plan_description: plan.into(),
            reality_delta: reality.into(),
            convergence,
            severity,
            witness_organ: witness_organ.into(),
            governance_floor: None,
            hypothesis: None,
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }

    /// Attach a governance floor reference
    pub fn with_floor(mut self, floor: impl Into<String>) -> Self {
        self.governance_floor = Some(floor.into());
        self
    }

    /// Attach a hypothesis
    pub fn with_hypothesis(mut self, hypothesis: impl Into<String>) -> Self {
        self.hypothesis = Some(hypothesis.into());
        self
    }
}

/// Cooling Ledger — append-only drift record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoolingLedger {
    entries: Vec<CoolingEntry>,
    /// Consecutive diverging steps — triggers alert
    divergence_streak: u64,
    /// Alert threshold for divergence streak
    alert_threshold: u64,
}

impl CoolingLedger {
    pub fn new(alert_threshold: u64) -> Self {
        Self {
            entries: Vec::new(),
            divergence_streak: 0,
            alert_threshold,
        }
    }

    /// Record a cooling entry (append-only)
    pub fn record(&mut self, entry: CoolingEntry) {
        if entry.convergence.is_diverging() {
            self.divergence_streak += 1;
        } else {
            self.divergence_streak = 0;
        }
        self.entries.push(entry);
    }

    /// Check if divergence streak has crossed the alert threshold
    pub fn should_alert(&self) -> bool {
        self.divergence_streak >= self.alert_threshold
    }

    /// Get the current divergence streak
    pub fn divergence_streak(&self) -> u64 {
        self.divergence_streak
    }

    /// Total entries recorded
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Count entries by convergence state
    pub fn count_by_state(&self, state: Convergence) -> usize {
        self.entries
            .iter()
            .filter(|e| e.convergence == state)
            .count()
    }

    /// Get recent entries (last N)
    pub fn recent(&self, n: usize) -> &[CoolingEntry] {
        let start = self.entries.len().saturating_sub(n);
        &self.entries[start..]
    }

    /// Get all entries
    pub fn all(&self) -> &[CoolingEntry] {
        &self.entries
    }

    /// Summary statistics
    pub fn summary(&self) -> CoolingSummary {
        let total = self.entries.len();
        let diverging = self.count_by_state(Convergence::Diverging);
        let converging = self.count_by_state(Convergence::Converging);

        CoolingSummary {
            total_entries: total,
            diverging_count: diverging,
            converging_count: converging,
            divergence_rate: if total > 0 {
                diverging as f64 / total as f64
            } else {
                0.0
            },
            divergence_streak: self.divergence_streak,
            alert_active: self.should_alert(),
        }
    }
}

impl Default for CoolingLedger {
    fn default() -> Self {
        Self::new(3) // default: alert after 3 consecutive diverging steps
    }
}

/// Summary of cooling ledger state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoolingSummary {
    pub total_entries: usize,
    pub diverging_count: usize,
    pub converging_count: usize,
    pub divergence_rate: f64,
    pub divergence_streak: u64,
    pub alert_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cooling_append_only() {
        let mut ledger = CoolingLedger::new(3);
        assert_eq!(ledger.entry_count(), 0);

        ledger.record(CoolingEntry::new(
            0,
            "plan: output=42",
            "reality: output=42",
            Convergence::Converging,
            DriftSeverity::Low,
            "GEOX",
        ));
        assert_eq!(ledger.entry_count(), 1);
        assert_eq!(ledger.divergence_streak(), 0);
    }

    #[test]
    fn test_divergence_streak_alerts() {
        let mut ledger = CoolingLedger::new(2); // alert after 2

        ledger.record(CoolingEntry::new(
            0,
            "plan",
            "drifted",
            Convergence::Diverging,
            DriftSeverity::Medium,
            "WEALTH",
        ));
        assert!(!ledger.should_alert());

        ledger.record(CoolingEntry::new(
            1,
            "plan",
            "drifted again",
            Convergence::Diverging,
            DriftSeverity::High,
            "WEALTH",
        ));
        assert!(ledger.should_alert());
        assert_eq!(ledger.divergence_streak(), 2);
    }

    #[test]
    fn test_streak_resets_on_convergence() {
        let mut ledger = CoolingLedger::new(3);

        ledger.record(CoolingEntry::new(
            0,
            "plan",
            "drift",
            Convergence::Diverging,
            DriftSeverity::Medium,
            "GEOX",
        ));
        ledger.record(CoolingEntry::new(
            1,
            "plan",
            "drift",
            Convergence::Diverging,
            DriftSeverity::Medium,
            "GEOX",
        ));
        assert_eq!(ledger.divergence_streak(), 2);

        // Convergent step resets streak
        ledger.record(CoolingEntry::new(
            2,
            "plan",
            "match",
            Convergence::Converging,
            DriftSeverity::Low,
            "GEOX",
        ));
        assert_eq!(ledger.divergence_streak(), 0);
        assert!(!ledger.should_alert());
    }

    #[test]
    fn test_cooling_summary() {
        let mut ledger = CoolingLedger::new(3);
        ledger.record(CoolingEntry::new(
            0,
            "p",
            "r",
            Convergence::Converging,
            DriftSeverity::Low,
            "GEOX",
        ));
        ledger.record(CoolingEntry::new(
            1,
            "p",
            "r",
            Convergence::Diverging,
            DriftSeverity::Medium,
            "WEALTH",
        ));
        ledger.record(CoolingEntry::new(
            2,
            "p",
            "r",
            Convergence::Converging,
            DriftSeverity::Low,
            "GEOX",
        ));

        let summary = ledger.summary();
        assert_eq!(summary.total_entries, 3);
        assert_eq!(summary.diverging_count, 1);
        assert_eq!(summary.converging_count, 2);
        assert!((summary.divergence_rate - 1.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_entry_with_floor_and_hypothesis() {
        let entry = CoolingEntry::new(
            5,
            "plan",
            "reality",
            Convergence::Diverging,
            DriftSeverity::High,
            "WEALTH",
        )
        .with_floor("F1")
        .with_hypothesis("Race condition in parallel lanes");

        assert_eq!(entry.governance_floor.as_deref(), Some("F1"));
        assert_eq!(
            entry.hypothesis.as_deref(),
            Some("Race condition in parallel lanes")
        );
        assert_eq!(entry.super_step, 5);
    }
}
