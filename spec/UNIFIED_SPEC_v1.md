# arifFlow Unified Spec v1

> **Status:** LIVE · **Forged:** 2026-07-25  
> **Constitution:** A1–A5 (`/root/arifFlow/ARIFLOWKERNELCANON.md`)  
> **Rust core:** 24 tests ✅ · **Python adapter:** Phase 1 live, Phase 2 forging  
> **Authority:** arifOS F1–F13 · **Hands:** A-FORGE · **Flow:** arifFlow
>
> **DITEMPA BUKAN DIBERI — Forged, Not Given**

---

## Purpose

Single source of truth that maps:

1. **G1 BSP Scheduler Spec** (AAA group, TypeScript)
2. **arifFlow Rust Core** (Phase 1, already compiled)
3. **Python Adapter** (existing + Phase 2 extensions)
4. **A-FORGE integration** (live services)

No rewrite. No duplicate. One scheduler, one merge engine, one governance surface.

---

## Architecture (4-Layer Stack)

```
┌──────────────────────────────────────────────────────────┐
│  AAA Governance Layer (TypeScript wrappers)              │
│  BSPExecutionPlan · SuperStep · Lane · Barrier · Envelope│
│  Thin wrappers — call Rust backend via Python adapter    │
├──────────────────────────────────────────────────────────┤
│  Python Adapter (arifFlow_adapter.py)                    │
│  Spawn Rust · stdin/stdout JSON · call arif_judge        │
│  F1 per-lane · cooling queue · Kabarkan · VAULT999        │
├──────────────────────────────────────────────────────────┤
│  Rust Core (scheduler.rs, channel.rs, merkle.rs, ...)    │
│  BSP execution · Merkle hasher · barrier · merge · vault │
│  No LLM calls · No host access · Only schedule + record  │
├──────────────────────────────────────────────────────────┤
│  Federation Organs (GEOX · WEALTH · WELL · A-FORGE)      │
│  Executed via ACT phases — scheduled by arifFlow         │
└──────────────────────────────────────────────────────────┘
```

---

## Data Model Mapping

| G1 BSP Spec (TypeScript) | arifFlow Rust (existing) | Status | Extend? |
|---|---|---|---|
| `BSPExecutionPlan` | `SuperStepScheduler` (`scheduler.rs`) | ✅ Implemented | Add `plan_status` field |
| `SuperStep` | `SuperStep` struct (`scheduler.rs:53`) | ✅ Implemented | Add `barrier_config` field |
| `Lane` | `Channel<T>` + `FlowNode` trait | ✅ Equivalent | Add `lane_state::Cooling` |
| `BarrierConfig` | `FanOutTopology` (`fan_out.rs`) | ⏳ Partial | Add timeout policy + modes |
| `MergeResult` | `merge_results()` + `verify_merge()` | ✅ Implemented | Add `TRI_WITNESS` strategy |
| `Envelope` | `CheckpointEnvelope` (`scheduler.rs:54`) | ✅ Implemented | Add `barrier_timeout` type |
| `WitnessResult` | Not in Rust core | ❌ Missing | New struct in `fan_out.rs` |

---

## State Machine Mapping

| G1 State | Rust Equivalent | Status |
|---|---|---|
| `PLANNING` | `TopologyConfig` + `begin_run()` | ✅ |
| `DISPATCHING` | `register_channel()` + `seed_channel()` | ✅ |
| `EXECUTING` | `step()` with `FlowNode` dispatch | ✅ |
| `BARRIER` | `read_all()` → `channel_roots` collection | ✅ — add timeout here |
| `MERGING` | `merge_results()` + `verify_merge()` | ✅ — add TRI_WITNESS |
| `SEALING` | `CheckpointEnvelope` → VAULT999 | ✅ |
| `COMPLETE` | `end_run()` → cooling receipt | ✅ |

---

## Integration Point Mapping

| Existing Component | Hook | Current Status | Phase 2 Action |
|---|---|---|---|
| `forge_parallel` | `forgeParallelViaBSP()` | A2A spawn | Wrap → `arifFlow_adapter.schedule()` |
| `DAG executor` | `dagLevelToSuperStep()` | Sequential | Each depth → 1 super-step |
| `PipelineCoordinator` | `pipelinePhase()` | Sequential | Parallel phases → BSP |
| `ConvergenceEngine` | TRI_WITNESS evaluator | Standalone | Call from `merge_results()` |
| `FloorEnforcer` | F1 per-lane | Global only | Per-lane before dispatch |
| `Kabarkan` | New span types | Sequential trace | `super_step`, `lane_spawn`, `barrier`, `merge` |
| `VAULT999` | New receipt types | Final seal only | `SUPER_STEP`, `MERGE_WITNESS`, `LANE_BREACH` |
| `Cooling ledger` | Per-lane cooling | Not exist | New ledger entries |

---

## Phase 2 Extensions (What OpenCode Will Forge)

### 1. Barrier Timeout — `src/scheduler.rs`

```rust
pub struct BarrierConfig {
    pub mode: BarrierMode,        // ALL | MAJORITY | N_OF_M
    pub n_of_m_count: Option<u32>,
    pub timeout_ms: u64,
    pub on_timeout: TimeoutPolicy, // HOLD_ALL | CONTINUE_MAJORITY | CANCEL_ALL
}

enum TimeoutPolicy {
    HoldAll,          // Hold semua lane, escalate
    ContinueMajority, // Proceed dengan yang complete
    CancelAll,        // Cancel semua, mark FAILED
}
```

### 2. Lane Cooling — `src/scheduler.rs`

```rust
enum LaneState {
    Pending,
    Executing,
    Cooling(u64),    // cooling_remaining_ms
    Hold888,
    Complete,
}
```

### 3. TRI_WITNESS Merge — `src/topology/fan_out.rs`

```rust
pub struct WitnessResult {
    pub human_score: f32,
    pub ai_score: f32,
    pub earth_score: f32,
    pub combined: f32,
    pub threshold: f32,   // default 0.75
    pub passed: bool,
}

pub struct MergeResult {
    pub merged_value: Vec<u8>,
    pub divergence_score: f32,
    pub witness: WitnessResult,
    pub verdict: VerdictClass,
}
```

### 4. F1 Per-Lane — `arifFlow_adapter.py`

```python
def _check_lane_reversibility(lane: dict) -> bool:
    """Call FloorEnforcer(F1) before dispatch.
    If irreversible && no verdict → block lane.
    """
    if lane.get("reversibility") == "IRREVERSIBLE":
        if not lane.get("verdict_id"):
            tracer.emit("LaneBreach", {"lane_id": lane["id"]})
            vault999.write("LANE_BREACH", lane)
            return False
    return True
```

### 5. TypeScript Wrappers (New)

```
A-FORGE/src/interfaces/mcp/bsptypes/
├── BSPExecutionPlan.ts
├── SuperStep.ts
├── Lane.ts
├── BarrierConfig.ts
├── Envelope.ts
└── index.ts
```

These are NOT executors. They are governance surfaces around the Rust backend.

---

## Test Suite (50+ total)

| Domain | Rust | Python | TypeScript |
|---|---|---|---|
| Barrier timeout modes | ✅ New | — | — |
| Lane cooling transitions | ✅ New | ✅ New | — |
| TRI_WITNESS merge divergence | ✅ New | — | — |
| Reversible rollback | ✅ Existing | — | — |
| Irreversible breach seal | ✅ Existing | ✅ New | — |
| Checkpoint restore | ✅ Existing | ✅ New | — |
| F1 per-lane enforcement | — | ✅ New | — |
| Cooling queue propagation | — | ✅ New | — |
| arif_judge roundtrip | — | ✅ New | — |
| BSPExecutionPlan construction | — | — | ✅ New |
| forge_parallel integration | — | — | ✅ New |

---

## File Tree (Final State After Phase 2)

```
/root/arifFlow/
├── ARIFLOWKERNELCANON.md
├── Cargo.toml
├── COOLING_RECEIPT.md
├── src/
│   ├── lib.rs
│   ├── channel.rs          ← EXISTING
│   ├── merkle.rs           ← EXISTING
│   ├── scheduler.rs        ← EXTEND (barrier timeout, lane cooling)
│   ├── topology/
│   │   ├── mod.rs
│   │   ├── fan_out.rs      ← EXTEND (TRI_WITNESS)
│   │   ├── pipeline.rs     ← EXISTING
│   │   └── cascade.rs      ← EXISTING
│   ├── bridge/
│   │   ├── mod.rs
│   │   ├── arifos_governance.rs  ← EXISTING (stub → real in Phase 2.1)
│   │   └── aforge_executor.rs    ← EXISTING (stub)
│   └── governance/
│       ├── mod.rs
│       ├── checkpoint.rs   ← EXISTING
│       ├── vault999.rs     ← EXISTING
│       └── kabarkan.rs     ← EXISTING
├── spec/
│   ├── AGI_SUBSTRATE_COMPARISON.md
│   ├── OPENCODE_FORGE_PROMPT.md
│   └── UNIFIED_SPEC_v1.md  ← THIS FILE
└── tests/
    └── test_crash_recovery.py  ← NEW (Phase 2)

/root/A-FORGE/domain/orchestration/
└── arifFlow_adapter.py     ← EXTEND (F1 per-lane, cooling)

/root/A-FORGE/src/interfaces/mcp/bsptypes/
├── BSPExecutionPlan.ts      ← NEW
├── SuperStep.ts             ← NEW
├── Lane.ts                  ← NEW
├── BarrierConfig.ts         ← NEW
├── Envelope.ts              ← NEW
└── index.ts                 ← NEW
```

---

## OpenCode Instructions (Summary)

```
1. READ all existing Rust files (DO NOT REWRITE)
2. ADD barrier timeout to scheduler.rs
3. ADD lane cooling to scheduler.rs
4. ADD TRI_WITNESS to fan_out.rs
5. ADD F1 per-lane to arifFlow_adapter.py
6. CREATE TypeScript wrappers in A-FORGE/src/interfaces/mcp/bsptypes/
7. ADD tests for all new features
8. RUN cargo test (must stay green)
```

---

> **DITEMPA BUKAN DIBERI — Forged, Not Given**  
> **Law: arifOS · Flow: arifFlow · Hands: A-FORGE**  
> **Unified Spec v1 — 2026-07-25**
