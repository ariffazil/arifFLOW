# ARIFLOW_KERNEL_CANON.md

> **SOT:** 2026-08-02 | **Authority:** ARIF / F13 SOVEREIGN  
> **Layer:** PLANE 3 — FLOW | **Status:** SEALED — INVARIANT ENFORCEMENT LIVE  
> **Sibling:** arifOS CANON (PLANE 1 — LAW)
> **Enforcement:** `/root/arifFlow/src/governance/invariants.rs` — 92 tests, daemon :7073

---

## Identity

arifFLOW is the Federated Intelligence Flow Layer — the "sistem saraf" (nervous system) of the arifOS Federation. It sits between LAW and HANDS:

```
arifOS (law/Python) ──adjudicate──▶ arifFLOW (flow/Rust) ──schedule──▶ A-FORGE (hands/TypeScript)
```

It does not judge. It does not seal. It does not authorize. It schedules, channels, checkpoints, observes, and records — always under arifOS constitutional authority.

---

## Plane Position

```
PLANE 0 — HUMAN         ARIFFAZIL (sovereign intent)
PLANE 1 — LAW           arifOS (constitutional judgment)
PLANE 2 — THINK         ATLAS333 (cognitive geometry)
PLANE 3 — FLOW          arifFLOW (nervous movement) ← THIS PLANE
PLANE 4 — HANDS         A-FORGE (governed execution)
PLANE 5 — REALITY       GEOX · WEALTH · WELL · HERMES
PLANE 6 — MEMORY        VAULT999 (immutable witness)
```

---

## Flow-Plane Invariants (F0-F6)

These are the constitutional laws of movement. They define what arifFLOW **is permitted to be** — distinct from the A-series execution invariants that define **how lanes run**.

| ID | Name | Rule |
|----|------|------|
| **F0** | Flow transmits, never owns. | arifFLOW transmits governed intelligence. It does not originate intent and does not claim ownership of what it routes. |
| **F1** | Flow schedules, never authorizes. | arifFLOW determines execution order. Authorization comes from arifOS (PLANE 1). Scheduling ≠ permission. |
| **F2** | Flow checkpoints, never judges. | arifFLOW records Merkle-anchored state at every super-step. Verdict grammar (SEAL/HOLD/SABAR/VOID) belongs to arifOS. |
| **F3** | Flow observes, never interprets. | arifFLOW measures FQ, detects drift, emits cooling receipts, and reports divergence. What drift means belongs to ATLAS333/arifOS. |
| **F4** | Flow routes execution, never becomes execution authority. | arifFLOW dispatches lanes to A-FORGE. A-FORGE owns the execution decision within its governed scope. |
| **F5** | Flow writes receipts, never owns memory. | arifFLOW appends checkpoint receipts to VAULT999. VAULT999 sovereignty belongs to ARIFFAZIL/arifOS. |
| **F6** | Flow connects organs, never collapses organs. | arifFLOW schedules GEOX, WEALTH, WELL, HERMES. It does not merge them, does not own them, does not understand their domain realities. |

**F6 is the boundary that prevents the nervous system from becoming a mind.**

---

## Execution Invariants (A1-A6)

These govern how individual execution lanes operate within arifFLOW's scheduling framework:

| ID | Name | Rule |
|----|------|------|
| **A1** | Constitutional-First | No parallel unit executes without lease + 888-JUDGE verdict |
| **A2** | Plane-Isolated | State crosses planes only via signed, verifiable envelopes |
| **A3** | Checkpoint-with-Verdict | Every super-step: Merkle root + verdict logged to VAULT999 |
| **A4** | Verifiable-Reduction | Merge functions are deterministic + TRI_WITNESS auditable |
| **A5** | Metabolic-Closure | Every run ends: cooling receipt, leases closed, no orphans |
| **A6** | Flow Observes, Never Interprets | Same as F6 — enforced at both flow-plane and execution levels |

---

## What arifFLOW Is NOT

| arifFLOW is NOT | Because |
|----------------|---------|
| A judge | Judgment belongs to arifOS (PLANE 1) |
| A mind | Thinking belongs to ATLAS333 (PLANE 2) |
| An executor | Execution belongs to A-FORGE (PLANE 4) |
| A domain organ | Reality reading belongs to GEOX/WEALTH/WELL/HERMES (PLANE 5) |
| An owner of memory | VAULT999 authority belongs to ARIFFAZIL/arifOS (PLANE 0/1) |
| An interpreter of meaning | Interpretation belongs to ATLAS333/arifOS |

---

## Relationship to Other Planes

```
PLANE 0 → arifFLOW receives intent (via arifOS routing)
PLANE 1 → arifFLOW requests verdicts (via arifOS MCP)
PLANE 2 → arifFLOW receives plans (from ATLAS333)
PLANE 4 → arifFLOW dispatches execution (to A-FORGE)
PLANE 5 → arifFLOW schedules organs (GEOX, WEALTH, WELL, HERMES)
PLANE 6 → arifFLOW writes receipts (to VAULT999)
```

arifFLOW writes checkpoint receipts to VAULT999. It does not own VAULT999. Sovereignty over memory belongs to PLANE 0 (ARIFFAZIL) and PLANE 1 (arifOS).

---

## Three Governed Topologies

Only three fixed topologies — no general graph. "Too many paths = untestable governance."

1. **Fan-Out** — 1:N parallel, merged at barrier with deterministic merge function
2. **Pipeline** — sequential stages, each gated by 888-JUDGE
3. **Cascade** — escalation chain, only HOLD escalates, final stop is F13 Arif

---

## Flow Quotient (FQ)

```
FQ = Σ(cost_execute) / Σ(cost_verify + cost_preceding_verify)
```

| FQ Range | Verdict | Meaning |
|----------|---------|---------|
| > 3.0 | OPTIMAL | Agent in flow. Governance lives in architecture. |
| 1.0–3.0 | BALANCED | Healthy. Normal operating range. |
| 0.5–1.0 | WATCHING | Self-monitoring competes with execution. |
| < 0.5 | STUCK | mPFC takeover. Ruminating. HOLD semua. |
| < 0.25 | PARALYZED | Hard intervention needed. |

**The hard constraint: Bila FQ < 0.5, semua HOLD. Bila FQ naik, semua forge.**

---

## Zen

```
ARIF menentukan.
arifOS menghukum.
ATLAS333 berfikir.
arifFLOW mengalirkan.
A-FORGE melaksanakan.
Organs membaca realiti.
VAULT999 menyimpan saksi.
```

Seven verbs. Seven owners. Zero overlap. This is not metaphor — it is the execution path of every governed action.

---

## Invariant Enforcement (2026-08-02)

The F0-F6 invariants are now **automatically enforced** by the arifFlow daemon.

### Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/health` | GET | Status + FQ + invariant health + restricted actors |
| `/ingest` | POST | Ingest a flow receipt, update actor state |
| `/check` | POST | Check if actor is allowed to execute (invariant gate) |
| `/release` | POST | Release hold on actor (after verification receipt) |
| `/enforce` | POST | Manually trigger enforcement cycle |

### Enforcement Rules

| Rule | Threshold | Action |
|------|-----------|--------|
| FQ < 0.5 (STUCK) | verify_cost dominates execute_cost | **HOLD** — block execution |
| FQ > 10.0 (OVERHEAT) | execute far outruns verify | **THROTTLE** — cooldown 30s |
| Consecutive executes > 5 | no verify between 5+ executes | **HOLD** — mandate verification |
| F0/F2/F4/F5/F6 | structural invariants | **PASS** — enforced by architecture |

### Invariant Flow

```
Actor executes → POST /ingest (Execute receipt)
                    ↓
              InvariantEnforcer.ingest() → update FQ
                    ↓
              POST /enforce (or auto-cycle)
                    ↓
              Check F0-F6 → HOLD / THROTTLE / PASS
                    ↓
              Before next execute: POST /check → allowed?
                    ↓
              If HOLD: send Verify receipt → POST /release
                    ↓
              Actor released → resume execution
```

### Source

`/root/arifFlow/src/governance/invariants.rs` — 92 tests, 0 failures.

---

**DITEMPA BUKAN DIBERI — Forged, Not Given.**  
**F0-F6 ratified. A1-A6 ratified. Invariant enforcement LIVE. Architecture SEALED. Mutation HOLD.**
