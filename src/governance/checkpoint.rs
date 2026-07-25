// arifFlow governance/checkpoint.rs
// Checkpoint Manager — Per-super-step state persistence with authority re-verification

use crate::scheduler::VerdictClass;
use crate::CheckpointEnvelope;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CheckpointError {
    #[error("Checkpoint not found for step {0}")]
    NotFound(u64),
    #[error("Constitutional chain re-verification failed")]
    ChainInvalid(String),
    #[error("Storage error: {0}")]
    Storage(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointState {
    Pending,
    Sealed,
    Invalidated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointRecord {
    pub step: u64,
    pub envelope: CheckpointEnvelope,
    pub state: CheckpointState,
    pub created_at_ns: u64,
}

pub struct CheckpointManager {
    checkpoints: Vec<CheckpointRecord>,
    strict_verification: bool,
}

impl CheckpointManager {
    pub fn new(strict: bool) -> Self {
        Self {
            checkpoints: Vec::new(),
            strict_verification: strict,
        }
    }

    pub fn write_checkpoint(
        &mut self,
        step: u64,
        envelope: CheckpointEnvelope,
    ) -> CheckpointRecord {
        let record = CheckpointRecord {
            step,
            envelope,
            state: CheckpointState::Pending,
            created_at_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
        };
        self.checkpoints.push(record.clone());
        record
    }

    pub fn mark_sealed(&mut self, step: u64) -> Result<(), CheckpointError> {
        self.checkpoints
            .iter_mut()
            .find(|r| r.step == step)
            .map(|r| r.state = CheckpointState::Sealed)
            .ok_or(CheckpointError::NotFound(step))
    }

    pub fn mark_invalidated(&mut self, step: u64) -> Result<(), CheckpointError> {
        self.checkpoints
            .iter_mut()
            .find(|r| r.step == step)
            .map(|r| r.state = CheckpointState::Invalidated)
            .ok_or(CheckpointError::NotFound(step))
    }

    pub fn restore(&self, step: u64) -> Result<&CheckpointRecord, CheckpointError> {
        let record = self.checkpoints.iter().find(|r| r.step == step);
        match record {
            Some(r) => {
                if self.strict_verification && r.state == CheckpointState::Invalidated {
                    return Err(CheckpointError::ChainInvalid(
                        r.envelope.constitutional_chain_id.to_string(),
                    ));
                }
                Ok(r)
            }
            None => Err(CheckpointError::NotFound(step)),
        }
    }

    pub fn all_checkpoints(&self) -> &[CheckpointRecord] {
        &self.checkpoints
    }

    pub fn latest_checkpoint(&self) -> Option<&CheckpointRecord> {
        self.checkpoints.last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle::MerkleRoot;
    use crate::scheduler::VerdictClass;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn make_envelope(step: u64) -> CheckpointEnvelope {
        CheckpointEnvelope {
            actor_id: "arif".into(),
            lease_id: Uuid::new_v4(),
            constitutional_chain_id: Uuid::new_v4(),
            super_step: step,
            channel_roots: BTreeMap::new(),
            state_root: MerkleRoot([step as u8; 32]),
            verdict_id: None,
            verdict_class: VerdictClass::SEAL,
            arifos_verdict_hash: MerkleRoot::ZERO,
            timestamp_ns: 0,
            previous_checkpoint_hash: MerkleRoot::ZERO,
            checkpoint_hash: MerkleRoot::ZERO,
        }
    }

    #[test]
    fn test_checkpoint_write_restore() {
        let mut mgr = CheckpointManager::new(true);
        mgr.write_checkpoint(0, make_envelope(0));
        let restored = mgr.restore(0).unwrap();
        assert_eq!(restored.step, 0);
        assert_eq!(restored.state, CheckpointState::Pending);
    }

    #[test]
    fn test_checkpoint_invalidated_rejected() {
        let mut mgr = CheckpointManager::new(true);
        mgr.write_checkpoint(0, make_envelope(0));
        mgr.mark_invalidated(0).unwrap();
        assert!(mgr.restore(0).is_err());
    }

    #[test]
    fn test_checkpoint_not_found() {
        let mgr = CheckpointManager::new(true);
        assert!(mgr.restore(99).is_err());
    }
}
