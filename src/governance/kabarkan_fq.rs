// arifFlow governance/kabarkan_fq.rs
// Kabarkan Flow Quotient Instrumentation — Real-time FQ alerting & trending
//
// Extends kabarkan.rs with:
//   - FqAlert: threshold breach events (WATCHING, STUCK, RECOVERED)
//   - FqTrendSnapshot: periodic trend with direction (rising/falling/stable)
//   - FqLaneSnapshot: per-lane FQ breakdown
//   - KabarkanFqInstrument: wires ReceiptStore → KabarkanTracer → alerts
//
// DITEMPA BUKAN DIBERI

use crate::governance::kabarkan::{KabarkanEvent, KabarkanTracer};
use crate::receipt::{FlowQuotient, FlowVerdict, ReceiptStore};
use serde::{Deserialize, Serialize};

// ── FQ Alert Severity ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FqAlertSeverity {
    /// FQ crossed below 1.0 — Watching. Verification cost rising.
    Warning,
    /// FQ crossed below 0.5 — Stuck. mPFC takeover detected.
    Critical,
    /// FQ recovered above threshold after alert.
    Recovered,
}

impl std::fmt::Display for FqAlertSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FqAlertSeverity::Warning => write!(f, "WARNING"),
            FqAlertSeverity::Critical => write!(f, "CRITICAL"),
            FqAlertSeverity::Recovered => write!(f, "RECOVERED"),
        }
    }
}

// ── FQ Trend Direction ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FqTrend {
    /// FQ increasing — improving flow
    Rising,
    /// FQ decreasing — flow degrading
    Falling,
    /// FQ stable within ±0.1
    Stable,
    /// FQ just crossed a threshold
    ThresholdCrossed,
}

impl std::fmt::Display for FqTrend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FqTrend::Rising => write!(f, "RISING"),
            FqTrend::Falling => write!(f, "FALLING"),
            FqTrend::Stable => write!(f, "STABLE"),
            FqTrend::ThresholdCrossed => write!(f, "THRESHOLD_CROSSED"),
        }
    }
}

// ── Extended Kabarkan Events ──────────────────────────────────────────────

/// FQ threshold breach alert — fires when FQ crosses a boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FqAlertEvent {
    /// When the alert fired (Unix timestamp)
    pub timestamp_ns: i64,
    /// Current Flow Quotient value
    pub fq: f64,
    /// FQ verdict at time of alert
    pub verdict: String,
    /// Alert severity
    pub severity: String,
    /// Previous FQ (before the breach)
    pub previous_fq: f64,
    /// Previous verdict
    pub previous_verdict: String,
    /// FQ trend direction
    pub trend: String,
    /// Session identifier
    pub session_id: String,
    /// Step number when alert fired
    pub step_number: u64,
    /// Execute count in window
    pub execute_count: usize,
    /// Verify count in window
    pub verify_count: usize,
    /// Human-readable diagnosis
    pub diagnosis: String,
}

impl FqAlertEvent {
    pub fn new(
        current: &FlowQuotient,
        previous: &FlowQuotient,
        session_id: &str,
        step_number: u64,
    ) -> Self {
        let severity = match current.verdict {
            FlowVerdict::Watching => FqAlertSeverity::Warning,
            FlowVerdict::Stuck => FqAlertSeverity::Critical,
            _ => FqAlertSeverity::Recovered,
        };

        let trend = compute_trend(
            current.quotient.unwrap_or(0.0),
            previous.quotient.unwrap_or(0.0),
        );

        let diagnosis = match current.verdict {
            FlowVerdict::Stuck => format!(
                "mPFC takeover detected. {} execution steps, {} verification steps. \
                 Verification cost ({:.0}ns) exceeds execution cost ({:.0}ns). \
                 Agent is watching itself think.",
                current.execute_count,
                current.verify_count,
                current.verify_cost_ns,
                current.execute_cost_ns,
            ),
            FlowVerdict::Watching => format!(
                "Verification overhead rising. {} execution, {} verification. \
                 FQ={:.2}. Consider routing more through FLAME or reducing verify frequency.",
                current.execute_count,
                current.verify_count,
                current.quotient.unwrap_or(0.0),
            ),
            _ => format!(
                "FQ recovered to {:.2}. {} execution, {} verification.",
                current.quotient.unwrap_or(0.0),
                current.execute_count,
                current.verify_count,
            ),
        };

        Self {
            timestamp_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
            fq: current.quotient.unwrap_or(0.0),
            verdict: current.verdict.to_string(),
            severity: severity.to_string(),
            previous_fq: previous.quotient.unwrap_or(0.0),
            previous_verdict: previous.verdict.to_string(),
            trend: trend.to_string(),
            session_id: session_id.to_string(),
            step_number,
            execute_count: current.execute_count,
            verify_count: current.verify_count,
            diagnosis,
        }
    }
}

/// Periodic FQ snapshot with trend — for cockpit time-series display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FqSnapshotEvent {
    pub timestamp_ns: i64,
    pub fq: f64,
    pub verdict: String,
    pub trend: String,
    pub execute_count: usize,
    pub verify_count: usize,
    pub execute_cost_ns: u64,
    pub verify_cost_ns: u64,
    pub session_id: String,
    pub step_number: u64,
    pub window_size: usize,
}

impl FqSnapshotEvent {
    pub fn new(fq: &FlowQuotient, previous_fq: f64, session_id: &str, step_number: u64) -> Self {
        let trend = compute_trend(fq.quotient.unwrap_or(0.0), previous_fq);
        Self {
            timestamp_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
            fq: fq.quotient.unwrap_or(0.0),
            verdict: fq.verdict.to_string(),
            trend: trend.to_string(),
            execute_count: fq.execute_count,
            verify_count: fq.verify_count,
            execute_cost_ns: fq.execute_cost_ns,
            verify_cost_ns: fq.verify_cost_ns,
            session_id: session_id.to_string(),
            step_number,
            window_size: fq.window_size,
        }
    }
}

/// Per-lane FQ breakdown — for identifying which lanes are bottlenecked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FqLaneEvent {
    pub timestamp_ns: i64,
    pub lane_id: u32,
    pub topology_id: String,
    pub fq: f64,
    pub verdict: String,
    pub execute_count: usize,
    pub verify_count: usize,
    pub session_id: String,
    pub step_number: u64,
}

impl FqLaneEvent {
    pub fn new(
        lane_id: u32,
        topology_id: &str,
        fq: &FlowQuotient,
        session_id: &str,
        step_number: u64,
    ) -> Self {
        Self {
            timestamp_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
            lane_id,
            topology_id: topology_id.to_string(),
            fq: fq.quotient.unwrap_or(0.0),
            verdict: fq.verdict.to_string(),
            execute_count: fq.execute_count,
            verify_count: fq.verify_count,
            session_id: session_id.to_string(),
            step_number,
        }
    }
}

/// FQ × Cooling cross-reference — correlation between flow health and cooling state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FqCoolingCorrelationEvent {
    pub timestamp_ns: i64,
    pub fq: f64,
    pub fq_verdict: String,
    /// Number of active cooling holds in the last window
    pub cooling_holds: u64,
    /// Number of cooling clamps in the last window
    pub cooling_clamps: u64,
    /// Number of cooling bypasses in the last window
    pub cooling_bypasses: u64,
    /// Correlation signal: FQ_RISING_DURING_COOLING | FQ_FALLING_DURING_EXECUTION | NEUTRAL
    pub correlation_signal: String,
    pub session_id: String,
    pub step_number: u64,
}

impl FqCoolingCorrelationEvent {
    pub fn new(
        fq: &FlowQuotient,
        cooling_holds: u64,
        cooling_clamps: u64,
        cooling_bypasses: u64,
        fq_trend: FqTrend,
        session_id: &str,
        step_number: u64,
    ) -> Self {
        let correlation_signal = match (fq_trend, fq.verdict.clone()) {
            (FqTrend::Rising, _) if cooling_holds > 0 || cooling_clamps > 0 => {
                "FQ_RISING_DURING_COOLING — cooling is working, flow recovering".to_string()
            }
            (FqTrend::Falling, FlowVerdict::Watching | FlowVerdict::Stuck)
                if cooling_holds == 0 =>
            {
                "FQ_FALLING_DURING_EXECUTION — agent needs cooling, but none active".to_string()
            }
            (FqTrend::Falling, _) => "FQ_FALLING — verification cost rising".to_string(),
            _ => "NEUTRAL".to_string(),
        };

        Self {
            timestamp_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
            fq: fq.quotient.unwrap_or(0.0),
            fq_verdict: fq.verdict.to_string(),
            cooling_holds,
            cooling_clamps,
            cooling_bypasses,
            correlation_signal,
            session_id: session_id.to_string(),
            step_number,
        }
    }
}

// ── Trend Computation ─────────────────────────────────────────────────────

fn compute_trend(current_fq: f64, previous_fq: f64) -> FqTrend {
    if previous_fq == 0.0 {
        return FqTrend::Stable; // first reading
    }
    let delta = current_fq - previous_fq;
    if delta > 0.1 {
        FqTrend::Rising
    } else if delta < -0.1 {
        FqTrend::Falling
    } else {
        FqTrend::Stable
    }
}

fn threshold_crossed(old_v: &FlowVerdict, new_v: &FlowVerdict) -> bool {
    old_v != new_v && (*new_v == FlowVerdict::Watching || *new_v == FlowVerdict::Stuck)
}

// ── Kabarkan FQ Instrument ────────────────────────────────────────────────

/// Wires ReceiptStore → FlowQuotient → KabarkanTracer → structured events.
///
/// Usage:
/// ```ignore
/// let mut instrument = KabarkanFqInstrument::new(session_id, 20);
/// // ... after pushing receipts to store ...
/// instrument.sample(&mut tracer, step_number);
/// // alerts fire automatically when FQ crosses thresholds
/// ```
pub struct KabarkanFqInstrument {
    session_id: String,
    window_size: usize,
    /// Previous FQ verdict — used to detect threshold crossings
    previous_verdict: FlowVerdict,
    /// Previous FQ value — used for trend computation
    previous_fq: f64,
    /// Snapshot counter — emit full snapshot every N samples
    sample_counter: u64,
    /// How often to emit full snapshots (in samples)
    snapshot_interval: u64,
    /// Cooling activity counters for correlation
    cooling_holds: u64,
    cooling_clamps: u64,
    cooling_bypasses: u64,
}

impl KabarkanFqInstrument {
    pub fn new(session_id: &str, window_size: usize) -> Self {
        Self {
            session_id: session_id.to_string(),
            window_size,
            previous_verdict: FlowVerdict::Balanced,
            previous_fq: 0.0,
            sample_counter: 0,
            snapshot_interval: 5, // emit full snapshot every 5 samples
            cooling_holds: 0,
            cooling_clamps: 0,
            cooling_bypasses: 0,
        }
    }

    /// Set how often full snapshots are emitted (in samples).
    pub fn with_snapshot_interval(mut self, interval: u64) -> Self {
        self.snapshot_interval = interval;
        self
    }

    /// Record cooling activity for correlation analysis.
    pub fn record_cooling(&mut self, holds: u64, clamps: u64, bypasses: u64) {
        self.cooling_holds = holds;
        self.cooling_clamps = clamps;
        self.cooling_bypasses = bypasses;
    }

    /// Sample FQ from the ReceiptStore and emit events through the tracer.
    /// Call this at every super-step boundary.
    pub fn sample(&mut self, store: &ReceiptStore, tracer: &mut KabarkanTracer, step_number: u64) {
        let current_fq = store.flow_quotient(self.window_size);
        self.sample_counter += 1;

        let trend = compute_trend(current_fq.quotient.unwrap_or(0.0), self.previous_fq);

        // ── Always emit lightweight AFQ snapshot (backward compat) ──
        tracer.emit(KabarkanEvent::afq_snapshot(step_number, &current_fq));

        // ── FQ Alert: emit on threshold breach ──
        if threshold_crossed(&self.previous_verdict, &current_fq.verdict) {
            let previous_fq = FlowQuotient {
                quotient: Some(self.previous_fq),
                verdict: self.previous_verdict.clone(),
                execute_count: 0,
                verify_count: 0,
                execute_cost_ns: 0,
                verify_cost_ns: 0,
                window_size: 0,
                apex_block: None,
            };
            let alert = FqAlertEvent::new(&current_fq, &previous_fq, &self.session_id, step_number);
            // Serialize into Kabarkan event envelope
            let alert_json = serde_json::to_value(&alert).unwrap_or_default();
            tracer.emit(KabarkanEvent::FqAlert {
                step: step_number,
                fq: current_fq.quotient.unwrap_or(0.0),
                verdict: current_fq.verdict.to_string(),
                severity: alert.severity.clone(),
                diagnosis: alert.diagnosis.clone(),
                payload: alert_json,
            });
        }

        // ── Periodic full snapshot ──
        if self.sample_counter % self.snapshot_interval == 0 {
            let snapshot =
                FqSnapshotEvent::new(&current_fq, self.previous_fq, &self.session_id, step_number);
            let snap_json = serde_json::to_value(&snapshot).unwrap_or_default();
            tracer.emit(KabarkanEvent::FqSnapshot {
                step: step_number,
                fq: current_fq.quotient.unwrap_or(0.0),
                verdict: current_fq.verdict.to_string(),
                trend: trend.to_string(),
                execute_count: current_fq.execute_count,
                verify_count: current_fq.verify_count,
                payload: snap_json,
            });
        }

        // ── FQ × Cooling correlation (on cooling activity) ──
        if self.cooling_holds > 0 || self.cooling_clamps > 0 || self.cooling_bypasses > 0 {
            let correlation = FqCoolingCorrelationEvent::new(
                &current_fq,
                self.cooling_holds,
                self.cooling_clamps,
                self.cooling_bypasses,
                trend,
                &self.session_id,
                step_number,
            );
            let corr_json = serde_json::to_value(&correlation).unwrap_or_default();
            tracer.emit(KabarkanEvent::FqCoolingCorrelation {
                step: step_number,
                fq: current_fq.quotient.unwrap_or(0.0),
                correlation_signal: correlation.correlation_signal.clone(),
                payload: corr_json,
            });
        }

        // ── Update state for next sample ──
        self.previous_verdict = current_fq.verdict.clone();
        self.previous_fq = current_fq.quotient.unwrap_or(0.0);
    }

    /// Emit per-lane FQ for a specific lane.
    pub fn sample_lane(
        &self,
        lane_id: u32,
        topology_id: &str,
        lane_receipts: &[crate::receipt::FlowReceipt],
        tracer: &mut KabarkanTracer,
        step_number: u64,
    ) {
        let lane_fq = FlowQuotient::compute(lane_receipts);
        let lane_event = FqLaneEvent::new(
            lane_id,
            topology_id,
            &lane_fq,
            &self.session_id,
            step_number,
        );
        let lane_json = serde_json::to_value(&lane_event).unwrap_or_default();
        tracer.emit(KabarkanEvent::FqLaneSnapshot {
            step: step_number,
            lane_id,
            topology_id: topology_id.to_string(),
            fq: lane_fq.quotient.unwrap_or(0.0),
            verdict: lane_fq.verdict.to_string(),
            payload: lane_json,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trend_rising() {
        assert_eq!(compute_trend(3.5, 2.0), FqTrend::Rising);
    }

    #[test]
    fn test_trend_falling() {
        assert_eq!(compute_trend(0.4, 0.9), FqTrend::Falling);
    }

    #[test]
    fn test_trend_stable() {
        assert_eq!(compute_trend(1.05, 1.0), FqTrend::Stable);
        assert_eq!(compute_trend(0.95, 1.0), FqTrend::Stable);
    }

    #[test]
    fn test_trend_first_reading() {
        assert_eq!(compute_trend(1.5, 0.0), FqTrend::Stable);
    }

    #[test]
    fn test_threshold_crossed_to_watching() {
        assert!(threshold_crossed(
            &FlowVerdict::Balanced,
            &FlowVerdict::Watching
        ));
        assert!(threshold_crossed(
            &FlowVerdict::Optimal,
            &FlowVerdict::Watching
        ));
    }

    #[test]
    fn test_threshold_crossed_to_stuck() {
        assert!(threshold_crossed(
            &FlowVerdict::Watching,
            &FlowVerdict::Stuck
        ));
        assert!(threshold_crossed(
            &FlowVerdict::Balanced,
            &FlowVerdict::Stuck
        ));
    }

    #[test]
    fn test_threshold_not_crossed_within_same_band() {
        assert!(!threshold_crossed(
            &FlowVerdict::Optimal,
            &FlowVerdict::Balanced
        ));
        assert!(!threshold_crossed(
            &FlowVerdict::Balanced,
            &FlowVerdict::Optimal
        ));
    }

    #[test]
    fn test_alert_event_diagnosis_stuck() {
        let current = FlowQuotient {
            execute_count: 10,
            verify_count: 30,
            execute_cost_ns: 1_000_000,
            verify_cost_ns: 3_000_000,
            quotient: Some(0.33),
            verdict: FlowVerdict::Stuck,
            window_size: 20,
            apex_block: None,
        };
        let previous = FlowQuotient {
            execute_count: 0,
            verify_count: 0,
            execute_cost_ns: 0,
            verify_cost_ns: 0,
            quotient: Some(0.8),
            verdict: FlowVerdict::Watching,
            window_size: 0,
            apex_block: None,
        };
        let alert = FqAlertEvent::new(&current, &previous, "test-session", 42);
        assert_eq!(alert.severity, "CRITICAL");
        assert!(alert.diagnosis.contains("mPFC takeover"));
    }

    #[test]
    fn test_cooling_correlation_rising_during_cooling() {
        let fq = FlowQuotient {
            execute_count: 20,
            verify_count: 5,
            execute_cost_ns: 5_000_000,
            verify_cost_ns: 1_000_000,
            quotient: Some(5.0),
            verdict: FlowVerdict::Optimal,
            window_size: 20,
            apex_block: None,
        };
        let event = FqCoolingCorrelationEvent::new(&fq, 2, 1, 0, FqTrend::Rising, "test", 42);
        assert!(event
            .correlation_signal
            .contains("FQ_RISING_DURING_COOLING"));
    }

    #[test]
    fn test_cooling_correlation_falling_no_cooling() {
        let fq = FlowQuotient {
            execute_count: 5,
            verify_count: 15,
            execute_cost_ns: 1_000_000,
            verify_cost_ns: 3_000_000,
            quotient: Some(0.4),
            verdict: FlowVerdict::Stuck,
            window_size: 20,
            apex_block: None,
        };
        let event = FqCoolingCorrelationEvent::new(&fq, 0, 0, 0, FqTrend::Falling, "test", 42);
        assert!(event
            .correlation_signal
            .contains("FQ_FALLING_DURING_EXECUTION"));
    }

    #[test]
    fn test_instrument_sample_emits_events() {
        let mut store = ReceiptStore::new(100);
        let mut tracer = KabarkanTracer::new(true);
        let mut instrument =
            KabarkanFqInstrument::new("test-session", 20).with_snapshot_interval(3);

        // Push some execute receipts
        use crate::receipt::{EpistemicLabel, FlowReceipt, StepType};
        for i in 0..5 {
            let r = if i == 0 {
                FlowReceipt::new_first(
                    "agent",
                    "test-session",
                    StepType::Execute,
                    EpistemicLabel::Observation,
                    1_000_000,
                )
            } else {
                let prev = store.all().last().unwrap();
                FlowReceipt::new_chained(
                    prev,
                    "agent",
                    "test-session",
                    StepType::Execute,
                    EpistemicLabel::Observation,
                    500_000,
                )
            };
            store.push(r).unwrap();
        }

        // Sample — should emit AFQ snapshot
        let before = tracer.event_count();
        instrument.sample(&store, &mut tracer, 1);
        let after = tracer.event_count();
        assert!(after > before, "Should have emitted at least one event");

        // Drain and verify event types
        let events = tracer.drain_events();
        let has_afq = events
            .iter()
            .any(|e| matches!(e, KabarkanEvent::AfqSnapshot { .. }));
        assert!(has_afq, "Should emit AFQ snapshot on every sample");
    }
}
