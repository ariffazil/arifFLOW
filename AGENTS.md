<!-- SOT-MANIFEST
federation_release: v2026.07.25
last_verified: 2026-07-25T00:00Z
live_commit: PENDING_FIRST_TAG
owner_summary: GREEN (pre-alpha)
truth_rule: cargo test beats any prose below
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

**5 invariants (A1–A5):**
| ID | Name | Rule |
|----|------|------|
| A1 | Constitutional-First | No execution without valid lease + 888_JUDGE |
| A2 | Plane-Isolated | Intelligence ↔ Execution planes: signed envelopes only |
| A3 | Checkpoint-with-Verdict | Every super-step: Merkle root + verdict logged |
| A4 | Verifiable-Reduction | Merge functions are deterministic + auditable |
| A5 | Metabolic-Closure | Every run ends: cooling receipt, leases closed, no orphans |

## Build & Test

```bash
cargo build --release
cargo test                    # 44/44 tests (Phase 4)
cargo clippy                  # lint
cargo fmt                     # format
./scripts/bench.sh            # barrier timeout benchmarks
```

## Protocol

STDIN/STDOUT JSON-L protocol — arifOS sends commands, arifFlow streams verdict requests and results:

1. `configure {topology, lease_id, actor_id, chain_id}` → setup
2. `seed {channel, data}` → inject seed data
3. `step {nodes}` → run super-step → emits `need_verdict`
4. `verdict {class: SEAL|HOLD|VOID|SABAR, ...}` → commit verdict
5. `stop` → emits `cooling {total_steps, final_root, leases_closed}`

## Live Endpoint

Not yet deployed. Planned systemd unit: `ariflow.service`.

## Federation Pointers

| What | Where |
|------|-------|
| arifOS kernel | `/root/arifOS/` — port 8088 |
| A-FORGE executor | `/root/A-FORGE/` — port 7071 |
| A-FORGE adapter | `/root/A-FORGE/domain/orchestration/arifFlow_adapter.py` |
| Spec | `/root/arifFlow/spec/EUREKA_PLAYBOOK_v1.md` |
| Spec | `/root/arifFlow/spec/UNIFIED_SPEC_v1.md` |
| Federation landing | `/root/AAA/CLAUDE.md` |

## Versioning

**Iron Rule — date-stamped only.** Tags: `vYYYY.MM.DD`. No semver.
