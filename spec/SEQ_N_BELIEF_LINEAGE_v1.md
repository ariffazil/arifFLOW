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

## RG-4 addendum (2026-09-13, commits 722c695 + scripts 89babc6)

Governance events are now first-class graph nodes:

- **fire-seal.py** emits structured governance receipts on ALL outcomes —
  `seal`, `seal_refused` (judge refusal), `bind_failed`. A refusal is a
  RECORD, not silence. Payload: `{governance_event, mode, verdict, chain_id,
  judge_state_hash, seal_purpose, f13_ack, actor, payload_head}`; lane-chained
  hash-bound (`edges/fire-seal-lane-a.last`). NOTE: fire-seal `--mode receipt`
  returns no `verdict` field from the kernel (pre-existing untested path) —
  emission fires on real `mode=seal` arcs (verdict SEAL/SABAR).
- **`POST /gov_events`** + MCP `flow_gov_events`: scan of
  `payload.governance_event` receipts with per-event supersession status and
  as-of time travel.
- **The junction (production-witnessed)**: a governance verdict revised by a
  later receipt reads as `belief_status: superseded, superseded_by:
  [verified]` — judge-refused-then-ratified is a computable governance
  belief death.
- Daemon quirk recorded: single-read socket can split header/body →
  transient `400 EOF` — retry once (the MCP bridge has always done this).

## RG-5 addendum (2026-09-13, commit 442252b)

Scar-bound policies — `payload.scar_binding` receipts citing the scar they
were compressed FROM and the surface where they are ENFORCED. Query:
`POST /scar_policies` + MCP `flow_scar_policies` (scan + causal parents +
supersession + as-of). Production witnesses (session rg5-witness):

- `retry-on-transient-400` ← scar deploy-race-20260913; enforced in
  scripts/fire-seal.py commit 8210114 (REAL CODE); causally parented to the
  400-closure receipt 7946ddaf — parent hash computed CLIENT-SIDE in Python
  and VERIFIED BY THE RUST DAEMON at ingest: cross-language JCS parity
  proven in production (python-computed == rust-recomputed).
- `no-pipe-exit-read` ← scar pipe-swallows-exit-20260913 (near-miss ×2 in
  one session); enforcement: verifier discipline. Honest gap: no receipt
  ancestor (process scar, not yet receipted) — parents=[].
- `move-dont-delete-evidence` ← SEALED scar scar_1789211553883
  (fp e4d9fc5be73367cd) — the bridge from Reality Graph to the existing
  VAULT999 scar machinery.

The chain `reality → scar → policy → enforced behaviour` is now traversable:
council layer map RG-8 seeded. Policies superseded by later policies read as
policy belief death. Known polish: /lineage target node shows jcs_body_hash
None for unstamped receipts (lazy compute exists only on edge verification).

## RG-7 addendum (2026-09-13, commit 08cbf1f)

Consequence records — reality's invoice as a first-class object.
`payload.consequence` receipts: claim_slug, observed_outcome, evidence,
outcome_class (recovery|regression|neutral), attributed_to (causal parents =
the policy/execution receipts that produced the outcome). Query:
`POST /consequences` + MCP `flow_consequences`. Attribution stays a
falsifiable CLAIM via the evidence field.

Production witness (session rg7-witness): the full metabolic loop traversed
by real artifacts —
`400 failure → scar → policy 10068702 → retry behavior → killer receipt
7b65a115 live → invoice 2207231e (attributed to BOTH, hash-bound edges)`.
Second invoice 98dc28e7 records the unplanned in-production JCS parity
(Python-computed hash verified by Rust daemon). As-of control: count=0 at
the policy — reality had not yet replied.

What remains: FQ_G (institutional metabolism rate) — measure LAST, per the
sovereign's own rule. The substrate for measuring it now exists: beliefs
born / revised / superseded / killed, policies compressed from scars,
invoices issued — all queryable, all hash-bound.
