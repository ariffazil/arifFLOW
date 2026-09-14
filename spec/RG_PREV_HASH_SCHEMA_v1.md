# RG PREV_HASH Schema v1 — cryptographically-chained causal edges

schema: arifflow.spec/rg-prev-hash/v1
status: CANDIDATE — implementation landed 2026-09-13 (RG-PH), live witness pending
work_order: F13 directive "sambung prev_hash schema" (2026-09-13)
authored: fi-003-qwen
prerequisite: P4-JCS (b5c6ee9) — deterministic bytes before linkage

## Purpose

The Reality Graph differentiator over temporal knowledge graphs (Graphiti lineage):
**causal edges that bind content, not just identity.** Ids prove "I point at that
receipt"; hashes prove "that receipt's content was exactly this when I pointed."

```
Ledger proves sequence.   Graph proves causality.   Hashes bind both.
```

## Fields (FlowReceipt, additive, backward compatible)

| Field | Semantics |
|---|---|
| `jcs_body_hash: Option<String>` | Canonical JCS SHA3-256 of THIS receipt, computed over the receipt **with `jcs_body_hash` excluded** (block-header trick — self-reference is impossible; the exclusion IS the contract). Cross-language reproducible under `arifflow-jcs-v1`. **Daemon-stamped at ingest; client values are recomputed server-side, never trusted.** |
| `parent_receipt_hashes: Vec<String>` | Parallel to `parent_receipt_ids`, 1:1. The `jcs_body_hash` of each parent **at edge-creation time**. |

## Ingest semantics (daemon)

1. **Stamping:** on ingest, the daemon recomputes `jcs_body_hash` and overwrites
   any client value. Stamp failure (schema-discipline violation in the receipt,
   e.g. integer > 2^53) = WARN + stored unhashed (fail-soft; the legacy
   `FlowReceipt::hash()` chain is unaffected).
2. **Causal-edge verification** (`push_chain_aware`):
   - `parent_receipt_hashes.len() != parent_receipt_ids.len()` → **REJECT**.
   - Parent found in store + hash matches → edge accepted (verified).
   - Parent found + hash MISMATCH → **REJECT** — "refusing to record a false
     'because'." A mismatched edge is perjury, worse than no edge.
   - Parent outside the in-memory store window → WARN + accepted unverified
     (the edge stays queryable; verification simply did not happen — visible
     in logs, never silent).
   - No hashes supplied (legacy callers) → accepted as before (compat path).
3. **Response:** the ingest response now returns `receipt_id` + `jcs_body_hash`
   so emitters can chain hashes without re-reading the ledger.

## Canonical-form rule (for hash computation)

`canonical_form(receipt) = JCS(receipt minus jcs_body_hash)` — all other fields
included exactly as serialized (absent optionals omitted per skip rules). Any
language reproducing the bytes must apply the same single-key exclusion.

## Migration posture

- Legacy receipts remain valid and unhashed until touched; a backfill stamp is
  possible later but changes nothing about existing seals (three-names interim
  rule for `FlowReceipt::hash()` remains in force — this schema does NOT migrate
  the seal chain; that stays a separate F13 gate).
- Emitters (RG-1.5 wiring) need NO change: they may send ids only. Hash-bound
  edges become the default when emitters store the returned `jcs_body_hash`
  in their edge state (planned micro-step).

## Acceptance witness (pending → closes the council HOLD)

Fresh production pair: root receipt (daemon-stamped) → child carrying
`parent_receipt_ids + parent_receipt_hashes` → ingest 200 → both persisted with
fields → tamper simulation rejected (unit-level) → live happy-path witnessed on
the production daemon.
