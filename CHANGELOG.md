## v2026.11.22 (rev 2) — one FQ formula, one source of truth

**Changed**
- `receipt::classify_quotient(verify_count, execute_count)` is now the ONLY
  FQ band decision. The system window (`FlowQuotient::compute`) and the
  per-actor enforcer (`ActorFlowState`) both call it. The threshold table
  existed in two places, so the health surface could answer two different
  things about the same actor.
- An actor with no steps reports `Unknown` (undefined) instead of the legacy
  `Balanced` alias.
- `metric_semantics` gains the canonical descriptor: `fq_version`,
  `quotient_type`, `producer`, `authority_ceiling`, plus per-metric `window`
  and `producer`.
- `GET /health` gains `verdict_grammar: "observed_diagnosis"` and a
  `verdict_note`, so the top-level `verdict` cannot be read as a
  constitutional verdict.

**Added**
- `tests/fq_single_formula.rs`: proves the per-actor band and the canonical
  function agree across the full boundary matrix (execute 0–4 × verify 0–6),
  so the two can never silently diverge again.

**Tests:** 296 passed / 0 failed (285 lib + 3 + 6 adversarial + 2 single-formula).

## v2026.11.22 — SOT manifest, grammar separation, P3 quotient exclusion

**Added**
- `GET /health` → `sot`: provenance manifest (repo_sha, release_id,
  binary_digest_hex, formula_version, policy_version, receipt_format_version,
  deployed_at, witnessed_by).
- `GET /health` → `capabilities`: explicit status of the four daemon paths
  that were compiled-but-inactive (arifOS bridge, A-FORGE bridge, Merkle
  checkpointing, BSP scheduler) — no silent claim drift.
- `GET /health` → `metric_semantics`: population/unit/formula/direction for
  every FQ-shaped number.
- `GET /health` → `fq.per_actor[*].excluded_verify_count`.
- `POST /check` → `gate_state` (ALLOW/FLOW_BLOCKED/THROTTLED/COOLDOWN) and
  `grammar` manifest. `action` retained, flagged `action_deprecated`.
- `POST /ingest` → independent witness gate: `OUTCOME_WITNESSED` requires
  `payload.witnessed_by`, else 400.
- `src/grammar.rs`, `src/provenance.rs`; `tests/adversarial_fq.rs`.

**Fixed**
- P3 quotient exclusion: read-only (T0Observe) and session-orphan verifies
  are no longer counted in the FQ numerator — closing the one-way ratchet that
  could freeze an actor in FOSSILIZED with no path out.
- `receipt.rs` module docstring described a cost-based FQ formula the code
  stopped implementing in v2.1; it now documents the count-based v2.2 formula
  and the seven-state band.

**Changed**
- `Cargo.toml` version 2026.7.26 → 2026.11.22; README release markers aligned
  (v2026.11.21 / v2026.11.14 → v2026.11.22).

**Known gap (pinned by a characterization test)**
- Replayed receipts are still accepted at `push_chain_aware`; de-duplication
  happens downstream. Changing that alters ingest semantics for every emitter
  and needs a gated window (888).

**Not done (declared, not hidden)**
- Live cross-organ bridge wiring, daemon-path Merkle checkpointing and BSP
  scheduler remain inactive; see `/health` → `capabilities`. Wiring them
  changes live authorization/execution paths and needs a gated window (888).

# Changelog — arifFlow (Orchestration)

All notable changes to the arifFlow organ.

## [Unreleased]

### Added
- Axiom 14 — governed uncertainty (STATE_BELOW_WORDS E1–E18)
- Eureka specification for state management

## [2026.08.30] — Federation Orchestration

### Added
- Workflow orchestration pipeline
- Cross-organ flow control
- Health monitoring

## [2026.07.13] — Initial Release

### Added
- arifFlow organ initialization
- Pipeline management
- Ingestion routing
