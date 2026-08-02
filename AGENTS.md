<!-- CANONICAL: /root/AGENTS.md -->
<!-- Status: DERIVED — organ-specific extension -->

<!-- SOT-MANIFEST
federation_release: v2026.08.02
last_verified: 2026-08-02T07:55Z
live_commit: 17d649b
owner_summary: GREEN — invariant enforcement LIVE
truth_rule: cargo test + curl :7073/health beat any prose below
-->

# AGENTS.md — arifFlow | arifOS Federation

> **DITEMPA BUKAN DIBERI**
> **Φ flow engine — schedules, checkpoints, channels, never judges.**
> **Trinity:** `arifOS (law/Python) · arifFlow (flow/Rust) · A-FORGE (hands/TypeScript)`

## Identity

Governed parallel execution engine — constitutional BSP (Bulk Synchronous Parallel) scheduler.
Replaces LangGraph's role under arifOS constitutional law.

**Authority chain:** `arif_judge 888` → `arifFlow schedule` → `A-FORGE execute` → `arif_seal 999`

**3 topologies:** Fan-out (1→N parallel), Pipeline (sequential stages), Cascade (escalation chain)

## Invariants

### Flow-Plane (F0-F6) — AUTOMATED ENFORCEMENT LIVE

| ID | Name | Rule | Enforcement |
|----|------|------|-------------|
| F0 | Transmit, Never Own | Flow transmits, never originates intent | Architecture |
| F1 | Schedule, Never Authorize | Scheduling ≠ permission | Architecture |
| F2 | Checkpoint, Never Judge | Verdict grammar belongs to arifOS | Architecture |
| F3 | Observe, Never Interpret | FQ measurement, drift detection | **AUTO** — FQ gate |
| F4 | Route, Never Execute | Execution belongs to A-FORGE | Architecture |
| F5 | Receipt, Never Own Memory | VAULT999 authority: ARIFFAZIL/arifOS | Architecture |
| F6 | Connect, Never Collapse | Organs are independent | Architecture |

### Execution (A1-A6) — enforced in scheduler

| ID | Name | Rule |
|----|------|------|
| A1 | Constitutional-First | No execution without valid lease + 888_JUDGE |
| A2 | Plane-Isolated | Intelligence ↔ Execution planes: signed envelopes only |
| A3 | Checkpoint-with-Verdict | Every super-step: Merkle root + verdict logged |
| A4 | Verifiable-Reduction | Merge functions are deterministic + auditable |
| A5 | Metabolic-Closure | Every run ends: cooling receipt, leases closed |
| A6 | Flow Observes, Never Interprets | Same as F6 — enforced at both planes |

## Invariant Enforcement (FQ Gate)

The daemon (:7073) automatically enforces F0-F6:

| Trigger | Threshold | Action |
|---------|-----------|--------|
| FQ < 0.5 (STUCK) | verify dominates execute | **HOLD** — block execution |
| FQ > 10.0 (OVERHEAT) | execute far outruns verify | **THROTTLE** (30s cooldown) |
| >5 consecutive executes | no verify between | **HOLD** — mandate verification |

### Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/health` | GET | Status + FQ + invariant health + restricted actors |
| `/ingest` | POST | Ingest flow receipt, update actor FQ |
| `/check` | POST | **Invariant gate** — check if actor can execute |
| `/release` | POST | Release hold on actor after verification |
| `/enforce` | POST | Trigger enforcement cycle manually |

### Agent Execution Flow (ALL agents MUST follow)

```
Before execute:
    POST /check {"actor_id": "333-AGI"}
    → 200 allowed=true  → proceed
    → 403 allowed=false → HOLD — send Verify receipt, then release

After execute:
    POST /ingest {step_type: "Execute", ...}

After verify:
    POST /ingest {step_type: "Verify", ...}
    POST /release {"actor_id": "333-AGI"}
```

### Client Libraries

**Shell:**
```bash
source /root/arifFlow/scripts/check.sh
arifflow_check "333-AGI"    # returns 0 if allowed, 1 if blocked
arifflow_release "333-AGI"  # release hold
```

**Python:**
```python
from arifflow.client import check, ingest, release

result = check("333-AGI")
if not result.allowed:
    raise SystemExit(f"HOLD: {result.reason}")

# ... execute ...

ingest("333-AGI", session_id, "Execute", "Observation", cost_ns)
```

## Build & Test

```bash
cargo build --release
cargo test                    # 92/92 tests (2026-08-02)
cargo clippy                  # lint
cargo fmt                     # format
```

## Protocol

STDIN/STDOUT JSON-L protocol — arifOS sends commands, arifFlow streams verdict requests and results:

1. `configure {topology, lease_id, actor_id, chain_id}` → setup
2. `seed {channel, data}` → inject seed data
3. `step {nodes}` → run super-step → emits `need_verdict`
4. `verdict {class: SEAL|HOLD|VOID|SABAR, ...}` → commit verdict
5. `stop` → emits `cooling {total_steps, final_root, leases_closed}`

## Federation Pointers

| What | Where |
|------|-------|
| arifOS kernel | `/root/arifOS/` — port 8088 |
| A-FORGE executor | `/root/A-FORGE/` — port 7071 |
| arifFlow daemon | systemd `arifflow.service` — port 7073 |
| Invariant enforcement | `/root/arifFlow/src/governance/invariants.rs` |
| Client (Python) | `/root/arifFlow/src/py/arifflow/client.py` |
| Client (Shell) | `/root/arifFlow/scripts/check.sh` |
| Canon | `/root/arifFlow/ARIFLOW_KERNEL_CANON.md` |
| Federation landing | `/root/AAA/CLAUDE.md` |

## Versioning

**Iron Rule — date-stamped only.** Tags: `vYYYY.MM.DD`. No semver.

**Current:** v2026.08.02 — invariant enforcement LIVE. 92/92 tests.