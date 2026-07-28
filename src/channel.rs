// arifFlow core/channel.rs
// Channel<T> — Governed message passing between super-step nodes
//
// Invariant A2 (Plane-Isolated): State crosses planes only via signed envelopes.
// No raw memory sharing between execution plane and intelligence plane.

use crate::merkle::MerkleRoot;
use crate::receipt::{EpistemicLabel, FlowReceipt, StepType};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Unique identifier for a channel within a topology run
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChannelId(pub String);

impl fmt::Display for ChannelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ch_{}", self.0)
    }
}

/// A single message within a channel, content-hashed at creation,
/// carrying a Flow Receipt v1 for governed provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message<T: Serialize + Clone> {
    /// The payload — type-erased at channel level
    pub payload: T,
    /// Epoch (super-step counter) when this message was produced
    pub epoch: u64,
    /// blake3 hash of (payload_bytes || epoch) — self-authenticating
    pub content_hash: [u8; 32],
    /// Flow Receipt v1 — the minimal verifiable unit of governed flow
    pub receipt: FlowReceipt,
}

impl<T: Serialize + Clone> Message<T> {
    pub fn new(payload: T, epoch: u64) -> Result<Self, bincode::Error> {
        let payload_bytes = bincode::serialize(&payload)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(&payload_bytes);
        hasher.update(&epoch.to_le_bytes());
        let hash = hasher.finalize();

        // Create a minimal flow receipt for this message
        let receipt = FlowReceipt::new_first(
            "channel::unknown",
            "session",
            StepType::Route,
            EpistemicLabel::Observation,
            0,
        );

        Ok(Self {
            payload,
            epoch,
            content_hash: *hash.as_bytes(),
            receipt,
        })
    }

    pub fn verify(&self) -> Result<bool, bincode::Error> {
        let payload_bytes = bincode::serialize(&self.payload)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(&payload_bytes);
        hasher.update(&self.epoch.to_le_bytes());
        let expected = *hasher.finalize().as_bytes();
        Ok(expected == self.content_hash)
    }
}

/// Channel capacity mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelMode {
    /// Fixed capacity — back-pressure when full (fan-out nodes)
    Bounded(usize),
    /// Streaming — grows as needed (pipeline stages)
    Unbounded,
}

/// Channel errors
#[derive(Debug, Error)]
pub enum ChannelError {
    #[error("Channel {0} is full (capacity {1})")]
    Full(ChannelId, usize),
    #[error("Channel {0} is closed")]
    Closed(ChannelId),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Content hash mismatch — message tampered or corrupted")]
    HashMismatch,
}

/// A governed message channel.
///
/// Channels are the ONLY communication primitive between super-step nodes.
/// All messages are content-hashed at creation and verified at consumption.
/// This enforces A2 (Plane-Isolated) — no raw pointers cross plane boundaries.
#[derive(Debug, Clone)]
pub struct Channel<T: Serialize + Clone> {
    id: ChannelId,
    mode: ChannelMode,
    /// Internal message buffer — private to enforce hash verification
    buffer: Vec<Message<T>>,
    /// Closed channels reject new messages (A5: Metabolic-Closure)
    closed: bool,
    /// Monotonic write counter
    write_count: u64,
}

impl<T: Serialize + Clone> Channel<T> {
    pub fn new(id: ChannelId, mode: ChannelMode) -> Self {
        Self {
            id,
            mode,
            buffer: Vec::new(),
            closed: false,
            write_count: 0,
        }
    }

    pub fn id(&self) -> &ChannelId {
        &self.id
    }

    /// Write a message into the channel. Messages are content-hashed atomically.
    /// Errors if channel is full (bounded) or closed.
    pub fn write(&mut self, payload: T) -> Result<Message<T>, ChannelError> {
        if self.closed {
            return Err(ChannelError::Closed(self.id.clone()));
        }
        if let ChannelMode::Bounded(cap) = self.mode {
            if self.buffer.len() >= cap {
                return Err(ChannelError::Full(self.id.clone(), cap));
            }
        }
        let epoch = self.write_count;
        self.write_count += 1;
        let msg =
            Message::new(payload, epoch).map_err(|e| ChannelError::Serialization(e.to_string()))?;
        self.buffer.push(msg.clone());
        Ok(msg)
    }

    /// Read all messages, verifying each content hash.
    /// Returns an error if ANY message has been tampered with (A4: Verifiable-Reduction).
    pub fn read_all(&self) -> Result<Vec<&Message<T>>, ChannelError> {
        let mut result = Vec::with_capacity(self.buffer.len());
        for msg in &self.buffer {
            if !msg.verify().unwrap_or(false) {
                return Err(ChannelError::HashMismatch);
            }
            result.push(msg);
        }
        Ok(result)
    }

    /// Drain all messages — empties the buffer (for merge operations).
    /// Verifies hashes before draining.
    pub fn drain(&mut self) -> Result<Vec<Message<T>>, ChannelError> {
        // Verify all before draining
        let _ = self.read_all()?;
        Ok(std::mem::take(&mut self.buffer))
    }

    /// Close the channel. No further writes allowed (A5: Metabolic-Closure).
    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Current Merkle root of the channel's message buffer
    pub fn merkle_root(&self) -> MerkleRoot {
        let mut hasher = blake3::Hasher::new();
        for msg in &self.buffer {
            hasher.update(&msg.content_hash);
        }
        MerkleRoot(*hasher.finalize().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_write_read() {
        let mut ch = Channel::new(ChannelId("test".into()), ChannelMode::Unbounded);
        let msg = ch.write("hello".to_string()).unwrap();
        assert!(msg.verify().unwrap());
        let msgs = ch.read_all().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].payload, "hello");
    }

    #[test]
    fn test_channel_hash_mismatch_detected() {
        let mut ch = Channel::new(ChannelId("test2".into()), ChannelMode::Unbounded);
        let _ = ch.write("data".to_string()).unwrap();
        // Tamper with the buffer directly (simulating corruption)
        ch.buffer[0].content_hash = [0u8; 32];
        let result = ch.read_all();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ChannelError::HashMismatch));
    }

    #[test]
    fn test_bounded_channel_backpressure() {
        let mut ch = Channel::new(ChannelId("bounded".into()), ChannelMode::Bounded(2));
        assert!(ch.write("a".to_string()).is_ok());
        assert!(ch.write("b".to_string()).is_ok());
        assert!(matches!(
            ch.write("c".to_string()),
            Err(ChannelError::Full(_, 2))
        ));
    }

    #[test]
    fn test_closed_channel_rejects_writes() {
        let mut ch = Channel::new(ChannelId("closed".into()), ChannelMode::Unbounded);
        ch.close();
        assert!(matches!(
            ch.write("data".to_string()),
            Err(ChannelError::Closed(_))
        ));
    }
}
