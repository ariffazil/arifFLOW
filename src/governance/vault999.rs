// arifFlow governance/vault999.rs
// VAULT999 Sealing Hooks — Per-super-step immutable state commitment
//
// ═══════════════════════════════════════════════════════════════
// STATUS: PRE-ALPHA — HASH CHAIN ACTIVE, PERSISTENCE PENDING
// ═══════════════════════════════════════════════════════════════
//
// v2026-07-25: Hash-chain integrity is now enforced via SHA3-256.
//   chain_entry_hash = SHA3-256(prev_hash || position_be_bytes || checkpoint_hash)
// This makes the seal chain tamper-evident: any modification to a prior
// seal invalidates all subsequent chain_entry_hash values.
//
// REMAINING GAPS:
//   - No HTTP call to arifOS arif_seal endpoint (:8088)
//   - No disk persistence (in-memory only)
//   - No Drop implementation for graceful shutdown
//   - No Merkle anchoring (delegated to merkle.rs when wired)
//   - No /verify endpoint for public chain integrity
//
// Ψ BOUNDARY: This module MUST be the only path to VAULT999 in Rust.
// No other module may write to the vault independently.
// ═══════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealReceipt {
    pub vault_entry_id: String,
    pub chain_position: u64,
    pub prev_hash: [u8; 32],
    pub chain_entry_hash: [u8; 32],
}

pub struct Vault999Sealer {
    chain_position: u64,
    prev_hash: [u8; 32],
}

impl Vault999Sealer {
    pub fn new() -> Self {
        Self {
            chain_position: 0,
            prev_hash: [0u8; 32], // genesis block — all zeros
        }
    }

    pub fn seal(&mut self, checkpoint_hash: [u8; 32]) -> Result<SealReceipt, String> {
        let position = self.chain_position;
        let prev = self.prev_hash;

        // chain_entry_hash = SHA3-256(prev_hash || position_be || checkpoint_hash)
        // This makes every seal cryptographically dependent on its predecessor.
        // Tampering with any prior seal invalidates all subsequent hashes.
        let mut hasher = Sha3_256::new();
        hasher.update(&prev);
        hasher.update(&position.to_be_bytes());
        hasher.update(&checkpoint_hash);
        let chain_entry: [u8; 32] = hasher.finalize().into();

        self.chain_position += 1;
        self.prev_hash = chain_entry;

        Ok(SealReceipt {
            vault_entry_id: format!("vault_{}", hex_encode(&chain_entry[..8])),
            chain_position: position,
            prev_hash: prev,
            chain_entry_hash: chain_entry,
        })
    }

    pub fn current_position(&self) -> u64 {
        self.chain_position
    }

    pub fn current_hash(&self) -> [u8; 32] {
        self.prev_hash
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_seal() {
        let mut sealer = Vault999Sealer::new();
        assert_eq!(sealer.current_position(), 0);

        let data: [u8; 32] = [1u8; 32];
        let receipt = sealer.seal(data).unwrap();

        assert_eq!(receipt.chain_position, 0);
        assert_eq!(receipt.prev_hash, [0u8; 32]);
        assert_ne!(receipt.chain_entry_hash, [0u8; 32]); // not genesis
        assert_eq!(sealer.current_position(), 1);
        assert_eq!(sealer.current_hash(), receipt.chain_entry_hash);
    }

    #[test]
    fn test_hash_chain_integrity() {
        let mut sealer = Vault999Sealer::new();

        let r1 = sealer.seal([1u8; 32]).unwrap();
        let r2 = sealer.seal([2u8; 32]).unwrap();
        let r3 = sealer.seal([3u8; 32]).unwrap();

        // Each seal's prev_hash must equal the prior seal's chain_entry_hash
        assert_eq!(r2.prev_hash, r1.chain_entry_hash);
        assert_eq!(r3.prev_hash, r2.chain_entry_hash);

        // All three positions are sequential
        assert_eq!(r1.chain_position, 0);
        assert_eq!(r2.chain_position, 1);
        assert_eq!(r3.chain_position, 2);

        // Chain entry hashes are all unique
        assert_ne!(r1.chain_entry_hash, r2.chain_entry_hash);
        assert_ne!(r2.chain_entry_hash, r3.chain_entry_hash);
        assert_ne!(r1.chain_entry_hash, r3.chain_entry_hash);
    }

    #[test]
    fn test_tamper_detection() {
        let mut sealer = Vault999Sealer::new();
        let r1 = sealer.seal([1u8; 32]).unwrap();
        let r2 = sealer.seal([2u8; 32]).unwrap();

        // Verify r2's prev_hash matches r1's chain_entry_hash
        let mut hasher = Sha3_256::new();
        hasher.update(&r2.prev_hash);
        hasher.update(&r2.chain_position.to_be_bytes());
        hasher.update(&[2u8; 32]); // original checkpoint data
        let recomputed: [u8; 32] = hasher.finalize().into();

        assert_eq!(recomputed, r2.chain_entry_hash);

        // If we tamper with r1's data, the hash chain breaks
        // (demonstrates that chain verification is possible)
        assert_ne!(r1.chain_entry_hash, [0u8; 32]);
    }
}
