# arifFlow — Unified Spec v1

> **Status:** Phase 2 in forge · **888-HOLD** on production deploy  
> **Repo:** `/root/arifFlow/` (Rust) + `A-FORGE/domain/orchestration/arifFlow_adapter.py` (Python)  
> **Constitution:** `ARIFLOWKERNELCANON.md` (A1–A5 invariants)  
> **Sovereign:** Arif (F13)  
> **Trinity:** arifOS = law · arifFlow = flow · A-FORGE = hands

---

## 1. Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    INTELLIGENCE PLANE                        │
│  (Hermes, OpenCode, GEOX, WEALTH, WELL)                     │
│  Sends topology definitions via JSON-L stdin                 │
└────────────────────────┬────────────────────────────────────┘
                         │ {topology, lanes, config}
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    arifFlow Adapter (Python)                  │
│  /root/A-FORGE/domain/orchestration/arifFlow_adapter.py      │
│                                                              │
│  • Spawns Rust binary as subprocess                          │
│  • Pipes topology via stdin (JSON-L)                         │
│  • Receives checkpoint envelopes via stdout (JSON-L)         │
│  • Calls arifOS 888-JUDGE per super-step                     │
│  • Writes VAULT999 micro-seals per step                      │
│  • Emits Kabarkan trace events                               │
│  • Handles verdict timeout + retry with backoff              │
│  • Crash recovery: restore checkpoint, re-verify authority   │
└───────────────────────┬─────────────────────────────────────┘
                        │ channel: "step" + "verdict"
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                    arifFlow Core (Rust)                       │
│  /root/arifFlow/target/release/arifflow (867K binary)        │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  Channel<T>  │  │  SuperStep   │  │   Merkle     │      │
│  │  Hashed msg  │◄─┤  Scheduler   │──┤   Hasher     │      │
│  │  Bounded/Unb │  │  BSP Pregel  │  │  Per-channel │      │
│  └──────────────┘  └──────┬───────┘  └──────────────┘      │
│                           │                                 │
│              ┌────────────┼────────────┐                    │
│              ▼            ▼            ▼                    │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐        │
│  │  Fan-Out     │ │  Pipeline    │ │  Cascade      │        │
│  │  Parallel +  │ │  Sequential  │ │  Escalation   │        │
│  │  Merge       │ │  Stages      │ │  Chain        │        │
│  └──────────────┘ └──────────────┘ └──────────────┘        │
│                                                              │
│  24 tests passing · 0 warnings · Release build               │
└──────────────────────────────────────────────────────────────┘
```

## 2. Existing Components (Do Not Rewrite)

### 2.1 Rust Core — `/root/arifFlow/src/`

| File | Lines | What it does | Tests |
|------|-------|-------------|-------|
| `lib.rs` | 53 | Crate root, invariants doc, re-exports | 1 |
| `channel.rs` | 228 | `Channel<T>` — content-hashed message passing, bounded/unbounded, Merkle root per channel | 4 |
| `merkle.rs` | 270 | `MerkleTree` — per-channel roots, authority binding (`bind_authority`), `chain_roots` | 8 |
| `scheduler.rs` | 460 | `SuperStepScheduler` — Pregel-BSP, `step()` returns checkpoint, `commit_verdict()` applies SEAL/HOLD/VOID | 5 |
| `topology/mod.rs` | 35 | Shared `TopologyError`, `NodeResult` types | — |
| `topology/fan_out.rs` | 169 | `FanOutTopology` — parallel dispatch + `merge_results()` (OrderedConcat, MerkleRoot) + `verify_merge()` | 4 |
| `topology/pipeline.rs` | 65 | `PipelineTopology` — sequential stages with review loop | — |
| `topology/cascade.rs` | 62 | `CascadeTopology` — multi-agent handoff | — |
| `bridge/mod.rs` | 1 | Module declaration | — |
| `bridge/arifos_governance.rs` | 210 | `ArifOSGovernanceBridge` — FFI stubs for lease, verdict, validate | — |
| `bridge/aforge_executor.rs` | 55 | `AForgeExecutor` — FFI stub for ACT execution | — |
| `governance/mod.rs` | 1 | Module declaration | — |
| `governance/checkpoint.rs` | 160 | `CheckpointManager` — write/restore/verify with chain validation | 3 |
| `governance/vault999.rs` | 45 | `Vault999Sealer` — per-step sealing, hash chain | — |
| `governance/kabarkan.rs` | 75 | `KabarkanTracer` — event emission (SuperStepStarted, DivergentMerge, Cooling) | — |
| `main.rs` | 270 | Binary entry — reads JSON-L stdin, routes to scheduler, writes stdout | — |

### 2.2 Python Adapter — `/root/A-FORGE/domain/orchestration/arifFlow_adapter.py`

| Component | Lines | What it does |
|-----------|-------|-------------|
| `ArifFlowAdapter` class | ~200 | Spawns Rust binary, manages stdin/stdout protocol |
| `spawn(topology)` | — | Starts Rust, configures topology + lease |
| `seed_channel(ch, data)` | — | Seeds initial data |
| `run_step(nodes)` | — | Full super-step cycle: dispatch → need_verdict → arif_judge → verdict → commit |
| `close()` | — | Graceful shutdown, cooling receipt |
| `restore_from_checkpoint()` | — | Crash recovery: re-verify authority, replay |
| `_call_arif_judge()` | — | HTTP POST to arifOS 888-JUDGE with Accept header, retry + backoff |
| `_call_validate_checkpoint()` | — | Validate checkpoint against constitutional chain |
| `_handle_divergence()` | — | A4 divergence logging + Kabarkan emit |
| `_emit_kabarkan()` | — | Observable event emission |
| `_write_vault999_micro_seal()` | — | Per-step envelope to VAULT999 |

## 3. Features to Extend (Not Rewrite)

### 3.1 Barrier Timeout Policy — Rust `src/scheduler.rs`

**Current state:** Super-step barrier is implicit — all nodes must complete. No timeout.

**Extend with:**
```rust
struct BarrierConfig {
    mode: BarrierMode,     // ALL | MAJORITY | N_OF_M
    timeout_ms: u64,
    policy: TimeoutPolicy, // HOLD_ALL | CONTINUE_MAJORITY | CANCEL_ALL
}
```
- Add timeout check inside `step()`
- On timeout: check policy, execute action
- Emit Kabarkan `barrier_timeout` span
- Write VAULT999 `BARRIER_TIMEOUT` envelope

### 3.2 Lane Cooling Queue — Rust + Python

**Rust `scheduler.rs`:**
- Add `LaneState::Cooling` variant
- Add `cooling_queue: Vec<ChannelId>`
- Cooling lanes skip barrier, rejoin next super-step

**Python adapter:**
- Propagate cooling state in envelopes
- Write cooling ledger entry

### 3.3 TRI_WITNESS Merge — Rust `src/topology/fan_out.rs`

**Current state:** `MergeStrategy::OrderedConcat` and `MerkleRoot`.

**Extend with:**
```rust
fn merge_results_tri_witness(
    results: &[NodeResult],
    witness: &WitnessScores,
) -> MergeResult
```
- Calculate divergence score
- Apply TRI_WITNESS formula: `(human + ai + earth) / 3`
- Threshold: 0.75 for SEAL
- Divergence > 0.6 → HOLD

### 3.4 F1 Per-Lane Reversibility — Python adapter

**Current state:** Adapter checks lease existence (A1). No per-lane reversibility.

**Extend `run_step()`:**
```python
for node in nodes:
    if node.get("irreversible") and not node.get("verdict"):
        # F1 violation — block lane
        self._emit_kabarkan("f1_blocked", {"lane": node["id"]})
        self._write_vault999_breach({"lane": node["id"]})
        node["status"] = "HELD"
```

### 3.5 forge_parallel Wrapper — TypeScript (new file)

**New file:** `A-FORGE/src/interfaces/mcp/parallelTools.ts`

**Purpose:** Wrap existing `forge_parallel` to call `arifFlow_adapter` instead of raw A2A delegation.

```typescript
class BSPExecutionPlan {
    plan_id: string;
    super_steps: SuperStep[];
    lease_id: string;
    ccId: string;
    
    async execute(adapter: ArifFlowAdapter): Promise<CoolingReceipt>;
    async holdLane(lane_id: string, reason: string): Promise<void>;
}
```

## 4. Data Flow (One Super-Step)

```
Adapter main() reads from sys.stdin
    │ line = json.loads([node definitions])
    ▼
run_step(nodes)
    │
    ├─ _send({"type": "step", "nodes": nodes})  ──▶ Rust stdin
    │
    │  Rust reads "step" message
    │  ↓
    │  sched.step(&nodes) → runs nodes, produces checkpoint
    │  ↓
    │  sends {"type": "need_verdict", state_root, lease_id, chain_id}
    │
    ├─ _recv() ◀── Rust stdout: need_verdict
    │
    ├─ _call_arif_judge(state_root, lease_id, chain_id)
    │   │
    │   ├─ POST /mcp {method: "tools/call", params: {name: "arif_judge"}}
    │   │                                      headers: {"Accept": "application/json"}
    │   ├─ Retry 3x with backoff [1s, 2s, 4s]
    │   └─ Returns VerdictResult {verdict, verdict_id, hash}
    │
    ├─ _send({"type": "verdict", "class": verdict, "verdict_id": vid})  ──▶ Rust stdin
    │
    │  Rust reads "verdict" message
    │  ↓
    │  sched.commit_verdict(class) → SEAL/HOLD/VOID
    │  ↓
    │  sends {"type": "step_result", step, verdict, state_root, deltas}
    │
    ├─ _recv() ◀── Rust stdout: step_result
    │
    ├─ if SEAL: write VAULT999 micro-seal
    ├─ emit Kabarkan event
    │
    └─ Return {"step": N, "verdict": "SEAL|HOLD", "state_root": "hex"}
```

## 5. Governance Enforcement Matrix

| Invariant | Rust | Python Adapter | Test |
|-----------|------|---------------|------|
| A1 — Lease required | `step()` returns error on nil lease | `spawn()` generates lease_id before Rust starts | ✅ `test_no_lease_returns_error` |
| A1 — 888-JUDGE per step | `step()` returns `need_verdict` | `_call_arif_judge()` called before `commit_verdict()` | ✅ adapter integration |
| A2 — Plane isolation | Channels use content-hashed `Message<T>` | Envelopes are JSON-L on pipes | ✅ `test_channel_hash_mismatch_detected` |
| A3 — Checkpoint envelope | `CheckpointEnvelope` in scheduler | Written to `VAULT999/flow_step_N.json` | ✅ `test_checkpoint_write_restore` |
| A3 — Crash recovery | `restore_from_checkpoint()` in main.rs | `_call_validate_checkpoint()` via arifOS | ⏳ Phase 2 |
| A4 — Deterministic merge | `merge_results()` pure function | `verify_merge()` checks against claimed output | ✅ `test_fanout_merge_verify` |
| A4 — Divergence detection | N/A (merge is sync in Rust) | `_handle_divergence()` → HOLD | ✅ `test_fanout_divergent_merge_detected` |
| A5 — Cooling receipt | `cooling` message sent on stop | `close()` returns `CoolingReceipt` | ✅ adapter integration |
| A5 — Channel closure | `Channel::close()` on VOID | Adapter `close()` sends stop → Rust closes | ✅ `test_closed_channel_rejects_writes` |
| Verdict timeout | N/A | 3 retries with backoff → safe HOLD fallback | ⏳ Phase 2 |
| F1 per-lane | ⏳ To extend | ⏳ To extend | ⏳ |

## 6. File Map

```
/root/
├── arifFlow/                          ← NEW REPO
│   ├── ARIFLOWKERNELCANON.md          ← Mini-constitution (A1-A5)
│   ├── AGI_SUBSTRATE_COMPARISON.md    ← 7-system comparison
│   ├── COOLING_RECEIPT.md             ← Phase 1 cooling receipt
│   ├── Cargo.toml                     ← Rust project (deps: serde, blake3, uuid)
│   ├── adapter/
│   │   └── arifFlow_adapter_spec.md   ← Adapter implementation spec
│   └── src/
│       ├── lib.rs                     ← Crate root + re-exports
│       ├── main.rs                    ← Binary entry (stdin/stdout JSON-L)
│       ├── channel.rs                 ← Channel<T> — content-hashed
│       ├── merkle.rs                  ← MerkleTree — state hasher
│       ├── scheduler.rs               ← SuperStepScheduler — BSP engine
│       ├── topology/
│       │   ├── mod.rs                 ← TopologyError, NodeResult
│       │   ├── fan_out.rs             ← Parallel + merge + divergence
│       │   ├── pipeline.rs            ← Sequential stages
│       │   └── cascade.rs             ← Escalation chain
│       ├── bridge/
│       │   ├── mod.rs
│       │   ├── arifos_governance.rs   ← FFI: lease, verdict, validate
│       │   └── aforge_executor.rs     ← FFI: execution request
│       └── governance/
│           ├── mod.rs
│           ├── checkpoint.rs          ← CheckpointManager
│           ├── vault999.rs            ← Vault999Sealer
│           └── kabarkan.rs            ← KabarkanTracer
│
├── A-FORGE/
│   └── domain/orchestration/
│       └── arifFlow_adapter.py        ← Python adapter (ALREADY FORGED)
│
└── forge_work/2026-07-25-arifflow-bsp-spec/
    └── G1_BSP_SCHEDULER_SPEC.md       ← AAA governance layer spec
```

## 7. 888-HOLD Conditions

| # | Condition | Test | Status |
|---|-----------|------|--------|
| 1 | FFI ke `arif_judge` stabil (100 calls, 0 failures) | Run adapter loop calling arif_judge 100x | ⏳ |
| 2 | Verdict timeout jelas (<15s) | Kill arifOS → adapter HOLDs within 15s | ⏳ |
| 3 | Crash recovery selamat | Kill Rust mid-run → restore checkpoint → re-verify authority | ⏳ |

All three must pass before production deploy.

---

*DITEMPA BUKAN DIBERI — This spec captures T₀ state. Re-probe at T₁.*
