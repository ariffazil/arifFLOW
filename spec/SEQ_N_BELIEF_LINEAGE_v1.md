# SEQ-N Belief Lineage v1 — ancestry, time travel, belief death

schema: arifflow.spec/seq-n-belief-lineage/v1
status: IMPLEMENTED + PRODUCTION-WITNESSED (2026-09-13)
work_order: F13 directive "sambung SEQ-N"
authored: fi-003-qwen
prerequisite: P4-JCS (b5c6ee9) + RG-PH (2a696e0) — deterministic bytes, then bound edges

## Purpose

The council's acceptance test, answered from receipts alone — never narrative:

> "Can every important belief point to its receipt ancestry, and can we reconstruct
> why a belief was born, changed, and finally died — without human narrative?"

## Three surfaces

### 1. Ancestry with per-edge verification (`POST /lineage`, MCP `flow_lineage`)

`{"receipt_id": "…"}` → BFS ancestry walk; every DAG edge carries a verdict:

| verdict | meaning |
|---|---|
| `verified` | claimed parent hash == recomputed canonical hash — the "because" is cryptographically bound |
| `mismatch` | claimed hash ≠ actual content — visible perjury in the historical record |
| `unverified` | edge predates hash-chaining (or no hash claimed) — honest gap, never faked |
| `missing_parent` | parent id not in ledger |

### 2. Time travel (`before_receipt_id`)

Inclusive as-of boundary: receipts after that ledger position are INVISIBLE —
including their supersessions. A death that happened after the boundary did not
happen yet, as far as that query is concerned. Unit-proven + production-witnessed
(same belief: `active` as-of itself, `superseded` now).

### 3. Belief death (`supersedes_receipt_ids` + `supersedes_receipt_hashes`)

Visible, non-deleting supersession on FlowReceipt:

- target stays in the ledger forever; the edge records that its belief DIED
- hash-bound: forged death (wrong target content hash) → 400
  `"refusing to record a false death"` — same posture as forged parents
- self-supersession rejected; ids-only legacy path accepted (unverified)
- `belief_status`: `active | superseded`, computed within the as-of window

## Production witnesses (2026-09-13, daemon seq-n build)

1. **Full session ancestry**: 7 nodes, depth 6 — the entire RG-1.5→SEQ-N arc
   reconstructed live; newest edge `verified`, older edges honestly `unverified`
   (they predate hash-chaining — the report shows WHEN verification began).
2. **Real belief death**: belief "hook read-failure cause UNKNOWN" (c2dfcded) →
   killed by "cause IDENTIFIED: missing ';'" (e9a56474) — both causal AND death
   edges hash-bound; the supersession records an actual belief revision that
   happened during this session.
3. **Forged death control**: 400 refused.
4. **Time travel**: same belief `active` as-of itself; `superseded/verified` now.
5. **MCP stdio**: `flow_lineage` through the full JSON-RPC stack returns the verdict.

## Invariants

- Queries are READ-ONLY — no ingest path touched.
- Ledger append order = time order (as-of semantics); the sealed chain remains
  the ordering authority (this schema does not alter `FlowReceipt::hash()` or
  the seal chain — scope-of-protection doctrine unchanged).
- Unparseable ledger lines are skipped, not fatal — the query witnesses what
  parses; gaps are countable (`receipts.len()` vs file lines).

## Open (next)

- Resolver-side integration (arbitrate tier for supersession conflicts: two
  receipts both claiming to supersede the same belief = competing beliefs —
  currently both visible, no winner-selection semantics).
- Emitter adoption: fire-seal receipts could supersede prior draft-receipts.
- FQ_G (RG-7) remains last, per the measure-last rule.
