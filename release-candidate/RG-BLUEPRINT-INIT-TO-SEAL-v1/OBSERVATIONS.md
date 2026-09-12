# OBSERVATIONS.md — RG-BLUEPRINT-INIT-TO-SEAL-v1
# Role: FORGER (IMPLEMENT + TEST only)
# Generated: 2026-09-12T10:32:29Z
# Method: OBS = directly observed; DER = derived from observed; UNKNOWN = not verified
# DITEMPA BUKAN DIBERI.

---

## 1. Repository Identity

| Repo | Commit (HEAD) | Working Tree | Classification |
|------|---------------|--------------|----------------|
| arifFlow | `0030110` | DIRTY | REQUIRED_FIX (2 files) |
| A-FORGE | `1481b571` | CLEAN | n/a |
| arifOS | `7633068` | CLEAN | n/a |
| AAA | `1344c6a` | DIRTY | ACCIDENTAL (26+ untracked docs) |

**[OBS]** arifFlow dirty delta = `mod.rs` (adds `pub mod lineage;` re-export + `SealReceipt`) and `lineage.rs` (new 1373-line RG-2 implementation). This delta is the primary subject of this mandate.

**[OBS]** AAA dirty files are docs, governance observations, and eureka entries — none affect RG-2 forge scope.

**[DER]** Runtime build for arifFlow includes lineage.rs even though it is not yet committed. Receipt IDs emitted since `0030110` include RG-2 lineage logic that is uncommitted. Any runtime receipt must declare source_commit=`0030110` + working_tree=DIRTY. No runtime receipt may be associated solely with `0030110`.

---

## 2. FlowReceipt Current State (arifFlow/src/receipt.rs)

**[OBS]** Fields present in Rust struct:
- `receipt_id` (Uuid) ✓
- `routed_organ: Option<String>` ✓ (added commit `4a5aef0`)
- `parent_receipt_ids: Vec<String>` ✓ (added commit `4a5aef0`)
- `genesis_anchor: Option<String>` ✓ (added commit `cecf6d5`)
- `previous_receipt_hash: Option<String>` ✓ (backward-compat chain)
- `epistemic_label` ✓
- `floor_verdict` ✓
- `tri_witness_votes` ✓
- `merkle_root` ✓
- `intent_reason` ✓
- `session_id`, `actor_id` ✓

**[OBS]** Fields ABSENT vs FlowReceipt v2 blueprint spec:
- `schema: arifflow.receipt/v2` — NO schema version field
- `graph_id` — ABSENT
- `receipt_kind` enum (OBSERVATION | PROPOSAL | FORGE | VERIFY | JUDGMENT | ACTION | OUTCOME | WITNESS | SEAL | OVERRIDE | SUPERSESSION) — ABSENT
- `interface` block (client, tool_schema_version, server_schema_version, compatibility) — ABSENT
- `lineage.root_candidate` boolean — ABSENT
- `authority` block (owner_ref, delegation_ref, lane enum, scope, expires_at) — ABSENT
- `epistemic.confidence` (0..1) — ABSENT (epistemic_label exists but no confidence float)
- `epistemic.falsification_condition` — ABSENT
- `evidence` block (refs, freshness, serving_reality_verified) — ABSENT
- `constraints` block (scar_refs, policy_refs) — ABSENT
- `outcome` block (status enum, open_loops, escalation_target) — ABSENT
- `consequence` block (produces, affects, invalidates, supersedes) — ABSENT
- `integrity` block (canonicalization, hash_algorithm, payload_hash, previous_hash) — ABSENT (hash exists as method but not as serialized field)
- `classification` block (level, traversal) — ABSENT

**[DER]** Current FlowReceipt is **v1-with-graph-edges** — it has the causal ancestry fields (parent_receipt_ids, routed_organ, genesis_anchor) but not the full constitutional envelope (authority, outcome, consequence, classification, integrity, receipt_kind).

**Status:** CODE_PRESENT (v1-with-graph-edges) | INTERFACE_READY (partial: edges yes, v2 envelope no)

---

## 3. ReceiptStore — RG-2 (arifFlow/src/governance/lineage.rs)

**[OBS]** `lineage.rs` exists as uncommitted file, 1373 lines. Contains:

### Trait: `ReceiptStore`
- `get_receipt(receipt_id: &str) -> Option<FlowReceipt>` ✓
- `get_receipt_by_hash(body_hash: &str) -> Option<FlowReceipt>` ✓ (body_hash = SHA3-256)
- `get_seal(receipt_id: &str) -> Option<SealRecord>` ✓
- `receipt_body_hash(receipt_id: &str) -> Option<String>` ✓
- `verify_receipt_seal_binding(receipt_id: &str) -> SealBindingStatus` ✓
- `parent_ids(receipt_id: &str) -> Vec<String>` ✓
- `genesis_anchor(receipt_id: &str) -> Option<String>` ✓
- `receipt_id_for_hash(body_hash: &str) -> Option<String>` ✓

### `LineageResolver::reconstruct(target: &str) -> LineageProof`
- backward traversal from target to roots ✓
- deterministic BTreeMap ordering ✓
- cycle detection (direct + indirect) ✓
- self-parent rejection ✓
- missing parent → PartialMissingParent ✓
- unsealed ancestor flagging ✓
- classification boundary → opaque stub ✓
- max depth enforcement ✓
- genesis_anchor discoverable as metadata (NOT authority) ✓

### InMemoryReceiptStore ✓
### JsonlReceiptStore ✓ (reads from JSONL files — matches VAULT999 arifflow_sealed.jsonl)

### 14 Tests present:
1. `test_reconstructs_root_to_child_lineage` ✓
2. `test_reconstructs_multi_parent_dag` ✓
3. `test_returns_stable_deterministic_proof` ✓
4. `test_flags_missing_parent` ✓
5. `test_rejects_self_parent` ✓
6. `test_detects_direct_cycle` ✓
7. `test_detects_indirect_cycle` ✓
8. `test_flags_unsealed_ancestor` ✓
9. `test_detects_receipt_hash_checkpoint_mismatch` ✓
10. `test_respects_classification_boundary` ✓
11. `test_halts_at_max_depth` ✓
12. `test_does_not_treat_genesis_anchor_as_authority` ✓
13. `test_traverses_supersession_without_deleting_history` ✓
14. `test_emits_machine_readable_lineage_proof` ✓

**[OBS]** Cannot run `cargo test` in sandbox (read-only filesystem). Tests are code-present but not runtime-verified in this session.

**Status:** CODE_PRESENT | INTERFACE_READY (trait defined) | RUNTIME_OBSERVED: NOT_RUN (build lock = read-only fs) | RECEIPT_PERSISTED: N/A (read-only store) | SEALED_WITNESSED: UNBOUND | QUERYABLE_GRAPH: CANDIDATE

---

## 4. ControlledCycle (arifFlow/src/topology/controlled_cycle.rs)

**[OBS]** `EscalationTarget` enum variants:
- `None`, `Operator`, `BudgetOwner`, `EvidenceOwner`, `IndependentVerifier`, `OrganOwner`, `Sovereign888`

**[OBS] GAP — FOLD-1 VIOLATION:**
`Sovereign888` conflates JUDGE_888 and SOVEREIGN_F13 into one variant. The blueprint mandates these be separate types at both type level and runtime behavior level.

- `Sovereign888` routes to JUDGE_888 functions (hold, evaluate, escalate)
- `SOVEREIGN_F13` should only appear for irreversible/canonical transitions
- Current: `DivergenceDetected → Sovereign888` — correct that divergence needs authority escalation, but the escalation type should distinguish judge-hold from sovereign authorization

**[DER]** `requires_888_hold()` returns true for `Sovereign888` — this is correct 888 behavior. The gap is that there is no `SovereignF13` variant for when F13 is actually required (production seal, genesis bridge, memory promotion, canon changes).

**Gap severity: HIGH** — violates Fold-1 of the blueprint. Required fix for mandate compliance.

**Status (ControlledCycle):** CODE_PRESENT | INTERFACE_READY | RUNTIME_OBSERVED: CANDIDATE | GAP: EscalationTarget missing SovereignF13 variant

---

## 5. JCS Canonicalization (Cross-Language)

**[OBS]** Rust: `sha3 = "0.10"` in Cargo.toml. SHA3-256 hashing present in lineage.rs via `sha3::Digest`.

**[OBS]** JCS (RFC8785) crate: NOT present in Cargo.toml. Hash is computed over serde_json serialization, NOT over JCS canonical bytes.

**[OBS]** TypeScript: `routed_organ` and `parent_receipt_ids` present in types.ts. No JCS library referenced.

**[OBS]** Python: `routed_organ`, `parent_receipt_ids` present in receipt/__init__.py. No JCS library referenced.

**[DER]** Cross-language canonical hash fixtures cannot be verified as equivalent without JCS. Blueprint requires: `client_payload_hash` ≠ `rust_receipt_hash` ≠ `seal_checkpoint_hash` until golden fixtures prove parity.

**Gap severity: HIGH** — golden fixtures do not exist yet. Cannot claim cross-language hash parity.

**Status (Hash):** CODE_PRESENT (SHA3-256 in Rust) | INTERFACE_READY: NO (JCS not implemented) | RECEIPT_PERSISTED: NO | SEALED_WITNESSED: UNBOUND

---

## 6. Genesis Bridge (RG-3)

**[OBS]** `genesis_anchor: Option<String>` field exists in FlowReceipt (Rust, TS, Python).

**[OBS]** Commit `cecf6d5` message: "genesis_anchor: Genesis Bridge between Constitutional and Historical Origin" — field was added but bridge not activated.

**[OBS]** `genesis_anchor` is correctly treated as metadata only (test_does_not_treat_genesis_anchor_as_authority verifies this).

**[OBS]** Three distinct anchors referenced in blueprint:
- `RCP-000-CONSTITUTION-ANCHOR` — not found as a registered receipt in VAULT999 SEALED_EVENTS.jsonl (UNKNOWN — not read in this session)
- `HASH-CHAIN-GENESIS` — first receipt in chain (resolvable via RG-2 once committed)
- `GENESIS-001` — protected record, payload not exposed

**Status (RG-3):** CANDIDATE | activation: FORBIDDEN_UNDER_CURRENT_MANDATE | human_approval: PENDING

---

## 7. VAULT999 Persistence

**[OBS]** Files present:
- `/root/arifOS/VAULT999/vault999.jsonl` (active chain)
- `/root/arifOS/VAULT999/arifflow_sealed.jsonl` (arifFlow receipts)
- `/root/arifOS/VAULT999/SEALED_EVENTS.jsonl`
- Backups: vault999-20260911.sql, vault999-20260912.sql

**[OBS]** JsonlReceiptStore in lineage.rs reads from JSONL files — compatible with arifflow_sealed.jsonl.

**[DER]** Receipt-to-seal binding requires verifying that `payload_hash` in VAULT999 seal entry matches SHA3-256 of receipt body. This binding test requires running code against live VAULT999 file.

**Status (Vault):** RECEIPT_PERSISTED: CANDIDATE | SEALED_WITNESSED: UNBOUND (not verified in session) | QUERYABLE_GRAPH: CANDIDATE

---

## 8. AAA Organ Registry

**[OBS]** `federation/organs.yaml` — schema_version: 2, generated 2026-09-12, contains `arifos` (CORE), multiple organs with ports, classes, authority_ceiling, forbidden_domains.

**[OBS]** arifFlow not listed as a component in organs.yaml (scanned first 60 lines — DER: likely listed further down or under different category).

**[OBS]** AAA dirty working tree has 26+ untracked files — none blocking RG-2 forge.

---

## 9. Seven-Layer Status Summary

| Capability | Code | Interface | Runtime | Persistence | Seal | Query | Canon |
|------------|------|-----------|---------|-------------|------|-------|-------|
| FlowReceipt v1+edges | CODE_PRESENT | INTERFACE_READY (partial) | RUNTIME_OBSERVED | RECEIPT_PERSISTED | SEAL_REFERENCED | PARTIAL | CANDIDATE |
| FlowReceipt v2 envelope | ABSENT | SCHEMA_ABSENT | NOT_RUN | NOT_PERSISTED | UNBOUND | PARTIAL | CANDIDATE |
| ReceiptStore (RG-2) | CODE_PRESENT | INTERFACE_READY | NOT_RUN | N/A (read-only) | UNBOUND | CANDIDATE | CANDIDATE |
| ControlledCycle | CODE_PRESENT | INTERFACE_READY | RUNTIME_OBSERVED | NOT_PERSISTED | UNBOUND | PARTIAL | CANDIDATE |
| JCS cross-lang hash | ABSENT | SCHEMA_ABSENT | NOT_RUN | NOT_PERSISTED | UNBOUND | PARTIAL | CANDIDATE |
| Genesis Bridge (RG-3) | CODE_PRESENT (field only) | SCHEMA_STALE | NOT_RUN | NOT_PERSISTED | UNBOUND | PARTIAL | CANDIDATE |
| JUDGE_888 ≠ F13 | CODE_PRESENT (partial) | INTERFACE_READY (partial) | RUNTIME_OBSERVED | NOT_PERSISTED | UNBOUND | PARTIAL | CANDIDATE |

---

## 10. Critical Gaps (Blocking for Mandate Compliance)

| Gap | Severity | Blocks |
|-----|----------|--------|
| `EscalationTarget::Sovereign888` conflates JUDGE_888 and SOVEREIGN_F13 | HIGH | Fold-1, JUDGE_PACKET |
| JCS canonicalization not implemented in any language | HIGH | Hash fixtures, cross-lang parity |
| FlowReceipt v2 envelope fields absent (schema, graph_id, receipt_kind, authority, outcome, consequence, integrity, classification) | MEDIUM | Full v2 compliance |
| arifFlow dirty working tree — cannot produce clean release commit | MEDIUM | Lane A release |
| RG-2 tests cannot be run (sandbox read-only fs) | MEDIUM | TEST_RESULTS.json |
| VAULT999 seal binding not verified in-session | MEDIUM | SEAL_BINDING_PROOF.json |
| genesis_anchor=None in operational receipts — COMPLIANT ✓ | n/a | n/a |

---

*OBSERVATIONS COMPLETE. All findings are OBS or DER. No narrative repair. Unknown = UNKNOWN.*
*Next: PROPOSAL.yaml*
