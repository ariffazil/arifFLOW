# arifFlow — METABOLISM & FLOW Layer

> **Governed Parallel Execution & Attention Metabolism Engine for the arifOS Federation**
>
> DITEMPA BUKAN DIBERI — Forged, Not Given.

**SOT:** 2026-08-02 | **Version:** `v2026.08.02` | **License:** AGPL-3.0
**live_commit:** 5c53232 | **last_verified:** 2026-08-02T06:19:13Z

---

## The 5-Organ Federation Substrate

| Organ | Role | Substrate | Key Function |
|---|---|---|---|
| **arifOS** | **LAW** | Python (`:8088`) | Constitutional judge, F1–F13, 888 verdicts |
| **AAA** | **MIND** | TypeScript (`:3001`) | Cockpit, A2A mesh, session routing |
| **arifFlow** | **METABOLISM** | Rust (Native Daemon) | Attention Metabolism (U = E × M × D), BSP scheduler, Merkle receipts |
| **A-FORGE** | **HANDS** | TypeScript (`:7071`) | Execution forge, MCP tools, deployments |
| **VAULT999** | **MEMORY** | Immutable Ledger | Merkle envelope sealing & audit persistence |

arifFlow sits between mind and hands. It is the engine that drives governed parallel execution and **Attention Metabolism** — ensuring truth is not just delivered, but *metabolized, ordered, and decoded* without cognitive overexposure.

---

## The 13 Constitutional Axioms (A1–A13)

Every agent booting in the federation loads `EUREKA_PLAYBOOK v1.1` and executes under these 13 axioms:

### Execution & Topology (A1–A10)

| Axiom | Rule |
|-------|------|
| **A1** | Per-lane reversibility (F1-compliant) |
| **A2** | Barrier timeout < configurable max |
| **A3** | Crash recovery: kill-9 → state restore |
| **A4** | Merkle anchor every receipt |
| **A5** | No cross-lane mutation outside barriers |
| **A6–A10** | Governed Fan-out, Pipeline, Cascade topologies |

### Attention Metabolism & Decoding (A11–A13)

| Axiom | Rule |
|-------|------|
| **A11** | **Attention Metabolism (EMD):** U = E × M × D. Filters entropy before rendering. |
| **A12** | **Attention Ordering:** Strict priority ordering — "what should appear now?" not "where should this go?" |
| **A13** | **Decoder Architecture:** One runtime, multiple decoders. Adapts output to target observer. |

---

## Hard Metabolic Anti-Patterns

1. ❌ **Surfacing all truths** — Overexposure = Failure. Raw un-metabolized data breaks F4 (Clarity).
2. ❌ **Treating arifFLOW as transport** — The metabolic spine is an active attention governor, not a passive pipe.

---

## Topologies Supported

- **Fan-out:** N workers in parallel with barrier reduction
- **Pipeline:** Sequential stage-by-stage with channel pass-through
- **Cascade:** Tree fan-out with conditional branch pruning

---

## Quick Start

```bash
cargo build --release
cargo test
./target/release/arifflow --help
```

### Prerequisites
- Rust 2024 edition (1.85+)
- Tokio runtime

---

## Federation Contract

arifFlow is a **sovereign organ** — called by other organs via adapter:

- **Adapter:** `/root/A-FORGE/domain/orchestration/arifFlow_adapter.py`
- `POST /bridge/execute` — submit execution topology
- `GET /bridge/status/:id` — poll execution state
- `POST /bridge/cooling` — cooling queue enqueue

Every organ (GEOX, WEALTH, WELL, AAA, Hermes) may call arifFlow directly through its bridge interface. No organ imports another to reach the flow engine.

---

## Versioning & Release Cycle

**Iron Rule:** Date-stamped `vYYYY.MM.DD` (never semver).

```bash
cargo build --release
cargo test
cargo test                    # 44/44 tests
# Bump Cargo.toml version → vYYYY.MM.DD
git tag vYYYY.MM.DD && git push --tags
systemctl restart arifflow    # on deploy
```

---

*AGPL-3.0 License · arifOS Federation Standard · Sovereign: Arif Fazil*
