# ⚡ arifFlow — Governed Parallel Execution Engine

> **DITEMPA BUKAN DIBERI** — Flow is forged, not given.
>
> Trinity: `arifOS (law) · arifFlow (flow) · A-FORGE (hands)`

arifFlow is the **constitutional BSP (Bulk Synchronous Parallel) engine** for the arifOS Federation. It replaces flat-graph orchestration with Pregel-style super-steps — every parallel transition adjudicated by arifOS 888-JUDGE.

**arifFlow is NOT a governor.** It does not judge, seal, or authorise. It schedules, channels, checkpoints, and records — always under the law of arifOS.

---

## Architecture

```
arifOS (law/Python) ──adjudicate──▶ arifFlow (flow/Rust) ──schedule──▶ A-FORGE (hands/TypeScript)
     ▲                                                                      │
     └────────────────────────── SEAL ──────────────────────────────────────┘
```

### Plane Separation

```
INTELLIGENCE PLANE          EXECUTION PLANE
(Hermes, OpenCode,          (arifFlow scheduler
 GEOX, WEALTH, WELL)         + A-FORGE tools)
       │                           │
       └── signed envelopes ───────┘
          (actor_id, lease_id, payload_hash)
```

### 3 Governed Topologies

| Topology | Shape | Use Case |
|----------|-------|----------|
| **Fan-out** | 1→N parallel, merge-witness | Multi-agent reasoning, parallel evidence gathering |
| **Pipeline** | Sequential stages with gates | CI/CD, deploy pipelines, staged computation |
| **Cascade** | Multi-agent escalation chain | Incident response, governance escalation |

---

## Core Invariants (A1–A5)

| ID | Invariant | Rule |
|----|-----------|------|
| A1 | Constitutional-first | No parallel unit runs without valid lease + 888-JUDGE scope |
| A2 | Plane-isolated | Intelligence plane and execution plane never share raw memory |
| A3 | Checkpoint-with-verdict | Every super-step checkpoint records Merkle root + verdict |
| A4 | Verifiable-reduction | Merge functions are deterministic and F3 TRI-WITNESS auditable |
| A5 | Metabolic-closure | Every run ends with: cooling receipt, leases closed, no orphans |

---

## Quick Start

```bash
# Build
cargo build --release

# Run tests
cargo test

# Run engine (requires arifOS kernel on :8088)
cargo run -- --lease-id <lease_id> --topology fan_out
```

### Dependencies

- **arifOS** (port 8088) — governance, judge, leases, verdicts
- **A-FORGE** (port 7071) — execution of scheduled tasks
- **Kabarkan** — observability span ingestion

---

## Repository Layout

```
/root/arifFlow/
├── ARIFLOWKERNELCANON.md     ← mini-constitution (binding)
├── src/
│   ├── main.rs               ← engine entry point
│   ├── lib.rs                ← library root
│   ├── channel.rs            ← Channel<T> abstraction
│   ├── scheduler.rs          ← BSP super-step scheduler
│   ├── merkle.rs             ← Merkle state hasher
│   ├── topology/             ← 3 governed topologies
│   │   ├── fan_out.rs
│   │   ├── pipeline.rs
│   │   └── cascade.rs
│   ├── bridge/               ← FFI to organs
│   │   ├── arifos_governance.rs
│   │   ├── aforge_executor.rs
│   │   └── kabarkan.rs
│   └── governance/           ← Flow-level governance
│       ├── checkpoint.rs
│       ├── cooling.rs
│       ├── tri_witness.rs
│       └── vault999.rs
└── tests/                    ← Deterministic test fixtures
```

---

## Versioning

Date-stamped only — Iron Rule. No semver.

Tags: `vYYYY.MM.DD`

---

## License

AGPL-3.0

---

*Forged 2026-07-25. DITEMPA BUKAN DIBERI.*
*Trinity: arifOS = law · arifFlow = flow · A-FORGE = hands*
