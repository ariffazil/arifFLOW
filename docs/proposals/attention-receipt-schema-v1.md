# Proposal: Attention Receipt Schema v1 — `attention:{H,R,C,D,leak_class}`

> **Status: PROPOSAL — blocked on F13 ratification of ATTENTION-COST-DOCTRINE** (`/root/AAA/skills/ATTENTION-COST-DOCTRINE-PROPOSAL.md`).
> Do not merge until the doctrine is ratified. This PR exists so ratification → merge → deploy is one motion.
> Forged 2026-09-04 · kimi-code/FI-008 · queue item B (carry-forward SEAL-83defc585b5a4296).

## What

Add an optional first-class `attention` event to `FlowReceipt`, carried on
`Execute`/`Verify`/`Barrier`/`Route` steps, measuring the human channel:

| Field | Meaning |
|---|---|
| `h` | Human interventions this step (Arif had to act) |
| `r` | Rejections / rework requested by human |
| `c` | Clarification asks the agent made |
| `d` | Human-delay events (step waited on human decision) |
| `expected_turns` | Agent's declared prediction of H+R+C+D (up-front, guard #2 of the doctrine — the gap trains the world model, same as `forge_shell expected_output`) |
| `leak_class` | Optional taxonomy enum when the event fired a discovery leak |

`LeakClass` enum (contract-standard taxonomy, isomorphic to anomalous-contrast ISOLATE):

```rust
pub enum LeakClass {
    CapabilityDiscovery,   // agent could not find a capability that exists
    OwnershipDiscovery,    // agent could not answer "who owns this"
    RouteDiscovery,        // agent could not resolve the routing contract
    GovernanceAmbiguity,   // floors/authority unclear at decision time
    DuplicateReality,      // two sources claimed different truths
}
```

## Exact diff (receipt.rs)

```rust
// ── Attention (PROPOSAL: ATTENTION-COST-DOCTRINE, F13-gated) ─────────────

/// Leak taxonomy — why an attention event fired (ISOLATE primitive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LeakClass {
    CapabilityDiscovery,
    OwnershipDiscovery,
    RouteDiscovery,
    GovernanceAmbiguity,
    DuplicateReality,
}

/// Human-channel attention event (MEASURE primitive).
/// H/R/C/D are event counts, not durations — same unit as doctrine §2.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttentionEvent {
    pub h: u32,
    pub r: u32,
    pub c: u32,
    pub d: u32,
    /// Agent's up-front prediction of total H+R+C+D for this step.
    /// Gap vs actual = supervision signal (world-model training fuel).
    pub expected_turns: Option<u32>,
    pub leak_class: Option<LeakClass>,
}
```

In `FlowReceipt`, after `expected_outcome`:

```rust
    /// PROPOSAL (F13-gated): human attention event — H/R/C/D counts + leak class.
    /// Absent on receipts without human-channel contact (zero-cost default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attention: Option<AttentionEvent>,
```

## Why this shape

1. **`Option` + `skip_serializing_if`** — backward-compatible: the receipts chain
   (`/var/lib/arifflow/receipts.jsonl`) parses old lines unchanged; serde default
   covers untagged ingest posts. No migration.
2. **Counts, not durations** — matches doctrine §2 ("count H+R+C+D, turns vs
   expected"). Durations live in `cost_ns` already; do not duplicate state.
3. **`expected_turns` mirrors `expected_outcome`** — arifFlow already carries the
   T2-1 WHY bridge (`intent_reason` + `expected_outcome`); this extends the same
   prediction-discipline to the human channel. Symmetry is the review argument.
4. **arifFlow never interprets** (F3): the daemon stores and forwards the field;
   ACSC computation lives in AAA cockpit / analysis plane, NOT here.
5. **No new invariant** — this is F5 (receipt) doing what receipts do. Enforcement
   of "leak → learning atom" is SKILL_LEARNING_PROTOCOL v1.1 (separate proposal).

## Ingest surface (HTTP)

`POST /ingest` body gains optional `"attention": {...}` — passed through to the
receipt. No validation beyond type shape (arifFlow is not the judge of attention
claims; the chain + downstream analysis are).

## Test plan (pre-merge, on ratification)

- `cargo test` — new unit tests: (a) old JSONL line parses with `attention: None`,
  (b) round-trip AttentionEvent, (c) kebab-case leak_class wire form.
- `curl :7073/health` unchanged shape (no FQ formula change).
- Chain compatibility: ingest a receipt with `attention` then verify
  `previous_receipt_hash` chaining unaffected.

## If ratified

1. Merge this PR → `cargo build --release` → restart `arifflow.service` (T2).
2. SKILL_LEARNING_PROTOCOL v1.1 leak→atom bridge (AAA repo).
3. AAA cockpit APC trend reads `attention` fields (analysis, not enforcement).

If NOT ratified: close this PR unmerged, delete branch. No runtime was harmed.
