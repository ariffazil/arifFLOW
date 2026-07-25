// arifFlow governance/kabarkan.rs
// Kabarkan Tracing Hooks — Per-super-step observability events

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
    CoolingReceipt {
        actor_id: String,
        lease_id: String,
        total_steps: u64,
        final_state_root: [u8; 32],
    },
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
