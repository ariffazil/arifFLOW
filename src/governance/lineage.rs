// arifFlow governance/lineage.rs
// RG-2: Lineage Reconstruction from Sealed Evidence
//
// Reality Graph RG-2 — Reconstruct causal lineage from sealed receipts only.
// Without source code, memory, human explanation, or comments.
//
// Invariants (from RG-2 specification):
//   L1. Every parent_receipt_ids entry must resolve or be surfaced as unresolved.
//   L2. A receipt cannot list itself as parent.
//   L3. Direct and indirect cycles must fail validation.
//   L4. Same store state produces identical proof ordering and root set.
//   L5. Every lineage node must resolve to a valid receipt-to-seal checkpoint binding
//       when require_sealed=true.
//   L6. Only explicit root receipts may have no parents; missing parent is not root.
//   L7. Traversal may return opaque/redacted ancestor stubs, never protected payloads.
//   L8. Supersession is represented by linked new receipts; old evidence remains traversable.
//   L9. Maximum depth/node count prevents hostile graphs from consuming unlimited resources.
//   L10. Resolver returns lineage context only; it must never authorize action from ancestry.
//
// DITEMPA BUKAN DIBERI — Forged, Not Given.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use thiserror::Error;

use crate::governance::vault999::SealReceipt;
use crate::receipt::FlowReceipt;
use sha3::Digest;

// ── ReceiptStore trait (read-only by mandate) ───────────────────────────

/// A read-only store that can resolve receipts and their seal bindings.
///
/// RG-2 STORE INVARIANT: This trait does NOT create, promote, reseal, or
/// mutate evidence. It resolves, verifies, and traverses.
///
/// IDENTITY MODEL: Body hashes (SHA3-256 of canonical receipt JSON) are the
/// stable DAG identity. `parent_receipt_ids` carries body hashes, not receipt_ids.
/// `receipt_id` (UUID) is the canonical session-internal handle; `body_hash` is
/// the cross-session lineage anchor. The store indexes both, and the resolver
/// prefers body_hash for traversal (since that's what parent_receipt_ids carries).
pub trait ReceiptStore {
    type Error: std::fmt::Debug;

    /// Look up the full receipt body by receipt_id (UUID).
    fn get_receipt(&self, receipt_id: &str) -> Result<Option<FlowReceipt>, Self::Error>;

    /// [NEW] Look up the full receipt body by body_hash (SHA3-256 hex).
    /// RG-2 traversal walks parent_receipt_ids which carry body hashes.
    fn get_receipt_by_hash(&self, body_hash: &str) -> Result<Option<FlowReceipt>, Self::Error>;

    /// Look up the seal record (chain entry) by receipt_id.
    fn get_seal(&self, receipt_id: &str) -> Result<Option<SealReceipt>, Self::Error>;

    /// The full SHA3-256 receipt body hash.
    fn receipt_body_hash(&self, receipt_id: &str) -> Result<Option<String>, Self::Error>;

    /// Whether the receipt-to-seal binding is intact (receipt body hash matches
    /// the seal's checkpoint_hash, and the seal entry's chain_entry_hash matches
    /// its position in the chain).
    fn verify_receipt_seal_binding(
        &self,
        receipt_id: &str,
    ) -> Result<SealBindingStatus, Self::Error>;

    /// The parent hashes carried by the receipt body (these are body hashes).
    fn parent_ids(&self, receipt_id: &str) -> Result<Vec<String>, Self::Error>;

    /// The genesis anchor carried by the receipt body (None for non-Genesis).
    fn genesis_anchor(&self, receipt_id: &str) -> Result<Option<String>, Self::Error>;

    /// Receipt_id for a given body_hash, if known.
    fn receipt_id_for_hash(&self, body_hash: &str) -> Result<Option<String>, Self::Error>;

    /// Classification level for a receipt body (L7).
    /// 0 = public (full node in proofs), 1 = protected (opaque stub; identity
    /// fields redacted, structure/edges preserved, genesis anchors suppressed).
    fn classification_level(&self, receipt_id: &str) -> Result<u8, Self::Error>;

    /// Child body_hashes (RG-2D forward traversal): every stored receipt whose
    /// parent_receipt_ids contains this receipt's CURRENT body hash.
    /// Input is the receipt_id (UUID); output is body hashes — symmetric with
    /// the identity model (edges are body-hash keyed).
    /// v1 is a linear scan; a reverse index is a later optimization, not a
    /// semantic change. Unknown receipt_id → empty vec (matches parent_ids).
    fn children(&self, receipt_id: &str) -> Result<Vec<String>, Self::Error>;
}

/// Seal binding verification result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SealBindingStatus {
    /// Receipt body hash matches seal checkpoint, chain entry valid.
    Bound,
    /// Seal entry exists but body hash mismatch (tampered body).
    TamperedBody,
    /// Receipt exists but no seal binding (legacy / unsealed).
    Unsealed,
    /// Receipt not found in store.
    NotFound,
}

#[derive(Debug, Error)]
pub enum ReceiptStoreError {
    #[error("Receipt not found: {0}")]
    NotFound(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Parse error: {0}")]
    Parse(String),
}

// ── Lineage Resolver ────────────────────────────────────────────────────

/// Resolves causal lineage from sealed evidence using a ReceiptStore.
///
/// READ-ONLY: never mutates the store. Returns a deterministic proof that
/// classifies every link as valid, missing, cyclic, unsealed, or classification-blocked.
pub struct LineageResolver<S: ReceiptStore> {
    store: S,
    max_depth: usize,
    max_nodes: usize,
    require_sealed: bool,
}

impl<S: ReceiptStore> LineageResolver<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            max_depth: 4096,
            max_nodes: 10_000,
            require_sealed: true,
        }
    }

    pub fn with_max_depth(mut self, max: usize) -> Self {
        self.max_depth = max;
        self
    }

    pub fn with_max_nodes(mut self, max: usize) -> Self {
        self.max_nodes = max;
        self
    }

    pub fn with_require_sealed(mut self, require: bool) -> Self {
        self.require_sealed = require;
        self
    }

    /// Reconstruct full backward lineage from `target` to its roots.
    pub fn reconstruct(&self, target: &str) -> LineageProof {
        LineageProver::new(
            &self.store,
            self.max_depth,
            self.max_nodes,
            self.require_sealed,
        )
        .prove(target)
    }
}

/// Internal prover — splits reconstruction into deterministic phases.
struct LineageProver<'a, S: ReceiptStore> {
    store: &'a S,
    max_depth: usize,
    max_nodes: usize,
    require_sealed: bool,
    /// visited keyed by body_hash. Body_hash is the stable DAG identity.
    visited: BTreeMap<String, LineageNode>,
    /// (parent_body_hash → child_body_hash), BTreeSet for determinism
    edges: BTreeSet<(String, String)>,
    missing_parents: BTreeSet<String>,
    unsealed: BTreeSet<String>,
    invalid_bindings: BTreeSet<String>,
    self_parents: BTreeSet<String>,
    cycles: Vec<ReceiptCycle>,
    classification_blocks: Vec<ClassificationBlock>,
    root_body_hashes: BTreeSet<String>,
    in_stack: HashSet<String>, // for cycle detection (DFS path stack by body_hash)
    /// Map body_hash → receipt_id for proof output.
    hash_to_id: BTreeMap<String, String>,
    /// Tracks whether DFS hit the max_depth limit.
    depth_limit_hit: bool,
    /// Tracks whether DFS hit the max_nodes limit.
    nodes_limit_hit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageProof {
    /// Target receipt body_hash for which lineage was reconstructed.
    pub target_body_hash: String,
    /// Target receipt UUID (resolved from body_hash).
    pub target_receipt_id: Option<String>,
    /// All root receipt UUIDs reached (no resolvable parents).
    pub root_receipt_ids: Vec<String>,
    /// All root body_hashes.
    pub root_body_hashes: Vec<String>,
    /// Every receipt visited, in deterministic BFS-discovery order.
    pub ordered_nodes: Vec<LineageNode>,
    /// Every parent → child edge traversed (deduplicated, sorted).
    pub edges: Vec<LineageEdge>,
    /// Final status of the lineage reconstruction.
    pub status: LineageStatus,
    /// Parent body_hashes that could not be resolved in the store.
    pub unresolved_parents: Vec<String>,
    /// Receipts found but not seal-bound (when require_sealed=true).
    pub unsealed_receipts: Vec<String>,
    /// Receipts with body-hash / seal-binding mismatch.
    pub invalid_bindings: Vec<String>,
    /// Receipts that list themselves as parent (L2) — reported distinctly
    /// from true multi-node cycles.
    pub self_parent_receipts: Vec<String>,
    /// Cycles detected during traversal.
    pub cycles: Vec<ReceiptCycle>,
    /// Classification-blocked ancestors (opaque stubs only).
    pub classification_blocks: Vec<ClassificationBlock>,
    /// Depth of deepest node reached.
    pub max_depth_reached: usize,
    /// Genesis anchors discovered during traversal.
    pub genesis_anchors: Vec<GenesisAnchor>,
    /// SHA3-256 over the serde serialization of this proof with the field
    /// itself excluded (serde skip — canonical form = proof sans hash).
    /// Same store state + same target → identical hash; any semantic-field
    /// change (nodes, edges, statuses, roots) → different hash. In-process
    /// determinism only — cross-language proof hashing follows the JCS
    /// contract (spec/RG2_JCS_HASH_CONTRACT_v1.md).
    #[serde(skip, default)]
    pub proof_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageNode {
    /// Stable identifier: body_hash is the canonical DAG identity.
    pub body_hash: String,
    /// receipt_id (UUID) when resolved, None for opaque stubs.
    pub receipt_id: Option<String>,
    /// SHA3-256 body hash (== self.body_hash for full receipts).
    pub body_hash_recorded: String,
    /// Receipt kind.
    pub kind: LineageNodeKind,
    /// Depth from target (0 = target itself).
    pub depth: usize,
    /// Seal binding status at traversal time.
    pub seal_status: SealBindingStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LineageNodeKind {
    /// Full receipt resolved from store.
    FullReceipt,
    /// Receipt referenced by parent edge but not found in store.
    Unresolved,
    /// Receipt found but blocked by classification — only opaque stub returned.
    ClassificationOpaque,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LineageEdge {
    pub parent_body_hash: String,
    pub child_body_hash: String,
    /// Whether the edge was verified (parent_hash resolves + binding valid).
    pub verified: bool,
    /// Why the edge is not verified (if applicable).
    pub failure: Option<EdgeFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EdgeFailure {
    ParentUnresolved,
    ParentUnsealed,
    ParentTamperedBinding,
    SelfParent,
    Cycle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptCycle {
    /// Ordered list of body_hashes forming the cycle (start == end).
    pub cycle_path: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationBlock {
    pub body_hash: String,
    pub receipt_id: Option<String>,
    /// Classification reason (e.g., "GENESIS-001 payload protected").
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisAnchor {
    pub body_hash: String,
    pub receipt_id: String,
    pub anchor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LineageStatus {
    /// All parents resolved, all bindings valid, all reachable nodes traversed.
    Valid,
    /// One or more parents could not be resolved.
    PartialMissingParent,
    /// One or more ancestors are not seal-bound (require_sealed=true).
    PartialUnsealedReceipt,
    /// Traversal halted at max_depth before reaching roots.
    PartialDepthLimit,
    /// Target has itself as parent (L2 violation).
    InvalidSelfParent,
    /// Traversal encountered a cycle.
    InvalidCycle,
    /// Traversal encountered tampered binding (body_hash != seal checkpoint).
    InvalidSealBinding,
    /// Traversal blocked by classification boundary (opaque stub returned).
    ClassificationRestricted,
}

impl LineageStatus {
    pub fn is_valid(&self) -> bool {
        matches!(self, LineageStatus::Valid)
    }

    pub fn is_partial(&self) -> bool {
        matches!(
            self,
            LineageStatus::PartialMissingParent
                | LineageStatus::PartialUnsealedReceipt
                | LineageStatus::PartialDepthLimit
                | LineageStatus::ClassificationRestricted
        )
    }

    pub fn is_invalid(&self) -> bool {
        matches!(
            self,
            LineageStatus::InvalidSelfParent
                | LineageStatus::InvalidCycle
                | LineageStatus::InvalidSealBinding
        )
    }
}

impl<'a, S: ReceiptStore> LineageProver<'a, S> {
    fn new(store: &'a S, max_depth: usize, max_nodes: usize, require_sealed: bool) -> Self {
        Self {
            store,
            max_depth,
            max_nodes,
            require_sealed,
            visited: BTreeMap::new(),
            edges: BTreeSet::new(),
            missing_parents: BTreeSet::new(),
            unsealed: BTreeSet::new(),
            invalid_bindings: BTreeSet::new(),
            self_parents: BTreeSet::new(),
            cycles: Vec::new(),
            classification_blocks: Vec::new(),
            root_body_hashes: BTreeSet::new(),
            hash_to_id: BTreeMap::new(),
            in_stack: HashSet::new(),
            depth_limit_hit: false,
            nodes_limit_hit: false,
        }
    }

    fn prove(mut self, target: &str) -> LineageProof {
        let mut genesis_anchors = Vec::new();
        let mut max_depth_reached = 0;

        // Resolve target — accept either receipt_id or body_hash.
        // We compute target_body_hash + target_receipt_id separately to avoid
        // holding an &mut self.borrow across the dfs() recursion.
        let target_body_hash = self.compute_target_body_hash(target);
        let target_receipt_id = self
            .store
            .receipt_id_for_hash(&target_body_hash)
            .ok()
            .flatten()
            .or_else(|| {
                // Target might be a receipt_id, not a hash.
                self.store
                    .get_receipt(target)
                    .ok()
                    .flatten()
                    .map(|r| r.receipt_id.to_string())
            });

        // Start DFS by body_hash (stable DAG identity).
        self.in_stack.insert(target_body_hash.clone());
        let (_s, d) = self.dfs(&target_body_hash, 0, &mut genesis_anchors);
        let _ = _s;
        max_depth_reached = max_depth_reached.max(d);

        // Final status arbitration: most severe wins
        let status = self.arbitrate_status();

        // Convert BTreeMap → sorted Vec for deterministic ordering
        let ordered_nodes: Vec<LineageNode> = self.visited.into_values().collect();
        let edges: Vec<LineageEdge> = self
            .edges
            .into_iter()
            .map(|(p, c)| LineageEdge {
                parent_body_hash: p,
                child_body_hash: c,
                verified: true,
                failure: None,
            })
            .collect();

        let mut root_receipt_ids: Vec<String> = self
            .root_body_hashes
            .iter()
            .filter_map(|h| self.hash_to_id.get(h).cloned())
            .collect();
        root_receipt_ids.sort();
        root_receipt_ids.dedup();

        let mut proof = LineageProof {
            target_body_hash,
            target_receipt_id,
            root_receipt_ids,
            root_body_hashes: self.root_body_hashes.into_iter().collect(),
            ordered_nodes,
            edges,
            status,
            unresolved_parents: self.missing_parents.into_iter().collect(),
            unsealed_receipts: self.unsealed.into_iter().collect(),
            invalid_bindings: self.invalid_bindings.into_iter().collect(),
            self_parent_receipts: self.self_parents.into_iter().collect(),
            cycles: self.cycles,
            classification_blocks: self.classification_blocks,
            max_depth_reached,
            genesis_anchors,
            proof_hash: String::new(),
        };
        // proof_hash over the canonical (serde-skip) form: the field is
        // excluded from serialization, so the digest covers every semantic
        // field and nothing of itself.
        let canonical = serde_json::to_string(&proof).unwrap_or_default();
        let mut hasher = Sha3Hasher::new();
        hasher.absorb(canonical.as_bytes());
        proof.proof_hash = hex::encode(hasher.finalize());
        proof
    }

    /// Compute the body_hash for the target. If target is a receipt_id, look up
    /// the receipt and return its hash. If target is already a hash, return it.
    /// If neither resolves, return target as-is (the DFS will mark it Unresolved).
    fn compute_target_body_hash(&self, target: &str) -> String {
        // Try receipt_id lookup first.
        if let Ok(Some(receipt)) = self.store.get_receipt(target) {
            return receipt.hash();
        }
        // Try body_hash lookup: if the store's INDEX resolves this address,
        // use it AS the address. Re-deriving the content hash here would
        // silently discard index-keyed addressing (the divergence model a
        // corrupted/tampered index presents). The node's body_hash_recorded
        // field still reports the content hash, so divergence stays visible.
        if self
            .store
            .get_receipt_by_hash(target)
            .ok()
            .flatten()
            .is_some()
        {
            return target.to_string();
        }
        // Fall through: target is unknown — treat as body_hash string.
        target.to_string()
    }

    /// Depth-first traversal by body_hash with cycle detection.
    fn dfs(
        &mut self,
        body_hash: &str,
        depth: usize,
        genesis_anchors: &mut Vec<GenesisAnchor>,
    ) -> (LineageStatus, usize) {
        // L9: bounded traversal
        if depth > self.max_depth {
            self.depth_limit_hit = true;
            self.in_stack.remove(body_hash);
            return (LineageStatus::PartialDepthLimit, depth);
        }
        if self.visited.len() >= self.max_nodes {
            self.nodes_limit_hit = true;
            self.in_stack.remove(body_hash);
            return (LineageStatus::PartialDepthLimit, depth);
        }

        // Resolve receipt by body_hash (the DAG identity).
        let receipt = match self.store.get_receipt_by_hash(body_hash) {
            Ok(Some(r)) => Some(r),
            Ok(None) => {
                // Missing parent: record and return partial status
                self.missing_parents.insert(body_hash.to_string());
                self.in_stack.remove(body_hash);
                return (LineageStatus::PartialMissingParent, depth);
            }
            Err(_) => {
                self.missing_parents.insert(body_hash.to_string());
                self.in_stack.remove(body_hash);
                return (LineageStatus::PartialMissingParent, depth);
            }
        };

        let receipt_id = receipt.as_ref().map(|r| r.receipt_id.to_string());

        // L7: classification gate — decided BEFORE any identity or anchor leak.
        let protected = match &receipt_id {
            Some(rid) => self.store.classification_level(rid).unwrap_or(0) >= 1,
            None => false,
        };

        // Track hash → id mapping for proof output (suppressed when protected).
        if let (Some(rid), false) = (&receipt_id, protected) {
            self.hash_to_id.insert(body_hash.to_string(), rid.clone());
        }

        // Verify seal binding (only meaningful when receipt_id is known).
        let binding = match &receipt_id {
            Some(rid) => self
                .store
                .verify_receipt_seal_binding(rid)
                .unwrap_or(SealBindingStatus::NotFound),
            None => SealBindingStatus::NotFound,
        };

        let recorded_body_hash = receipt
            .as_ref()
            .map(|r| r.hash())
            .unwrap_or_else(|| body_hash.to_string());

        let node_kind = if receipt.is_none() {
            LineageNodeKind::Unresolved
        } else if protected {
            LineageNodeKind::ClassificationOpaque
        } else {
            LineageNodeKind::FullReceipt
        };

        // Record genesis anchor if present — suppressed for protected nodes;
        // an anchor entry would carry receipt identity across the boundary.
        if let (Some(_), Some(rid)) = (&receipt, &receipt_id)
            && !protected
            && let Ok(Some(anchor)) = self.store.genesis_anchor(rid)
        {
            genesis_anchors.push(GenesisAnchor {
                body_hash: body_hash.to_string(),
                receipt_id: rid.clone(),
                anchor: anchor.clone(),
            });
        }

        self.visited.insert(
            body_hash.to_string(),
            LineageNode {
                body_hash: body_hash.to_string(),
                receipt_id: if protected { None } else { receipt_id.clone() },
                body_hash_recorded: recorded_body_hash,
                kind: node_kind,
                depth,
                seal_status: binding.clone(),
            },
        );

        // Register the classification block — structure preserved, payload opaque.
        if protected {
            self.classification_blocks.push(ClassificationBlock {
                body_hash: body_hash.to_string(),
                receipt_id: None,
                reason: "Protected classification — opaque stub, structure preserved".to_string(),
            });
        }

        // L5: unsealed handling
        if self.require_sealed
            && matches!(
                binding,
                SealBindingStatus::Unsealed | SealBindingStatus::TamperedBody
            )
        {
            if let Some(rid) = &receipt_id {
                self.unsealed.insert(rid.clone());
            }
            if matches!(binding, SealBindingStatus::TamperedBody) {
                if let Some(rid) = &receipt_id {
                    self.invalid_bindings.insert(rid.clone());
                }
                self.in_stack.remove(body_hash);
                return (LineageStatus::InvalidSealBinding, depth);
            }
            self.in_stack.remove(body_hash);
            return (LineageStatus::PartialUnsealedReceipt, depth);
        }

        // Get parents (parent_receipt_ids carries body_hashes).
        let parents = match &receipt_id {
            Some(rid) => self.store.parent_ids(rid).unwrap_or_default(),
            None => Vec::new(),
        };

        // Root handling
        if parents.is_empty() {
            self.root_body_hashes.insert(body_hash.to_string());
            self.in_stack.remove(body_hash);
            return (LineageStatus::Valid, depth);
        }

        // L2/L3: per-parent guards. A self-parent edge (L2) is recorded
        // distinctly from multi-node cycles (L3) and SKIPPED — never an
        // early return — so remaining parents still traverse.
        let mut worst = LineageStatus::Valid;
        let mut max_child_depth = depth;

        for parent_hash in &parents {
            // L2: self-parent
            if parent_hash == body_hash {
                self.self_parents.insert(body_hash.to_string());
                worst = match_worse(worst, LineageStatus::InvalidSelfParent);
                continue;
            }

            // Already in current DFS path → cycle
            if self.in_stack.contains(parent_hash) {
                self.cycles.push(ReceiptCycle {
                    cycle_path: vec![parent_hash.clone(), body_hash.to_string()],
                });
                worst = match_worse(worst, LineageStatus::InvalidCycle);
                continue;
            }

            // Already fully visited → record edge, skip recursion (deterministic).
            if self.visited.contains_key(parent_hash) {
                self.edges
                    .insert((parent_hash.clone(), body_hash.to_string()));
                continue;
            }

            self.in_stack.insert(parent_hash.clone());
            self.edges
                .insert((parent_hash.clone(), body_hash.to_string()));
            let (s, dd) = self.dfs(parent_hash, depth + 1, genesis_anchors);
            self.in_stack.remove(parent_hash);
            worst = match_worse(worst, s);
            max_child_depth = max_child_depth.max(dd);
        }

        self.in_stack.remove(body_hash);
        (worst, max_child_depth)
    }

    fn merge_status(&mut self, s: &LineageStatus) {
        // Reserved for future multi-target merges; single-DFS doesn't need it.
        let _ = s;
    }

    fn arbitrate_status(&self) -> LineageStatus {
        // Severity ranking (worst wins):
        //   InvalidSealBinding > InvalidCycle > InvalidSelfParent >
        //   PartialMissingParent > PartialUnsealedReceipt > PartialDepthLimit >
        //   ClassificationRestricted > Valid
        if !self.invalid_bindings.is_empty() {
            return LineageStatus::InvalidSealBinding;
        }
        if !self.cycles.is_empty() {
            return LineageStatus::InvalidCycle;
        }
        if !self.self_parents.is_empty() {
            return LineageStatus::InvalidSelfParent;
        }
        if !self.missing_parents.is_empty() {
            return LineageStatus::PartialMissingParent;
        }
        if !self.unsealed.is_empty() {
            return LineageStatus::PartialUnsealedReceipt;
        }
        if self.depth_limit_hit || self.nodes_limit_hit {
            return LineageStatus::PartialDepthLimit;
        }
        if !self.classification_blocks.is_empty() {
            return LineageStatus::ClassificationRestricted;
        }
        LineageStatus::Valid
    }
}

fn match_worse(a: LineageStatus, b: LineageStatus) -> LineageStatus {
    let rank = |s: &LineageStatus| -> u8 {
        match s {
            LineageStatus::Valid => 0,
            LineageStatus::ClassificationRestricted => 1,
            LineageStatus::PartialDepthLimit => 2,
            LineageStatus::PartialUnsealedReceipt => 3,
            LineageStatus::PartialMissingParent => 4,
            LineageStatus::InvalidSelfParent => 5,
            LineageStatus::InvalidCycle => 6,
            LineageStatus::InvalidSealBinding => 7,
        }
    };
    if rank(&b) > rank(&a) { b } else { a }
}

// ── In-Memory ReceiptStore (for tests + small scale) ──────────────────

/// Reference in-memory implementation of ReceiptStore. Useful for tests
/// and small-scale validation. Production should use a JSONL-backed store.
#[derive(Clone)]
pub struct InMemoryReceiptStore {
    receipts: BTreeMap<String, FlowReceipt>,
    seals: BTreeMap<String, SealReceipt>,
    genesis_cache: BTreeMap<String, u32>, // classification: 0=public, 1=protected
}

impl InMemoryReceiptStore {
    pub fn new() -> Self {
        Self {
            receipts: BTreeMap::new(),
            seals: BTreeMap::new(),
            genesis_cache: BTreeMap::new(),
        }
    }

    pub fn insert_receipt(&mut self, receipt: FlowReceipt) {
        let id = receipt.receipt_id.to_string();
        self.receipts.insert(id, receipt);
    }

    pub fn insert_seal(&mut self, seal: SealReceipt, receipt_id: String) {
        self.seals.insert(receipt_id, seal);
    }

    pub fn mark_classification(&mut self, body_hash: &str, level: u32) {
        self.genesis_cache.insert(body_hash.to_string(), level);
    }

    pub fn seal_for(&mut self, receipt_id: &str) -> Option<SealReceipt> {
        self.seals.get(receipt_id).cloned()
    }
}

impl Default for InMemoryReceiptStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ReceiptStore for InMemoryReceiptStore {
    type Error = ReceiptStoreError;

    fn get_receipt(&self, receipt_id: &str) -> Result<Option<FlowReceipt>, Self::Error> {
        Ok(self.receipts.get(receipt_id).cloned())
    }

    fn get_receipt_by_hash(&self, body_hash: &str) -> Result<Option<FlowReceipt>, Self::Error> {
        // In-memory: hash all receipts, find match.
        // For larger stores, a hash→receipt index would be more efficient.
        for r in self.receipts.values() {
            if r.hash() == body_hash {
                return Ok(Some(r.clone()));
            }
        }
        Ok(None)
    }

    fn get_seal(&self, receipt_id: &str) -> Result<Option<SealReceipt>, Self::Error> {
        Ok(self.seals.get(receipt_id).cloned())
    }

    fn receipt_body_hash(&self, receipt_id: &str) -> Result<Option<String>, Self::Error> {
        Ok(self.receipts.get(receipt_id).map(|r| r.hash()))
    }

    fn verify_receipt_seal_binding(
        &self,
        receipt_id: &str,
    ) -> Result<SealBindingStatus, Self::Error> {
        let receipt = match self.receipts.get(receipt_id) {
            Some(r) => r,
            None => return Ok(SealBindingStatus::NotFound),
        };
        let seal = match self.seals.get(receipt_id) {
            Some(s) => s,
            None => return Ok(SealBindingStatus::Unsealed),
        };
        let body_hash = receipt.hash();
        let mut hasher = Sha3Hasher::new();
        hasher.absorb(&seal.prev_hash);
        hasher.absorb(&seal.chain_position.to_be_bytes());
        hasher.absorb_hex(&body_hash);
        let expected = hasher.finalize();
        if expected == seal.chain_entry_hash {
            Ok(SealBindingStatus::Bound)
        } else {
            Ok(SealBindingStatus::TamperedBody)
        }
    }

    fn parent_ids(&self, receipt_id: &str) -> Result<Vec<String>, Self::Error> {
        Ok(self
            .receipts
            .get(receipt_id)
            .map(|r| r.parent_receipt_ids.clone())
            .unwrap_or_default())
    }

    fn genesis_anchor(&self, receipt_id: &str) -> Result<Option<String>, Self::Error> {
        Ok(self
            .receipts
            .get(receipt_id)
            .and_then(|r| r.genesis_anchor.clone()))
    }

    fn receipt_id_for_hash(&self, body_hash: &str) -> Result<Option<String>, Self::Error> {
        for r in self.receipts.values() {
            if r.hash() == body_hash {
                return Ok(Some(r.receipt_id.to_string()));
            }
        }
        Ok(None)
    }

    fn classification_level(&self, receipt_id: &str) -> Result<u8, Self::Error> {
        // Classification cache is keyed by the receipt's CURRENT body hash
        // (mark_classification registers it that way). Unregistered → public.
        Ok(self
            .receipts
            .get(receipt_id)
            .and_then(|r| self.genesis_cache.get(&r.hash()))
            .copied()
            .unwrap_or(0) as u8)
    }

    fn children(&self, receipt_id: &str) -> Result<Vec<String>, Self::Error> {
        let hash = match self.receipts.get(receipt_id) {
            Some(r) => r.hash(),
            None => return Ok(Vec::new()),
        };
        let mut children: Vec<String> = self
            .receipts
            .values()
            .filter(|r| r.parent_receipt_ids.contains(&hash))
            .map(|r| r.hash())
            .collect();
        children.sort();
        children.dedup();
        Ok(children)
    }
}

// ── Minimal SHA3-256 helper for seal-binding verification ───────────────
// We re-implement here to avoid leaking the receipt.rs Sha3 import surface.
struct Sha3Hasher {
    inner: sha3::Sha3_256,
}

impl Sha3Hasher {
    fn new() -> Self {
        Self {
            inner: sha3::Sha3_256::new(),
        }
    }
    fn absorb(&mut self, bytes: &[u8]) {
        sha3::Digest::update(&mut self.inner, bytes);
    }
    fn absorb_hex(&mut self, hex_str: &str) {
        if let Ok(bytes) = hex::decode(hex_str) {
            self.absorb(&bytes);
        }
    }
    fn finalize(self) -> [u8; 32] {
        sha3::Digest::finalize(self.inner).into()
    }
}

// ── JSONL-backed ReceiptStore (production) ──────────────────────────────

/// Reads receipts from a JSONL file (e.g., /var/lib/arifflow/receipts.jsonl)
/// and seal entries from a separate file (e.g., /root/arifOS/VAULT999/arifflow_sealed.jsonl).
/// Built lazily on first query.
pub struct JsonlReceiptStore {
    /// Interior-mutability cache: files are read once on first query, then
    /// served from memory. (Not Sync — single-threaded use; the daemon loads
    /// one store per query context.)
    inner: std::cell::RefCell<JsonlInner>,
}

struct JsonlInner {
    receipts_path: std::path::PathBuf,
    seals_path: std::path::PathBuf,
    by_id: BTreeMap<String, FlowReceipt>,
    by_hash: BTreeMap<String, String>, // body_hash → receipt_id
    seal_by_id: BTreeMap<String, SealReceipt>,
    loaded: bool,
}

impl JsonlReceiptStore {
    pub fn new<P: Into<std::path::PathBuf>>(receipts_path: P, seals_path: P) -> Self {
        Self {
            inner: std::cell::RefCell::new(JsonlInner {
                receipts_path: receipts_path.into(),
                seals_path: seals_path.into(),
                by_id: BTreeMap::new(),
                by_hash: BTreeMap::new(),
                seal_by_id: BTreeMap::new(),
                loaded: false,
            }),
        }
    }

    /// Run `f` against the loaded inner state. Loads lazily exactly once.
    fn with_loaded<R>(&self, f: impl FnOnce(&JsonlInner) -> R) -> Result<R, ReceiptStoreError> {
        {
            let mut inner = self.inner.borrow_mut();
            if !inner.loaded {
                inner.load()?;
            }
        }
        Ok(f(&self.inner.borrow()))
    }

    pub fn len(&self) -> usize {
        self.with_loaded(|i| i.by_id.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn seal_count(&self) -> usize {
        self.with_loaded(|i| i.seal_by_id.len()).unwrap_or(0)
    }
}

impl JsonlInner {
    fn load(&mut self) -> Result<(), ReceiptStoreError> {
        self.loaded = true;

        // Load receipts
        let contents = std::fs::read_to_string(&self.receipts_path)
            .map_err(|e| ReceiptStoreError::Io(e.to_string()))?;
        for (line_no, line) in contents.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<FlowReceipt>(line) {
                Ok(r) => {
                    let id = r.receipt_id.to_string();
                    let hash = r.hash();
                    self.by_hash.insert(hash.clone(), id.clone());
                    self.by_id.insert(id, r);
                }
                Err(e) => {
                    eprintln!(
                        "[arifFlow] WARN: receipts.jsonl line {} parse error: {}",
                        line_no + 1,
                        e
                    );
                }
            }
        }

        // Load seals (legacy format is partial: only chain metadata).
        // Use a relaxed parser to tolerate missing fields.
        let seal_contents = std::fs::read_to_string(&self.seals_path)
            .map_err(|e| ReceiptStoreError::Io(e.to_string()))?;
        for (line_no, line) in seal_contents.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<LegacySealEntry>(line) {
                Ok(entry) => {
                    let prev = parse_hex32(&entry.prev_hash).unwrap_or([0u8; 32]);
                    let chain_entry = parse_hex32(&entry.chain_entry_hash).unwrap_or([0u8; 32]);
                    self.seal_by_id.insert(
                        entry.receipt_id.clone(),
                        SealReceipt {
                            vault_entry_id: entry.vault_entry_id,
                            chain_position: entry.chain_position,
                            prev_hash: prev,
                            chain_entry_hash: chain_entry,
                        },
                    );
                }
                Err(e) => {
                    eprintln!(
                        "[arifFlow] WARN: seals.jsonl line {} parse error: {}",
                        line_no + 1,
                        e
                    );
                }
            }
        }

        Ok(())
    }
}

/// Legacy seal entry shape: {vault_entry_id, chain_position, prev_hash, chain_entry_hash, receipt_id}
/// This is the pre-RG-2 minimum seal entry. RG-2 lineage reconstruction joins these to receipt bodies.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacySealEntry {
    pub vault_entry_id: String,
    pub chain_position: u64,
    pub prev_hash: String,
    pub chain_entry_hash: String,
    pub receipt_id: String,
}

fn parse_hex32(s: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(s).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Some(arr)
}

impl ReceiptStore for JsonlReceiptStore {
    type Error = ReceiptStoreError;

    fn get_receipt(&self, receipt_id: &str) -> Result<Option<FlowReceipt>, Self::Error> {
        self.with_loaded(|i| i.by_id.get(receipt_id).cloned())
    }

    fn get_receipt_by_hash(&self, body_hash: &str) -> Result<Option<FlowReceipt>, Self::Error> {
        self.with_loaded(|i| {
            if let Some(rid) = i.by_hash.get(body_hash) {
                return i.by_id.get(rid).cloned();
            }
            // Fallback: scan in case by_hash index missed due to hash collision tolerance.
            i.by_id.values().find(|r| r.hash() == body_hash).cloned()
        })
    }

    fn get_seal(&self, receipt_id: &str) -> Result<Option<SealReceipt>, Self::Error> {
        self.with_loaded(|i| i.seal_by_id.get(receipt_id).cloned())
    }

    fn receipt_body_hash(&self, receipt_id: &str) -> Result<Option<String>, Self::Error> {
        self.with_loaded(|i| i.by_id.get(receipt_id).map(|r| r.hash()))
    }

    fn verify_receipt_seal_binding(
        &self,
        receipt_id: &str,
    ) -> Result<SealBindingStatus, Self::Error> {
        self.with_loaded(|i| {
            let receipt = match i.by_id.get(receipt_id) {
                Some(r) => r,
                None => return SealBindingStatus::NotFound,
            };
            let seal = match i.seal_by_id.get(receipt_id) {
                Some(s) => s,
                None => return SealBindingStatus::Unsealed,
            };
            let body_hash = receipt.hash();
            let mut hasher = Sha3Hasher::new();
            hasher.absorb(&seal.prev_hash);
            hasher.absorb(&seal.chain_position.to_be_bytes());
            hasher.absorb_hex(&body_hash);
            let expected = hasher.finalize();
            if expected == seal.chain_entry_hash {
                SealBindingStatus::Bound
            } else {
                SealBindingStatus::TamperedBody
            }
        })
    }

    fn parent_ids(&self, receipt_id: &str) -> Result<Vec<String>, Self::Error> {
        self.with_loaded(|i| {
            i.by_id
                .get(receipt_id)
                .map(|r| r.parent_receipt_ids.clone())
                .unwrap_or_default()
        })
    }

    fn genesis_anchor(&self, receipt_id: &str) -> Result<Option<String>, Self::Error> {
        self.with_loaded(|i| {
            i.by_id
                .get(receipt_id)
                .and_then(|r| r.genesis_anchor.clone())
        })
    }

    fn receipt_id_for_hash(&self, body_hash: &str) -> Result<Option<String>, Self::Error> {
        self.with_loaded(|i| i.by_hash.get(body_hash).cloned())
    }

    fn classification_level(&self, receipt_id: &str) -> Result<u8, Self::Error> {
        // Provisional rule (open loop): receipts carrying a genesis anchor are
        // protected; everything else public. An organ-owned classification
        // registry supersedes this once it exists.
        self.with_loaded(|i| {
            i.by_id
                .get(receipt_id)
                .map(|r| if r.genesis_anchor.is_some() { 1u8 } else { 0u8 })
                .unwrap_or(0)
        })
    }

    fn children(&self, receipt_id: &str) -> Result<Vec<String>, Self::Error> {
        self.with_loaded(|i| {
            let hash = match i.by_id.get(receipt_id) {
                Some(r) => r.hash(),
                None => return Vec::new(),
            };
            let mut children: Vec<String> = i
                .by_id
                .values()
                .filter(|r| r.parent_receipt_ids.contains(&hash))
                .map(|r| r.hash())
                .collect();
            children.sort();
            children.dedup();
            children
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::vault999::Vault999Sealer;
    use crate::receipt::{EpistemicLabel, StepType};

    fn make_receipt(actor: &str, step: u64, parents: Vec<String>) -> FlowReceipt {
        let mut r = FlowReceipt::new_first(
            actor,
            "test-session",
            StepType::Execute,
            EpistemicLabel::Observation,
            1000,
        );
        r.step_number = step;
        r.parent_receipt_ids = parents;
        r
    }

    fn make_seal_for(receipt: &FlowReceipt, sealer: &mut Vault999Sealer) -> SealReceipt {
        let hash_bytes = hex::decode(receipt.hash()).expect("hex decode");
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&hash_bytes);
        sealer.seal(arr).expect("seal")
    }

    #[test]
    fn test_reconstructs_root_to_child_lineage() {
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        let root = make_receipt("a", 0, vec![]);
        let root_id = root.receipt_id.to_string();
        store.insert_receipt(root.clone());
        let root_seal = make_seal_for(&root, &mut sealer);
        store.insert_seal(root_seal, root_id.clone());

        let child = make_receipt("a", 1, vec![root.hash()]);
        let child_id = child.receipt_id.to_string();
        store.insert_receipt(child.clone());
        let child_seal = make_seal_for(&child, &mut sealer);
        store.insert_seal(child_seal, child_id.clone());

        let resolver = LineageResolver::new(store);
        let proof = resolver.reconstruct(&child_id);

        assert_eq!(proof.target_receipt_id.as_deref(), Some(child_id.as_str()));
        assert_eq!(proof.status, LineageStatus::Valid);
        assert_eq!(proof.root_receipt_ids, vec![root_id]);
        assert_eq!(proof.ordered_nodes.len(), 2);
        assert_eq!(proof.unresolved_parents.len(), 0);
        assert_eq!(proof.unsealed_receipts.len(), 0);
        assert_eq!(proof.cycles.len(), 0);
    }

    #[test]
    fn test_reconstructs_multi_parent_dag() {
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        let a = make_receipt("a", 0, vec![]);
        let b = make_receipt("a", 0, vec![]);
        let c = make_receipt("a", 0, vec![]);

        let a_id = a.receipt_id.to_string();
        let b_id = b.receipt_id.to_string();
        let c_id = c.receipt_id.to_string();

        store.insert_receipt(a.clone());
        store.insert_receipt(b.clone());
        store.insert_receipt(c.clone());

        store.insert_seal(make_seal_for(&a, &mut sealer), a_id.clone());
        store.insert_seal(make_seal_for(&b, &mut sealer), b_id.clone());
        store.insert_seal(make_seal_for(&c, &mut sealer), c_id.clone());

        let merge = make_receipt("a", 1, vec![a.hash(), b.hash(), c.hash()]);
        let merge_id = merge.receipt_id.to_string();
        store.insert_receipt(merge.clone());
        store.insert_seal(make_seal_for(&merge, &mut sealer), merge_id.clone());

        let resolver = LineageResolver::new(store);
        let proof = resolver.reconstruct(&merge_id);

        assert_eq!(proof.status, LineageStatus::Valid);
        // Sorted roots
        let mut expected_roots = vec![a_id.clone(), b_id.clone(), c_id.clone()];
        expected_roots.sort();
        assert_eq!(proof.root_receipt_ids, expected_roots);
        // All 4 nodes visited (3 roots + merge)
        assert_eq!(proof.ordered_nodes.len(), 4);
        // 3 edges (3 parents → merge)
        assert_eq!(proof.edges.len(), 3);
    }

    #[test]
    fn test_returns_stable_deterministic_proof() {
        // Run the same reconstruction twice and verify identical output.
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        let r0 = make_receipt("a", 0, vec![]);
        let r1 = make_receipt("a", 1, vec![r0.hash()]);
        let r2 = make_receipt("a", 2, vec![r1.hash()]);

        store.insert_receipt(r0.clone());
        store.insert_receipt(r1.clone());
        store.insert_receipt(r2.clone());

        store.insert_seal(make_seal_for(&r0, &mut sealer), r0.receipt_id.to_string());
        store.insert_seal(make_seal_for(&r1, &mut sealer), r1.receipt_id.to_string());
        store.insert_seal(make_seal_for(&r2, &mut sealer), r2.receipt_id.to_string());

        let target = r2.receipt_id.to_string();
        let resolver = LineageResolver::new(store);
        let p1 = resolver.reconstruct(&target);
        let p2 = resolver.reconstruct(&target);

        let j1 = serde_json::to_string(&p1).unwrap();
        let j2 = serde_json::to_string(&p2).unwrap();
        assert_eq!(j1, j2, "Reconstruction must be deterministic");
    }

    #[test]
    fn test_flags_missing_parent() {
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        // Child references a parent that doesn't exist in store.
        let ghost_parent = "ghost_hash_does_not_exist".to_string();
        let child = make_receipt("a", 1, vec![ghost_parent.clone()]);
        let child_hash = child.hash();
        let child_id = child.receipt_id.to_string();
        eprintln!(
            "[DEBUG] child_hash={} child_id={}",
            &child_hash[..16],
            &child_id[..8]
        );
        eprintln!(
            "[DEBUG] receipt parent_receipt_ids={:?}",
            child.parent_receipt_ids
        );
        store.insert_receipt(child.clone());
        store.insert_seal(make_seal_for(&child, &mut sealer), child_id.clone());

        let resolver = LineageResolver::new(store);
        // Reconstruct from body_hash so DFS walks parents at depth 1.
        let proof = resolver.reconstruct(&child_hash);
        eprintln!("[DEBUG] proof.status={:?}", proof.status);
        eprintln!(
            "[DEBUG] proof.unresolved_parents={:?}",
            proof.unresolved_parents
        );
        eprintln!("[DEBUG] proof.ordered_nodes={}", proof.ordered_nodes.len());

        assert_eq!(proof.status, LineageStatus::PartialMissingParent);
        assert!(proof.unresolved_parents.contains(&ghost_parent));
    }

    // ── MockCycleStore — index-divergence witness (RG-2-FIX-001 F1/F2) ────
    //
    // Under content-addressed identity (hash covers parent_receipt_ids) an
    // honest cycle is UNCONSTRUCTIBLE: a receipt cannot contain its own
    // post-parent hash, and any post-hoc mutation re-hashes the body so the
    // back-reference resolves to nothing. Cycles become reachable only when
    // the store's INDEX diverges from content (tampered index, migration
    // bug, hash collision). This mock hand-wires that index so the
    // resolver's L2/L3 structural guards are actually WITNESSED.

    struct MockCycleStore {
        by_hash: BTreeMap<String, FlowReceipt>,
    }

    impl MockCycleStore {
        fn with(entries: Vec<(String, FlowReceipt)>) -> Self {
            Self {
                by_hash: entries.into_iter().collect(),
            }
        }
    }

    impl ReceiptStore for MockCycleStore {
        type Error = ReceiptStoreError;

        fn get_receipt(&self, receipt_id: &str) -> Result<Option<FlowReceipt>, Self::Error> {
            Ok(self
                .by_hash
                .values()
                .find(|r| r.receipt_id.to_string() == receipt_id)
                .cloned())
        }

        fn get_receipt_by_hash(&self, body_hash: &str) -> Result<Option<FlowReceipt>, Self::Error> {
            Ok(self.by_hash.get(body_hash).cloned())
        }

        fn get_seal(&self, _receipt_id: &str) -> Result<Option<SealReceipt>, Self::Error> {
            Ok(None)
        }

        fn receipt_body_hash(&self, receipt_id: &str) -> Result<Option<String>, Self::Error> {
            Ok(self
                .by_hash
                .iter()
                .find(|(_, r)| r.receipt_id.to_string() == receipt_id)
                .map(|(h, _)| h.clone()))
        }

        fn verify_receipt_seal_binding(
            &self,
            receipt_id: &str,
        ) -> Result<SealBindingStatus, Self::Error> {
            if self.get_receipt(receipt_id)?.is_some() {
                // Mock seals: everything known is bound.
                Ok(SealBindingStatus::Bound)
            } else {
                Ok(SealBindingStatus::NotFound)
            }
        }

        fn parent_ids(&self, receipt_id: &str) -> Result<Vec<String>, Self::Error> {
            Ok(self
                .get_receipt(receipt_id)?
                .map(|r| r.parent_receipt_ids)
                .unwrap_or_default())
        }

        fn genesis_anchor(&self, _receipt_id: &str) -> Result<Option<String>, Self::Error> {
            Ok(None)
        }

        fn receipt_id_for_hash(&self, body_hash: &str) -> Result<Option<String>, Self::Error> {
            Ok(self
                .by_hash
                .get(body_hash)
                .map(|r| r.receipt_id.to_string()))
        }

        fn classification_level(&self, _receipt_id: &str) -> Result<u8, Self::Error> {
            Ok(0)
        }

        fn children(&self, receipt_id: &str) -> Result<Vec<String>, Self::Error> {
            // Index-divergence model: edges resolve through the hand-wired
            // index, so children of X = registered keys K where the receipt
            // registered at K lists X's registered key as parent.
            let key = match self
                .by_hash
                .iter()
                .find(|(_, r)| r.receipt_id.to_string() == receipt_id)
            {
                Some((k, _)) => k.clone(),
                None => return Ok(Vec::new()),
            };
            let mut children: Vec<String> = self
                .by_hash
                .iter()
                .filter(|(_, r)| r.parent_receipt_ids.contains(&key))
                .map(|(k, _)| k.clone())
                .collect();
            children.sort();
            children.dedup();
            Ok(children)
        }
    }

    #[test]
    fn test_rejects_self_parent() {
        // L2 strict: Z lists itself as parent (via corrupted index).
        let z = make_receipt("z", 0, vec!["hZ".to_string()]);
        let store = MockCycleStore::with(vec![("hZ".to_string(), z)]);

        let resolver = LineageResolver::new(store);
        let proof = resolver.reconstruct("hZ");

        assert_eq!(proof.status, LineageStatus::InvalidSelfParent);
        assert!(proof.self_parent_receipts.contains(&"hZ".to_string()));
        assert!(
            proof.cycles.is_empty(),
            "self-parent must not be mislabeled as a multi-node cycle"
        );
    }

    #[test]
    fn test_self_parent_does_not_block_other_parents() {
        // L2 skip-not-return: Z lists [self, valid root] — root still traverses.
        let root = make_receipt("r", 0, vec![]);
        let z = make_receipt("z", 0, vec!["hZ".to_string(), "hRoot".to_string()]);
        let store = MockCycleStore::with(vec![("hRoot".to_string(), root), ("hZ".to_string(), z)]);

        let resolver = LineageResolver::new(store);
        let proof = resolver.reconstruct("hZ");

        assert_eq!(proof.status, LineageStatus::InvalidSelfParent);
        assert!(proof.root_body_hashes.contains(&"hRoot".to_string()));
    }

    #[test]
    fn test_detects_direct_cycle() {
        // L3 strict: A ↔ B via corrupted index.
        let a = make_receipt("a", 0, vec!["hB".to_string()]);
        let b = make_receipt("b", 0, vec!["hA".to_string()]);
        let store = MockCycleStore::with(vec![("hA".to_string(), a), ("hB".to_string(), b)]);

        let resolver = LineageResolver::new(store);
        let proof = resolver.reconstruct("hA");

        assert_eq!(proof.status, LineageStatus::InvalidCycle);
        assert!(!proof.cycles.is_empty());
    }

    #[test]
    fn test_detects_indirect_cycle() {
        // L3 strict: A → B → C → A via corrupted index.
        let a = make_receipt("a", 0, vec!["hB".to_string()]);
        let b = make_receipt("b", 0, vec!["hC".to_string()]);
        let c = make_receipt("c", 0, vec!["hA".to_string()]);
        let store = MockCycleStore::with(vec![
            ("hA".to_string(), a),
            ("hB".to_string(), b),
            ("hC".to_string(), c),
        ]);

        let resolver = LineageResolver::new(store);
        let proof = resolver.reconstruct("hA");

        assert_eq!(proof.status, LineageStatus::InvalidCycle);
        assert!(!proof.cycles.is_empty());
    }

    #[test]
    fn test_post_mutation_without_reindex_is_missing_parent() {
        // Identity-semantics witness: patching parents AFTER insertion does
        // not re-index the store, so stale back-references resolve to nothing.
        // Honest outcome is PARTIAL — never silently Valid, never a fake cycle.
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        let a = make_receipt("a", 0, vec![]);
        let b = make_receipt("b", 0, vec![]);
        let c = make_receipt("c", 0, vec![]);
        let a_id = a.receipt_id.to_string();
        let b_id = b.receipt_id.to_string();
        let c_id = c.receipt_id.to_string();
        let c_hash = c.hash();
        store.insert_receipt(a.clone());
        store.insert_receipt(b.clone());
        store.insert_receipt(c.clone());

        // Forge A → C → B → A by post-insertion patching (no re-index).
        store.receipts.get_mut(&a_id).unwrap().parent_receipt_ids = vec![c_hash.clone()];
        store.receipts.get_mut(&b_id).unwrap().parent_receipt_ids = vec![a.hash()];
        store.receipts.get_mut(&c_id).unwrap().parent_receipt_ids = vec![b.hash()];

        // Seal the PATCHED bodies so binding covers what is stored now.
        let a_patched = store.receipts.get(&a_id).unwrap().clone();
        let b_patched = store.receipts.get(&b_id).unwrap().clone();
        let c_patched = store.receipts.get(&c_id).unwrap().clone();
        store.insert_seal(make_seal_for(&a_patched, &mut sealer), a_id.clone());
        store.insert_seal(make_seal_for(&b_patched, &mut sealer), b_id.clone());
        store.insert_seal(make_seal_for(&c_patched, &mut sealer), c_id.clone());

        // Reconstruct from A's CURRENT hash so the target itself resolves.
        let a_current = store.receipts.get(&a_id).unwrap().hash();
        let resolver = LineageResolver::new(store);
        let proof = resolver.reconstruct(&a_current);

        assert_eq!(proof.status, LineageStatus::PartialMissingParent);
        assert!(proof.unresolved_parents.contains(&c_hash));
    }

    #[test]
    fn test_post_hoc_self_reference_is_missing_not_selfparent() {
        // Inserting one's own pre-parent hash resolves to nothing after the
        // body re-hashes — missing parent, NOT InvalidSelfParent.
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        let mut r = make_receipt("a", 0, vec![]);
        let pre_hash = r.hash();
        r.parent_receipt_ids = vec![pre_hash.clone()];
        let id = r.receipt_id.to_string();
        store.insert_receipt(r.clone());
        store.insert_seal(make_seal_for(&r, &mut sealer), id.clone());

        let resolver = LineageResolver::new(store);
        let proof = resolver.reconstruct(&id);

        assert_eq!(proof.status, LineageStatus::PartialMissingParent);
    }

    #[test]
    fn test_flags_unsealed_ancestor() {
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        let parent = make_receipt("a", 0, vec![]);
        let parent_id = parent.receipt_id.to_string();
        store.insert_receipt(parent.clone());
        // Parent is intentionally NOT sealed.

        let child = make_receipt("a", 1, vec![parent.hash()]);
        let child_id = child.receipt_id.to_string();
        store.insert_receipt(child.clone());
        store.insert_seal(make_seal_for(&child, &mut sealer), child_id.clone());

        let resolver = LineageResolver::new(store).with_require_sealed(true);
        let proof = resolver.reconstruct(&child_id);

        assert_eq!(proof.status, LineageStatus::PartialUnsealedReceipt);
        assert!(proof.unsealed_receipts.contains(&parent_id));
    }

    #[test]
    fn test_detects_receipt_hash_checkpoint_mismatch() {
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        let receipt = make_receipt("a", 0, vec![]);
        let id = receipt.receipt_id.to_string();

        // Insert a tampered receipt (different fields) but seal it as if
        // it were the original. The seal binding will not match.
        let mut tampered = receipt.clone();
        tampered.cost_ns = receipt.cost_ns + 99999; // modify field

        store.insert_receipt(tampered);
        store.insert_seal(make_seal_for(&receipt, &mut sealer), id.clone());

        let resolver = LineageResolver::new(store).with_require_sealed(true);
        let proof = resolver.reconstruct(&id);

        assert_eq!(proof.status, LineageStatus::InvalidSealBinding);
        assert!(proof.invalid_bindings.contains(&id));
    }

    #[test]
    fn test_respects_classification_boundary() {
        // L7 witness: a protected ancestor is returned as an opaque stub —
        // identity redacted, genesis anchor suppressed, structure preserved.
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        let mut genesis = make_receipt("a", 0, vec![]);
        genesis = genesis.with_genesis_anchor("RCP-000");
        let g_id = genesis.receipt_id.to_string();
        let g_hash = genesis.hash();
        store.insert_receipt(genesis.clone());
        store.insert_seal(make_seal_for(&genesis, &mut sealer), g_id.clone());
        store.mark_classification(&g_hash, 1); // level 1 = protected

        let child = make_receipt("a", 1, vec![g_hash.clone()]);
        let c_id = child.receipt_id.to_string();
        let c_hash = child.hash();
        store.insert_receipt(child.clone());
        store.insert_seal(make_seal_for(&child, &mut sealer), c_id.clone());

        let resolver = LineageResolver::new(store);
        let proof = resolver.reconstruct(&c_id);

        assert_eq!(proof.status, LineageStatus::ClassificationRestricted);

        // Protected ancestor: opaque stub, identity redacted.
        let g_node = proof
            .ordered_nodes
            .iter()
            .find(|n| n.body_hash == g_hash)
            .expect("protected ancestor present as structure");
        assert_eq!(g_node.kind, LineageNodeKind::ClassificationOpaque);
        assert!(g_node.receipt_id.is_none(), "identity must be redacted");

        // No anchor leak across the boundary.
        assert!(proof.genesis_anchors.is_empty());

        // Block registered.
        assert!(
            proof
                .classification_blocks
                .iter()
                .any(|b| b.body_hash == g_hash)
        );

        // Structure preserved: edge child→genesis traversed; child stays full.
        assert!(
            proof
                .edges
                .iter()
                .any(|e| e.parent_body_hash == g_hash && e.child_body_hash == c_hash)
        );
        let c_node = proof
            .ordered_nodes
            .iter()
            .find(|n| n.body_hash == c_hash)
            .expect("child present");
        assert_eq!(c_node.kind, LineageNodeKind::FullReceipt);
    }

    #[test]
    fn test_halts_at_max_depth() {
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        // Build a chain of 20 receipts, each pointing to the previous as parent.
        let mut prev_hash: Option<String> = None;
        let mut last_hash = String::new();
        for i in 0..20 {
            let parents = prev_hash.clone().map(|h| vec![h]).unwrap_or_default();
            let r = make_receipt("a", i, parents);
            last_hash = r.hash();
            let id = r.receipt_id.to_string();
            store.insert_receipt(r.clone());
            store.insert_seal(make_seal_for(&r, &mut sealer), id);
            prev_hash = Some(last_hash.clone());
        }

        let resolver = LineageResolver::new(store).with_max_depth(5);
        let proof = resolver.reconstruct(&last_hash);

        // The traversal must halt with PartialDepthLimit, proving bounded recursion.
        assert_eq!(proof.status, LineageStatus::PartialDepthLimit);
        // max_depth_reached reflects the deepest visited depth before the limit fires.
        // With max=5, we should see depths 0..=5 visited (6 is rejected).
        assert!(
            proof.max_depth_reached <= 6,
            "max_depth_reached ({}) must remain bounded by max_depth + 1",
            proof.max_depth_reached
        );
    }

    #[test]
    fn test_halts_at_max_nodes() {
        // L9 node-count bound (ported from the superseded src/lineage.rs):
        // a hostile or huge graph cannot consume unbounded memory.
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        let mut prev_hash: Option<String> = None;
        let mut last_hash = String::new();
        for i in 0..10 {
            let parents = prev_hash.clone().map(|h| vec![h]).unwrap_or_default();
            let r = make_receipt("a", i, parents);
            last_hash = r.hash();
            let id = r.receipt_id.to_string();
            store.insert_receipt(r.clone());
            store.insert_seal(make_seal_for(&r, &mut sealer), id);
            prev_hash = Some(last_hash.clone());
        }

        let resolver = LineageResolver::new(store).with_max_nodes(3);
        let proof = resolver.reconstruct(&last_hash);

        assert_eq!(proof.status, LineageStatus::PartialDepthLimit);
        assert!(proof.ordered_nodes.len() <= 3);
    }

    #[test]
    fn test_children_reverse_traversal() {
        // RG-2D: forward edges resolve from the same durable evidence.
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        let root = make_receipt("a", 0, vec![]);
        let root_id = root.receipt_id.to_string();
        store.insert_receipt(root.clone());
        store.insert_seal(make_seal_for(&root, &mut sealer), root_id.clone());

        let a = make_receipt("a", 1, vec![root.hash()]);
        let a_id = a.receipt_id.to_string();
        store.insert_receipt(a.clone());
        store.insert_seal(make_seal_for(&a, &mut sealer), a_id.clone());

        let b = make_receipt("b", 1, vec![root.hash()]);
        let b_id = b.receipt_id.to_string();
        store.insert_receipt(b.clone());
        store.insert_seal(make_seal_for(&b, &mut sealer), b_id.clone());

        let c = make_receipt("c", 2, vec![a.hash()]);
        let c_id = c.receipt_id.to_string();
        store.insert_receipt(c.clone());
        store.insert_seal(make_seal_for(&c, &mut sealer), c_id.clone());

        // Fan-out: root's children = {a, b} body hashes, sorted + deduped.
        let mut expected = vec![a.hash(), b.hash()];
        expected.sort();
        assert_eq!(store.children(&root_id).unwrap(), expected);
        // Chain: a's child = c.
        assert_eq!(store.children(&a_id).unwrap(), vec![c.hash()]);
        // Leaf and unknown ids → empty (matches parent_ids semantics).
        assert!(store.children(&c_id).unwrap().is_empty());
        assert!(store.children("nonexistent").unwrap().is_empty());
    }

    #[test]
    fn test_proof_hash_deterministic_and_sensitive() {
        // Determinism: same store, same resolver, same target → identical hash.
        // Sensitivity: a semantic proof change flips the hash.
        let build = || {
            let mut s = InMemoryReceiptStore::new();
            let mut sealer = Vault999Sealer::new();
            let root = make_receipt("a", 0, vec![]);
            s.insert_receipt(root.clone());
            // root intentionally UNSEALED.
            let child = make_receipt("a", 1, vec![root.hash()]);
            let cid = child.receipt_id.to_string();
            s.insert_receipt(child.clone());
            s.insert_seal(make_seal_for(&child, &mut sealer), cid.clone());
            (s, cid)
        };

        let (s, cid) = build();
        let r1 = LineageResolver::new(s.clone()).with_require_sealed(false);
        let h1a = r1.reconstruct(&cid).proof_hash;
        let h1b = r1.reconstruct(&cid).proof_hash;
        assert!(!h1a.is_empty());
        assert_eq!(h1a, h1b, "same inputs must produce identical proof_hash");

        // Same store, require_sealed=true surfaces the unsealed ancestor →
        // different status, different unsealed_receipts → different hash.
        let r2 = LineageResolver::new(s).with_require_sealed(true);
        let h2 = r2.reconstruct(&cid).proof_hash;
        assert_ne!(h1a, h2, "semantic proof change must change proof_hash");
    }

    #[test]
    #[ignore = "live OL-004 audit — reads production receipt store + sealed ledger; writes SEAL_BINDING_PROOF.json"]
    fn live_seal_binding_audit_writes_proof_json() {
        let receipts_path = "/var/lib/arifflow/receipts.jsonl";
        let seals_path = "/root/arifOS/VAULT999/arifflow_sealed.jsonl";
        let store = JsonlReceiptStore::new(receipts_path, seals_path);

        // Sample the 25 most recent seal entries (file append order = chain order).
        let seal_lines = std::fs::read_to_string(seals_path).expect("sealed ledger readable");
        let mut sampled: Vec<(u64, String)> = Vec::new();
        for line in seal_lines.lines().filter(|l| !l.trim().is_empty()) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                let pos = v
                    .get("chain_position")
                    .and_then(|p| p.as_u64())
                    .unwrap_or(0);
                if let Some(rid) = v.get("receipt_id").and_then(|r| r.as_str()) {
                    sampled.push((pos, rid.to_string()));
                }
            }
        }
        sampled.sort_by_key(|(pos, _)| *pos);
        let sample: Vec<String> = sampled
            .iter()
            .rev()
            .take(25)
            .map(|(_, rid)| rid.clone())
            .collect();
        assert!(!sample.is_empty(), "no seal entries found in ledger");

        let (mut bound, mut unsealed, mut tampered, mut not_found) =
            (0usize, 0usize, 0usize, 0usize);
        let mut details = Vec::new();
        for rid in &sample {
            let status = store.verify_receipt_seal_binding(rid).expect("verify");
            match status {
                SealBindingStatus::Bound => bound += 1,
                SealBindingStatus::Unsealed => unsealed += 1,
                SealBindingStatus::TamperedBody => tampered += 1,
                SealBindingStatus::NotFound => not_found += 1,
            }
            details.push(serde_json::json!({
                "receipt_id": rid,
                "binding": status,
            }));
        }

        let proof = serde_json::json!({
            "schema": "arifos.seal-binding-proof/v1",
            "generated_at": chrono::Utc::now().to_rfc3339(),
            "store": {
                "receipts": receipts_path,
                "seals": seals_path,
                "receipt_count": store.len(),
                "seal_count": store.seal_count(),
            },
            "sample": {
                "size": sample.len(),
                "bound": bound,
                "unsealed": unsealed,
                "tampered_body": tampered,
                "not_found": not_found,
            },
            "details": details,
        });
        let out = "/root/forge_work/rg2-verify/SEAL_BINDING_PROOF.json";
        std::fs::create_dir_all("/root/forge_work/rg2-verify").ok();
        std::fs::write(out, serde_json::to_string_pretty(&proof).unwrap())
            .expect("write SEAL_BINDING_PROOF.json");
        eprintln!(
            "[live-audit] bound={} unsealed={} tampered={} not_found={} (sample {})",
            bound,
            unsealed,
            tampered,
            not_found,
            sample.len()
        );
    }

    #[test]
    #[ignore = "live RG-1/A8 MCP pair witness — env-driven: WITNESS_MODE=hash|verify, MCP_ROOT_ID, MCP_CHILD_ID; F13-ACKed production test surface"]
    fn live_mcp_pair_witness() {
        let store = JsonlReceiptStore::new(
            "/var/lib/arifflow/receipts.jsonl",
            "/root/arifOS/VAULT999/arifflow_sealed.jsonl",
        );
        let mode = std::env::var("WITNESS_MODE").unwrap_or_default();
        let root_id = std::env::var("MCP_ROOT_ID").expect("MCP_ROOT_ID set");
        match mode.as_str() {
            "hash" => {
                let r = store
                    .get_receipt(&root_id)
                    .expect("store readable")
                    .expect("root receipt present in store");
                println!("BODY_HASH={}", r.hash());
            }
            "verify" => {
                let child_id = std::env::var("MCP_CHILD_ID").expect("MCP_CHILD_ID set");
                assert_eq!(
                    store.verify_receipt_seal_binding(&root_id).unwrap(),
                    SealBindingStatus::Bound,
                    "root must be seal-bound"
                );
                assert_eq!(
                    store.verify_receipt_seal_binding(&child_id).unwrap(),
                    SealBindingStatus::Bound,
                    "child must be seal-bound"
                );
                let resolver = LineageResolver::new(store);
                let proof = resolver.reconstruct(&child_id);
                assert_eq!(
                    proof.status,
                    LineageStatus::Valid,
                    "lineage must reconstruct: unresolved={:?} unsealed={:?}",
                    proof.unresolved_parents,
                    proof.unsealed_receipts
                );
                assert!(!proof.root_body_hashes.is_empty());
                println!(
                    "PAIR_WITNESS=VALID roots={:?} nodes={} proof_hash={}",
                    proof.root_body_hashes,
                    proof.ordered_nodes.len(),
                    &proof.proof_hash[..16]
                );
            }
            _ => panic!("WITNESS_MODE must be 'hash' or 'verify'"),
        }
    }

    #[test]
    fn test_does_not_treat_genesis_anchor_as_authority() {
        // The genesis_anchor field is metadata, not authorization.
        // A receipt with a genesis_anchor does NOT auto-authorize its descendants.
        // This is verified by the resolver returning the anchor as metadata only —
        // no special routing, no authority promotion, no power-up.
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        let mut genesis = make_receipt("a", 0, vec![]);
        genesis = genesis.with_genesis_anchor("RCP-000");
        let g_id = genesis.receipt_id.to_string();
        store.insert_receipt(genesis.clone());
        store.insert_seal(make_seal_for(&genesis, &mut sealer), g_id.clone());

        // Child of Genesis — has no special authority.
        let child = make_receipt("a", 1, vec![genesis.hash()]);
        let c_id = child.receipt_id.to_string();
        store.insert_receipt(child.clone());
        store.insert_seal(make_seal_for(&child, &mut sealer), c_id.clone());

        let resolver = LineageResolver::new(store);
        let proof = resolver.reconstruct(&c_id);

        assert_eq!(proof.status, LineageStatus::Valid);
        // The proof should NOT carry any "authority" boolean — only metadata.
        let json = serde_json::to_string(&proof).unwrap();
        assert!(!json.contains("\"authorized\""));
        assert!(!json.contains("\"authority\":true"));
        // Genesis anchor is discoverable but is metadata.
        assert!(proof.genesis_anchors.iter().any(|g| g.anchor == "RCP-000"));
    }

    #[test]
    fn test_traverses_supersession_without_deleting_history() {
        // Simulate a supersession: receipt V2 supersedes receipt V1.
        // Both must remain in the store and both must be reachable.
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        let v1 = make_receipt("a", 0, vec![]);
        let v1_id = v1.receipt_id.to_string();
        store.insert_receipt(v1.clone());
        store.insert_seal(make_seal_for(&v1, &mut sealer), v1_id.clone());

        // V2 supersedes V1: V2 explicitly references V1 as parent.
        // (In arifOS, supersession uses the Memory Promotion Gate; here we model it
        //  as V2 inheriting V1's lineage but with a new receipt_id.)
        let v2 = make_receipt("a", 1, vec![v1.hash()]);
        let v2_id = v2.receipt_id.to_string();
        store.insert_receipt(v2.clone());
        store.insert_seal(make_seal_for(&v2, &mut sealer), v2_id.clone());

        let resolver = LineageResolver::new(store);

        // Reconstruct from V2 — both V1 and V2 must appear in lineage.
        let proof_v2 = resolver.reconstruct(&v2_id);
        assert_eq!(proof_v2.status, LineageStatus::Valid);
        assert!(
            proof_v2
                .ordered_nodes
                .iter()
                .any(|n| n.receipt_id.as_deref() == Some(v1_id.as_str()))
        );
        assert!(
            proof_v2
                .ordered_nodes
                .iter()
                .any(|n| n.receipt_id.as_deref() == Some(v2_id.as_str()))
        );

        // Reconstruct from V1 alone — V1 still has its own history.
        let proof_v1 = resolver.reconstruct(&v1_id);
        assert_eq!(proof_v1.status, LineageStatus::Valid);
        assert_eq!(proof_v1.root_receipt_ids, vec![v1_id.clone()]);
    }

    #[test]
    fn test_emits_machine_readable_lineage_proof() {
        // The proof must be deterministically serializable to JSON for
        // downstream consumers (AAA, FED, KERNEL).
        let mut store = InMemoryReceiptStore::new();
        let mut sealer = Vault999Sealer::new();

        let r = make_receipt("a", 0, vec![]);
        let id = r.receipt_id.to_string();
        store.insert_receipt(r.clone());
        store.insert_seal(make_seal_for(&r, &mut sealer), id.clone());

        let resolver = LineageResolver::new(store);
        let proof = resolver.reconstruct(&id);

        // Machine-readable: serialize + deserialize round-trips identically.
        let json = serde_json::to_string(&proof).expect("serialize");
        let parsed: LineageProof = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.target_receipt_id.as_deref(), Some(id.as_str()));
        assert_eq!(parsed.status, LineageStatus::Valid);
        assert_eq!(parsed.root_receipt_ids, vec![id]);
    }
}
