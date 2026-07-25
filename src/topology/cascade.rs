// arifFlow topology/cascade.rs
// Cascade Topology: Multi-agent handoff with F3 witness audit
//
// Topology:
//   Hermes ──▶ SubAgent A ──▶ SubAgent B ──▶ Synthesis ──▶ F3 Witness ──▶ SEAL

use serde::{Deserialize, Serialize};
use crate::channel::{Channel, ChannelId, ChannelMode};
use crate::scheduler::TopologyKind;
use super::TopologyError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeStep {
    pub agent_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub requires_witness: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeConfig {
    pub steps: Vec<CascadeStep>,
    pub final_witness: bool,
}

pub struct CascadeTopology {
    config: CascadeConfig,
    current_step: usize,
}

impl CascadeTopology {
    pub fn new(config: CascadeConfig) -> Self {
        Self {
            current_step: 0,
            config,
        }
    }

    pub fn next_step(&mut self) -> Option<&CascadeStep> {
        let step = self.config.steps.get(self.current_step);
        if step.is_some() {
            self.current_step += 1;
        }
        step
    }

    pub fn is_complete(&self) -> bool {
        self.current_step >= self.config.steps.len()
    }

    pub fn needs_final_witness(&self) -> bool {
        self.config.final_witness && self.is_complete()
    }

    pub fn config(&self) -> &CascadeConfig {
        &self.config
    }

    pub fn reset(&mut self) {
        self.current_step = 0;
    }
}
