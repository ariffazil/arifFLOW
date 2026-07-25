// arifFlow governance/kabarkan.rs
// Kabarkan Tracing Hooks — Per-super-step observability events

use crate::receipt::FlowQuotient;
use crate::scheduler::TopologyKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum KabarkanEvent {
    SuperStepStarted {
        step: u64,
        topology: String,
        lease_id: String,
        actor_id: String,
    },
    SuperStepCompleted {
        step: u64,
        state_root: [u8; 32],
        verdict_id: String,
        verdict_class: String,
    },
    DivergentMerge {
        topology: String,
        step: u64,
        expected_hash: [u8; 32],
        actual_hash: [u8; 32],
    },
    ExecutionHeld {
        step: u64,
        lease_id: String,
        verdict: String,
    },
    AfqSnapshot {
        step: u64,
        execution_steps: u64,
        governance_steps: u64,
        afq: f64,
        diagnosis: String,
    },
    /// FQ threshold breach alert — WARNING (< 1.0), CRITICAL (< 0.5), RECOVERED
    FqAlert {
        step: u64,
        fq: f64,
        verdict: String,
        severity: String,
        diagnosis: String,
        /// Full FqAlertEvent as JSON payload
        payload: serde_json::Value,
    },
    /// Periodic FQ snapshot with trend direction
    FqSnapshot {
        step: u64,
        fq: f64,
        verdict: String,
        trend: String,
        execute_count: usize,
        verify_count: usize,
        /// Full FqSnapshotEvent as JSON payload
        payload: serde_json::Value,
    },
    /// Per-lane FQ breakdown
    FqLaneSnapshot {
        step: u64,
        lane_id: u32,
        topology_id: String,
        fq: f64,
        verdict: String,
        /// Full FqLaneEvent as JSON payload
        payload: serde_json::Value,
    },
    /// FQ × Cooling correlation analysis
    FqCoolingCorrelation {
        step: u64,
        fq: f64,
        correlation_signal: String,
        /// Full FqCoolingCorrelationEvent as JSON payload
        payload: serde_json::Value,
    },
    CoolingReceipt {
        actor_id: String,
        lease_id: String,
        total_steps: u64,
        final_state_root: [u8; 32],
    },
}

impl KabarkanEvent {
    pub fn afq_snapshot(step: u64, fq: &FlowQuotient) -> Self {
        KabarkanEvent::AfqSnapshot {
            step,
            execution_steps: fq.execute_count as u64,
            governance_steps: fq.verify_count as u64,
            afq: fq.quotient,
            diagnosis: fq.verdict.to_string(),
        }
    }
}

pub struct KabarkanTracer {
    enabled: bool,
    events: Vec<KabarkanEvent>,
}

impl KabarkanTracer {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            events: Vec::new(),
        }
    }

    pub fn emit(&mut self, event: KabarkanEvent) {
        if self.enabled {
            self.events.push(event);
        }
    }

    pub fn drain_events(&mut self) -> Vec<KabarkanEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}
