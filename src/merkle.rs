// arifFlow core/merkle.rs
// Merkle State Hasher — Per-plane state tree with content-addressed proofs
//
// Invariant A3 (Checkpoint-with-Verdict): Each super-step records a Merkle root
// that binds state, actor, lease, and verdict into a single hash chain.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// A 32-byte Merkle root (blake3 output)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MerkleRoot(pub [u8; 32]);

impl MerkleRoot {
    pub const ZERO: MerkleRoot = MerkleRoot([0u8; 32]);

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let hash = blake3::hash(bytes);
        Self(*hash.as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::LowerHex for MerkleRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl fmt::Display for MerkleRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:x}", self)
    }
}

/// Compute content hash for a serializable payload
pub fn content_hash<T: Serialize>(payload: &T) -> Result<MerkleRoot, bincode::Error> {
    let bytes = bincode::serialize(payload)?;
    Ok(MerkleRoot::from_bytes(&bytes))
}

/// Chain two Merkle roots (for sequential checkpoint binding)
pub fn chain_roots(prev: &MerkleRoot, curr: &MerkleRoot) -> MerkleRoot {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"ariflow_chain");
    hasher.update(&prev.0);
    hasher.update(&curr.0);
    MerkleRoot(*hasher.finalize().as_bytes())
}

/// Merkle tree for a plane's state at a super-step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleTree {
    leaves: Vec<MerkleRoot>,
    root: MerkleRoot,
    leaf_count: usize,
}

impl MerkleTree {
    pub fn from_leaves(leaves: Vec<MerkleRoot>) -> Result<Self, MerkleError> {
        let leaf_count = leaves.len();
        let root = match leaf_count {
            0 => MerkleRoot::ZERO,
            1 => {
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"ariflow_merkle_single");
                hasher.update(&leaves[0].0);
                MerkleRoot(*hasher.finalize().as_bytes())
            }
            _ => Self::compute_root(&leaves),
        };
        Ok(Self { leaves, root, leaf_count })
    }

    pub fn from_channels(
        channels: &std::collections::BTreeMap<String, MerkleRoot>,
    ) -> Result<Self, MerkleError> {
        let leaves: Vec<MerkleRoot> = channels.values().copied().collect();
        Self::from_leaves(leaves)
    }

    fn compute_root(leaves: &[MerkleRoot]) -> MerkleRoot {
        if leaves.is_empty() {
            return MerkleRoot::ZERO;
        }
        if leaves.len() == 1 {
            return leaves[0];
        }
        let mut current: Vec<MerkleRoot> = leaves.to_vec();
        while current.len() > 1 {
            let mut next = Vec::with_capacity((current.len() + 1) / 2);
            for chunk in current.chunks(2) {
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"ariflow_merkle_node");
                hasher.update(&chunk[0].0);
                if chunk.len() > 1 {
                    hasher.update(&chunk[1].0);
                } else {
                    hasher.update(&chunk[0].0);
                }
                next.push(MerkleRoot(*hasher.finalize().as_bytes()));
            }
            current = next;
        }
        current[0]
    }

    pub fn bind_authority(&self, lease_id: &uuid::Uuid, actor_id: &str) -> MerkleRoot {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ariflow_authority_binding");
        hasher.update(&self.root.0);
        hasher.update(lease_id.to_string().as_bytes());
        hasher.update(actor_id.as_bytes());
        MerkleRoot(*hasher.finalize().as_bytes())
    }

    pub fn root(&self) -> &MerkleRoot {
        &self.root
    }

    pub fn leaf_count(&self) -> usize {
        self.leaf_count
    }

    pub fn is_zero(&self) -> bool {
        self.root == MerkleRoot::ZERO
    }
}

#[derive(Debug, Error)]
pub enum MerkleError {
    #[error("No leaves provided")]
    NoLeaves,
    #[error("Serialization error: {0}")]
    Serialization(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_merkle_root_single_leaf() {
        let leaf = MerkleRoot::from_bytes(b"hello");
        let tree = MerkleTree::from_leaves(vec![leaf]).unwrap();
        assert!(!tree.is_zero());
        assert_eq!(tree.leaf_count(), 1);
    }

    #[test]
    fn test_merkle_root_empty() {
        let tree = MerkleTree::from_leaves(vec![]).unwrap();
        assert!(tree.is_zero());
    }

    #[test]
    fn test_merkle_root_multi_leaf() {
        let leaves: Vec<MerkleRoot> = (0..4).map(|i| MerkleRoot::from_bytes(&[i])).collect();
        let tree = MerkleTree::from_leaves(leaves).unwrap();
        assert!(!tree.is_zero());
        assert_eq!(tree.leaf_count(), 4);
    }

    #[test]
    fn test_from_channels_btreemap() {
        let mut channels = BTreeMap::new();
        channels.insert("ch_geo".into(), MerkleRoot([1u8; 32]));
        channels.insert("ch_wealth".into(), MerkleRoot([2u8; 32]));
        channels.insert("ch_well".into(), MerkleRoot([3u8; 32]));
        let tree = MerkleTree::from_channels(&channels).unwrap();
        assert_eq!(tree.leaf_count(), 3);
    }

    #[test]
    fn test_authority_binding() {
        let leaf = MerkleRoot([42u8; 32]);
        let tree = MerkleTree::from_leaves(vec![leaf]).unwrap();
        let lease_id = uuid::Uuid::new_v4();
        let bound = tree.bind_authority(&lease_id, "arif");
        assert_ne!(bound, leaf);
        assert_ne!(bound, MerkleRoot::ZERO);
    }

    #[test]
    fn test_content_hash_roundtrip() {
        let data = "hello arifFlow";
        let hash = content_hash(&data).unwrap();
        assert_ne!(hash, MerkleRoot::ZERO);
        let hash2 = content_hash(&data).unwrap();
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_chain_roots() {
        let prev = MerkleRoot([1u8; 32]);
        let curr = MerkleRoot([2u8; 32]);
        let chained = chain_roots(&prev, &curr);
        assert_ne!(chained, prev);
        assert_ne!(chained, curr);
        let chained_rev = chain_roots(&curr, &prev);
        assert_ne!(chained, chained_rev);
    }
}
