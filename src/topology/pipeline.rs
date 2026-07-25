// arifFlow topology/pipeline.rs
// Pipeline Topology: Sequential stages with optional review loop
//
// Topology:
//   Input ──▶ Stage 1 ──▶ Stage 2 ──▶ Stage 3 ──▶ Output
//                ↑────────── Review ────────↓

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStage {
    pub id: String,
    pub max_retries: u32,
    pub requires_review: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub stages: Vec<PipelineStage>,
    pub review_every_n: u32,
}

pub struct PipelineTopology {
    config: PipelineConfig,
    current_stage: usize,
    iteration: u32,
}

impl PipelineTopology {
    pub fn new(config: PipelineConfig) -> Self {
        Self {
            current_stage: 0,
            iteration: 0,
            config,
        }
    }

    pub fn next_stage(&mut self) -> Option<&PipelineStage> {
        let stage = self.config.stages.get(self.current_stage);
        if stage.is_some() {
            self.current_stage += 1;
        }
        stage
    }

    pub fn should_review(&self) -> bool {
        self.iteration > 0 && self.iteration % self.config.review_every_n == 0
    }

    pub fn advance_iteration(&mut self) {
        self.iteration += 1;
        self.current_stage = 0;
    }

    pub fn is_complete(&self) -> bool {
        self.current_stage >= self.config.stages.len()
    }

    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }
}
