# RELATIONSHIP_EDGE_SCHEMA_v0.1 — P1: Relationship Edges as Reality Graph Receipts

> **Status:** v0.1 — P1.2 force vocabulary added (F13 binary 2026-09-24 chat:
> "buat P1.2, Five Living Forces sebagai predicate vocabulary").
> v0 — F13 binary approved 2026-09-24 chat ("1 — bina P1 atas RG receipts").
> **Owner schema:** FI-003 (builder) · Substrate owner: arifFlow daemon (no daemon change in v0).
> **Authority evidence:** F13 chat binaries + `/root/forge_work/2026-09-24-FI-003-small-world-helix-realitygraph-linkage.md`.
> **Absorbed:** parallel-lane v2 audit (`parent_assertion_ids`, write-permission matrix, identity
> capability levels, witnessing≠claiming) · "Genesis" 5-frame analysis (as QUERY FRAMES, not a
> new graph name — canon: agents may not mint Reality Graph aliases) · Five Living Forces
> (`SOVEREIGN-HUMAN-REALITY-MEMORY-ARCHITECTURE.md`, sha256 e2173f9a…) as `force:*` predicate
> vocabulary (§2b, P1.2).

---

## 1. Design

A relationship edge IS a FlowReceipt. No new store, no new endpoint, no daemon mutation in v0.

```
POST :7073/ingest  (existing FlowReceipt contract, FLOW_RECEIPT_v1.md)
  payload.relationship_edge = { ...edge assertion below... }
  parent_receipt_ids / parent_receipt_hashes = causal + evidence edges (RG-PH hash-chained)
  routed_organ = "AAA" (APEX citizen directive: AAA maintains Relationship/Reality Graph)
  epistemic_label = OBS | DER | INT | SPEC  (edge status inherits receipt epistemics)
```

Rides everything already live: hash-chained `jcs_body_hash`, SEQ-N lineage traversal,
RG-4 governance events, RG-5 invalidation (an edge can DIE — belief-death machinery
applies to relationships), RG-7 consequence attribution.

## 2. Edge assertion (payload.relationship_edge)

| Field | Type | Rule |
|---|---|---|
| `edge_id` | string | `rel:<uuid4>` |
| `q_frame` | enum | `identity \| relationship \| authority \| consequence \| reality` — the five query frames over ONE Reality Graph (NOT five graphs, NOT a new name) |
| `subject` / `object` | string | canonical IDs (`human:*`, `org:*`, `agent:*`, `canon:*`) — never free-text names |
| `predicate` | string | namespaced: `employed_by`, `bond:witnessed`, `kin:family_of`, `delegates`, `affects`, `schema_bound_to` … |
| `witness` | list | who witnessed the assertion (human:*, agent:*) — empty = INVALID |
| `basis` | enum | `sovereign_testimony \| subject_testimony \| witnessed_observation \| documented_artifact \| self_reported` |
| `confidence` | obj | `{value, band: CONFIRMED\|PLAUSIBLE\|SPECULATIVE, basis}` — F2 bands |
| `temporal` | obj | `{asserted_at, valid_from, valid_until, status: proposed\|active\|disputed\|expired}` |
| `zkpc_class` | enum | `CONTENT_SEALED \| SHADOW_CATEGORY_ONLY \| OPEN` — structure-not-content |
| `sensitivity` | enum | `private \| workspace \| public` |
| `consent` | enum | `sovereign \| required \| granted` |
| `authority_implication` | const | **always `"none"` for relationship frame** (hard rule) |
| `grant_ref` | string? | REQUIRED iff `q_frame=authority` — explicit scoped grant evidence |
| `note` | string? | category-level only when sensitivity=private |

## 2b. Force vocabulary (P1.2 — Five Living Forces as `force:*` predicates)

Source: Five Living Forces (agy architecture doc, sha e2173f9a…). Canonization discipline
applied per term (4 tests: semantic delta · operational delta · testability · non-overlap).
**All five PROMOTED TO CANON 2026-09-25 (F13_RATIFIED_CHAT):**
`/root/AAA/canon/FIVE-LIVING-FORCES.md` (sha256 4130c517…, canon-mutate receipt 76aeb1e3).
Rule: a force edge is a
**pointer into its existing organ home**, never a copy (helix pointer pattern).

| Force | Semantic delta | Required field (operational) | Organ home (non-overlap) | Test |
|---|---|---|---|---|
| `relationships` | pressure of reciprocal bond — not mere connection | `--frame relationship` enforced | this schema (the edge registry itself) | frame mismatch → refuse |
| `commitments` | human-owned promise ≠ CHRON prediction ≠ session carry-forward | `--valid-until` (ISO deadline) | CHRON `verify_at` + state-transition object contract | missing deadline → refuse |
| `scars` | lived cost that forbids repetition — pointer only, never re-stored | `--scar-ref` (existing scar id) | VAULT999 H5 / RG-5 scar-bound policies | missing ref → refuse |
| `constraints` | standing bound on interaction (biological/temporal scope), ≠ one-off refusal | `--note` (scope of the bound) | WELL H-plane bounds | empty scope → refuse |
| `open_questions` | human-owned unresolved tension ≠ system open loop | `--chron-ref` OR `--parent` | CHRON events / kernel `open_loops_888_HOLD` | no anchor → refuse |

Emitter: `--force <name>` + `--scar-ref/--chron-ref/--valid-until`. Force rides in
`payload.relationship_edge.force`. Guards are client-side v0 (same as §3); daemon
validation = v1. **F5 hard rule unchanged:** private human forces stay ZKPC
(SHADOW_CATEGORY_ONLY or CONTENT_SEALED).

## 3. Hard rules (client-enforced in `relationship_edge_emit.py`; daemon validation = v1)

- **R1** Relationship may contextualize authority, never confer it. `authority_implication` is forced `none` unless `q_frame=authority` with `grant_ref` present.
- **R2** No relationship from name overlap — `basis=inferred`-class assertions require an explicit witness and are auto-banded SPECULATIVE.
- **R3** No consequence without evidence — `q_frame=consequence` requires `--parent` (a real receipt), else refuse.
- **R4** No authority edge without a scoped grant — `q_frame=authority` without `--grant-ref` → refuse.
- **R5** Private human frames require `zkpc_class != OPEN` — no raw content for P-axis persons (F5).
- **R6** Subject-sealed always wins: a `subject_testimony` edge supersedes `witnessed_observation` at same predicate (supersession via RG-5 parent edges, never deletion).
- **R7** Witnessing ≠ claiming — an agent may witness; only sovereign/subject testimony seals.

## 4. What v0 does NOT do

- No personal-edge seeding without owner testimony (P-axis mutations await Hermes-owner/F13 direct testimony).
- No daemon-side schema validation (receipts still accept arbitrary payloads — v0 guards are client-side only; treat emitted edges as SPEC-grade until v1).
- No topology metrics (P2), no plasticity ledger (P3).

## 5. Emitter

`/root/scripts/relationship_edge_emit.py` — mirrors the proven carry_forward emitter:
edge state `/root/.local/share/arifflow/edges/<lane>.last`, capture daemon response,
write `receipt_id jcs_body_hash` only on `"status":"ingested"` (a rejected receipt can
never become a parent). `--dry-run` prints without POST.

## 6. Verification ladder (F2)

| Level | Test | Status |
|---|---|---|
| 1 | Emitter dry-run renders schema-valid receipt | build gate |
| 2 | Live ingest 200 + `status:ingested` + `jcs_body_hash` returned | build gate |
| 3 | Edge state file written with id+hash | build gate |
| 4 | `/lineage` traversal reaches the edge from a parent | v1 (needs organic parent) |
| 5 | Independent FI re-emit + cross-check | pending (C4 independence) |
