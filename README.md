# arifFlow — FLOW Layer

> **Governed Parallel Execution Engine for the arifOS Federation**
>
> DITEMPA BUKAN DIBERI — Forged, Not Given.

---

## Trinity

```
arifOS       → LAW      → Python     → constitution, judge, 888
arifFlow     → FLOW     → Rust       → orchestration, parallelism, Merkle
A‑FORGE      → HANDS    → TypeScript → execution, MCP, deploy, forge
```

arifFlow sits **between** law and hands. It is the engine that drives governed parallel execution — BSP scheduler, channel-based topology orchestration, Merkle receipt anchoring, and VAULT999 envelope sealing.

---

## Architecture

| Layer | Language | Role | Port |
|-------|----------|------|------|
| **arifOS** | Python | Constitutional judge, F1–F13, cc_id issuance | 8088 |
| **arifFlow** | Rust | BSP scheduler, topology executor, Merkle hash chain, cooling queue | (Unix socket / HTTP bridge) |
| **A‑FORGE** | TypeScript | Execution forge, MCP tools, orchestrators | 7071 |

### Topologies

- **Fan-out** — N workers in parallel, reduce at barrier
- **Pipeline** — Sequential stage-by-stage with channel pass-through
- **Cascade** — Tree fan-out with conditional pruning

### 5 Invariants (A1–A5)

1. **A1** — Per-lane reversibility (F1-compliant)
2. **A2** — Barrier timeout < configurable max
3. **A3** — Crash recovery: kill-9 → state restore
4. **A4** — Merkle anchor every receipt
5. **A5** — No cross-lane mutation outside barriers

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

- `/root/A-FORGE/domain/orchestration/arifFlow_adapter.py`
- `POST /bridge/execute` — submit a topology
- `GET /bridge/status/:id` — poll execution state
- `POST /bridge/cooling` — cooling queue enqueue

Every organ (GEOX, WEALTH, WELL, AAA, Hermes) may call arifFlow directly through its bridge interface. No organ imports another to reach the flow engine.

---

## Versioning

Date-stamped: `vYYYY.MM.DD` (Iron Rule — never semver).

---

## Release Cycle

1. `cargo build --release`
2. `cargo test`
3. Bump Cargo.toml version → `vYYYY.MM.DD`
4. `git tag vYYYY.MM.DD && git push --tags`
5. `systemctl restart arifflow` (on deploy)

---

## License

AGPL-3.0 — arifOS Federation standard. See `LICENSE`.
