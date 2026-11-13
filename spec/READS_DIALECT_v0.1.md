# READS_DIALECT_v0.1 — Observed Causal-Access Receipt

> **Status:** v0.1 DRAFT — for F13 review before any `receipt.rs` / `federation_hook_engine` mutation.
> **Owner schema:** 333-AGI (forger) · Substrate owner: arifFlow daemon (receipts.jsonl) + federation_hook_engine (observer).
> **Authority evidence:** live conversation thread 2026-11-12 (333-AGI ↔ sovereign relay); F13 green-light on the
> DRAFT route only — no substrate mutation is authorised by this document.
> **Absorbed:** Dagster asset-vs-observation distinction (declared lineage ≠ observed access) · correction round
> 2026-11-12 (the `reads` edge is a *causal-access dialect*, NOT a `relationship_edge` `q_frame=relationship`
> predicate — that frame is reserved for reciprocal bond, per `RELATIONSHIP_EDGE_SCHEMA_v0.2` §2b) ·
> 888 review 2026-11-12 (acceptance gates A1–A7/R1–R6/B1–B6 folded as §10; hash-class split + organ_scope
> added per verdict).
> **Baseline frozen:** `fixtures/reads-baseline-f338d351-2026-11-12.json`.

---

## 0. The one-sentence problem

arifFlow already witnesses **what really changed** (instrumented WRITE consequence via
`federation_hook_engine`). It does not yet witness **what was really read**. Declared `parent_receipt_ids`
tell us what an execution *claims* to depend on; nothing tells us what it *actually consumed* at runtime.
The gap is a symmetry gap in the observer, not a missing lineage engine.

```
WRITE: actual mutation observed      LIVE   (federation_hook_engine, consequence string dialect)
READ:  actual consumption observed   MISSING
```

## 1. Ontology placement (binding)

```
relationship graph        = bond / authority / delegation / witnessed relation
causal receipt DAG        = production / consumption / mutation / supersession
```

A `read` is **mere connection** (dataflow). It belongs on the causal DAG substrate — the same place as
`parent_receipt_ids`, `parent_receipt_hashes`, consequence receipts, supersession, and hash verification.
It does **NOT** belong in `payload.relationship_edge`, and it does **NOT** mint a new `q_frame`.
This is a new **receipt dialect** read by `lineage_query.rs`, exactly the way the consequence string-dialect
was added in RG-7 (2026-10-28).

## 2. Receipt shape — `dialect: reads.v0.1`

```yaml
receipt:
  receipt_type: causal_observation
  dialect: reads.v0.1

  run_id: run:...            # R2: run-scoped
  actor: consumer-organ      # R2: actor-scoped
  tool: ...                  # the accessor (e.g. read_file, geox ingest)
  target: ...                # human-readable path/artifact label

  access:
    operation: read
    artifact_id: artifact:...      # resolves to a canonical artifact OR a prior receipt
    actual_sha256: sha256:...      # R4: from the bytes/object accessed, never copied from declared metadata
    bytes_observed: null | int
    observation_boundary: federation_hook_engine   # R1: the trusted boundary that minted it
    observed_at: ISO-8601

  parent_receipt_ids:
    - receipt:<artifact-source>    # R3: causal ancestry rides the existing parent edge

  parent_receipt_hashes:
    - sha256:...                   # may be null (client-populated; see baseline §7)

  witness:
    emitter: federation_hook_engine
    engine_version: ...
    event_id: ...                  # R6: unique per attempt, stable per run

  result:
    access_succeeded: true | false # R5: failed vs successful reads are distinguishable
    hash_verified: true | false
```

`parent_receipt_ids` stays the **causal ancestry** edge. The `access` payload describes **what was observed
to happen** — it does not duplicate the edge as a relationship record.

## 3. Invariants (v0.1)

| ID | Rule |
|----|------|
| READ-1 | A READ receipt is mintable only by a trusted runtime observer (`federation_hook_engine`), never by the consumer declaring it. |
| READ-2 | Every receipt is run-scoped (`run_id`) and actor-scoped. |
| READ-3 | The read artifact is referenced through `parent_receipt_ids` (causal edge), not a free-text name. |
| READ-4 | `actual_sha256` comes from the accessed bytes/object, not copied from declared metadata. |
| READ-5 | Failed reads and successful reads are distinguishable (`access_succeeded`). |
| READ-6 | Retry never creates ambiguity: `event_id` unique per attempt, attributable to one run. |
| READ-7 | Observed consumption never rewrites declared historical lineage. |
| READ-8 | Reconciliation is per-run, never global-only. |
| READ-9 | `consumed_by` is a derived reverse projection from observed READ receipts — never an authored field. |
| READ-10 | Observer coverage must be declared per run: `COMPLETE | PARTIAL | UNKNOWN`. Absence of a READ receipt is only "not observed" under COMPLETE coverage — otherwise it proves nothing. |

## 4. Reconciliation states (with coverage + hash-class discipline)

Two independent axes guard the verdict: **coverage** (READ-10) and **hash class** (three-way, not boolean):

```text
claimed hash present + matches   → VERIFIED
claimed hash present + mismatches → MISMATCH
claimed hash absent              → UNVERIFIED   (cannot be checked — NOT a mismatch)
```

```rust
match (declared, observed, coverage, hash_class) {
    (true,  true,  _,        VERIFIED)   => CORROBORATED,
    (true,  true,  _,        UNVERIFIED) => OBSERVED_UNVERIFIED,   // observed, but no claimed hash to check against
    (_,     true,  _,        MISMATCH)   => INTEGRITY_MISMATCH,
    (false, true,  _,        _)          => UNDECLARED_OBSERVED,
    (true,  false, COMPLETE, _)          => DECLARED_UNOBSERVED,
    (true,  false, _,        _)          => OBSERVATION_INCONCLUSIVE, // coverage PARTIAL/UNKNOWN
    (false, false, _,        _)          => NO_EDGE_EVIDENCE,
}
```

Response ladder (enforcement binds **after** query semantics stabilise — see §6 step 7):

```
CORROBORATED           → continue
OBSERVED_UNVERIFIED    → record; do not count as CORROBORATED (null claimed_hash ≠ proof of integrity)
DECLARED_UNOBSERVED    → record; no automatic HOLD (optional branch / cache / short-circuit are legitimate)
OBSERVATION_INCONCLUSIVE → record; never label DECLARED_UNOBSERVED (access may have happened outside the hook)
UNDECLARED_OBSERVED    → mark resulting transition verify | quarantine by risk class (real declaration gap)
INTEGRITY_MISMATCH     → hard HOLD for dependent materialization (declared dependency, wrong bytes)
```

Historical parent edges with empty `parent_receipt_hashes` (baseline §7) therefore settle to
`OBSERVED_UNVERIFIED` at best — READ-4 fixes **new** observations, never retroactively verifies old edges.

## 5. Downstream traversal (D3) — reverse adjacency, no new engine

`POST /lineage` is **upstream-only** today (see baseline §7). Add a direction parameter over the
same parent edges; build reverse adjacency by inverting `parent_receipt_ids` at read time.

```
POST /lineage
{ "receipt_id": "receipt:X", "direction": "downstream", "transitive": true, "include_superseded": false }
```

Conceptual response:

```yaml
root: receipt:X
direct_consumers: [ ... ]          # receipt-level closure — valid even when routed_organ is null
transitive_consumers: [ ... ]      # cycle-safe, deduplicated
affected_organs: [ ... ]
organ_scope: COMPLETE | PARTIAL | UNKNOWN   # UNKNOWN when any closure node has routed_organ null
integrity_summary: { verified, mismatch, unverified }
blast_radius_verdict: CONTAINED | PROPAGATED | UNKNOWN
```

Receipt-level blast radius is independent of organ attribution: a receipt whose `routed_organ` is null
is still a downstream consumer; only the *organ projection* degrades to `organ_scope: UNKNOWN`. Never
return an empty blast radius merely because organ tagging is missing (B4/B5).

This is what lets quarantine become artifact-scoped (HOLD only the causal closure that reaches X),
replacing today's coarser per-lane / global FQ HOLD.

## 6. Implementation scope (vertical extension, not greenfield)

```
1. federation_hook_engine  → mint reads.v0.1 causal-observation receipts (extend the live WRITE observer)
2. lineage_query.rs        → third dialect in the reader (today: object + string consequence → add reads)
3. lineage_query.rs        → index READ receipts by (run_id, actor, parent artifact)
4. lineage_query.rs        → reconcile declared parent vs observed read, per run
5. lineage_query.rs        → reverse adjacency from existing parent edges
6. POST /lineage           → add direction: downstream (transitive)
7. quarantine              → bind ONLY after reconciliation is proven free of false positives (READ-10)
```

Step 7 is deliberately last. Do not bind enforcement to a reconciliation whose observer coverage is not yet
proven complete — a false `DECLARED_UNOBSERVED` under PARTIAL coverage would HOLD healthy lanes.

## 7. Baseline (frozen 2026-11-12, `fixtures/reads-baseline-f338d351-2026-11-12.json`)

Observed current behaviour of `POST /lineage` — the pre-READ state, to be used as a regression fixture:

- **direction:** upstream only — no downstream/transitive primitive exposed. (D3 gap confirmed.)
- **edge verdicts default to `unverified`:** the target receipt's `parent_receipt_hashes` was empty
  (`claimed_hash: null`), so the edge could not be verified against a claimed hash. Hash-chaining is
  *supported* by the substrate but **not populated** by current emitters. This is a separate, pre-existing
  observation gap — READ-4 will not fix it, but reconciliation must not silently count `claimed_hash=null`
  edges as `CORROBORATED`.
- **`routed_organ: null`** on these receipts → the connectivity census classifies their edges as
  `unclassifiable`. Organ-scoped blast radius requires `routed_organ` population.

## 8. What v0.1 does NOT do

- Does not mutate `receipt.rs`, `federation_hook_engine`, or the daemon (spec-only).
- Does not mint a new `q_frame` or a new registry (rides receipts.jsonl + lineage_query.rs).
- Does not implement partitioned rematerialization (Dagster "backfill") — out of scope; we are not a
  time-series data platform.
- Does not bind quarantine to reconciliation yet (step 7 deferred).

## 9. Verification ladder (F2)

| Level | Test | Status |
|-------|------|--------|
| 1 | Baseline `POST /lineage` captured + frozen as fixture | PASS (this session) |
| 2 | `reads.v0.1` receipt parses in `LoadedLedger` as a third dialect | NOT RUN (needs code) |
| 3 | Reconciliation returns CORROBORATED for a declared+observed+hash-VERIFIED pair | NOT RUN |
| 4 | Observed + null `claimed_hash` settles to OBSERVED_UNVERIFIED, not CORROBORATED | NOT RUN |
| 5 | PARTIAL coverage yields OBSERVATION_INCONCLUSIVE, not DECLARED_UNOBSERVED | NOT RUN |
| 6 | `direction: downstream` returns transitive consumers for a known receipt | NOT RUN |
| 7 | Receipt-level closure survives `routed_organ: null`; organ_scope degrades to UNKNOWN | NOT RUN |
| 8 | `consumed_by` derived == reverse of observed READ parent edges (never authored) | NOT RUN |

## 10. Acceptance gates — D1/D2/D3 (888 review contract)

### D1 — READ observation
`A1` READ receipt minted by instrumented hook, not consumer declaration · `A2` causal-DAG substrate ·
`A3` `parent_receipt_ids` refers the actually-read artifact · `A4` `actual_hash` from accessed bytes/object ·
`A5` success vs failure distinguishable · `A6` run_id + actor + observation boundary recorded ·
`A7` observer coverage reported COMPLETE/PARTIAL/UNKNOWN.

### D2 — Reconciliation
`R1` per-run · `R2` null claimed hash never yields CORROBORATED · `R3` undeclared-observed has its own verdict ·
`R4` declared-unobserved only valid under COMPLETE coverage · `R5` historical receipts immutable ·
`R6` three-dialect parsing does not change interpretation of old fixtures.

Minimum verdict set: CORROBORATED · OBSERVED_UNVERIFIED · INTEGRITY_MISMATCH · UNDECLARED_OBSERVED ·
DECLARED_UNOBSERVED · OBSERVATION_INCONCLUSIVE · NO_EDGE_EVIDENCE.

### D3 — Downstream traversal
`B1` direct consumers returnable · `B2` transitive closure cycle-safe + deduplicated ·
`B3` superseded receipts gated by explicit param · `B4` receipt blast radius exists without organ attribution ·
`B5` null routed_organ → `organ_scope=UNKNOWN`, never empty blast radius · `B6` upstream response backward-compatible.

## 11. Pre-existing gaps — never credited to READ implementation

After D1 ships, report these four independently — READ gets no credit for fixing them:

```
READ observer status             IMPLEMENTED | NOT IMPLEMENTED
Parent hash population coverage  COMPLETE | PARTIAL | ABSENT   (emitters leave parent_receipt_hashes empty — baseline §7)
Organ routing coverage           COMPLETE | PARTIAL | ABSENT   (routed_organ null on many receipts)
Downstream traversal             IMPLEMENTED | NOT IMPLEMENTED
```

They may become prerequisites to enforcement later, but they do **not** block the D1 proof-of-observation.
