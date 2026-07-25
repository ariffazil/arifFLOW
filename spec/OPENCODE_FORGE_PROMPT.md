# OpenCode Forge Prompt — arifFlow Phase 2: Real Bridge

> **Mission:** Upgrade `/root/A-FORGE/domain/orchestration/arifFlow_adapter.py` from Python-native stubs to a real Rust ↔ Python bridge that spawns `/root/arifFlow` as a subprocess, calls live `arif_judge` MCP, and executes ACT phases via A-FORGE.
>
> **Sovereign:** Arif (F13)  
> **Constitution:** `/root/arifFlow/ARIFLOWKERNELCANON.md` (A1–A5)  
> **Cooling receipt:** `/root/arifFlow/COOLING_RECEIPT.md`  
> **Comparison table:** `/root/arifFlow/spec/AGI_SUBSTRATE_COMPARISON.md`
>
> **888-HOLD on production deploy** until FFI stability, verdict timeout, and crash recovery are proven.

---

## Files You Must Read First

| File | Why |
|---|---|
| `/root/arifFlow/ARIFLOWKERNELCANON.md` | 5 invariants. Break none. |
| `/root/arifFlow/src/lib.rs` | Rust crate re-exports |
| `/root/arifFlow/src/channel.rs` | Channel<T> — you read/write from here |
| `/root/arifFlow/src/scheduler.rs` | SuperStepScheduler — you call this |
| `/root/arifFlow/src/merkle.rs` | MerkleRoot, CheckpointEnvelope, chain_roots |
| `/root/arifFlow/src/bridge/arifos_governance.rs` | FFI stub — replace with real calls |
| `/root/arifFlow/src/bridge/aforge_executor.rs` | FFI stub — replace with real calls |
| `/root/arifFlow/src/topology/fan_out.rs` | FanOutTopology — merge + verify |
| `/root/arifFlow/src/topology/pipeline.rs` | PipelineConfig — stages |
| `/root/arifFlow/src/topology/cascade.rs` | CascadeConfig — handoff steps |
| `/root/arifFlow/src/governance/checkpoint.rs` | CheckpointManager — restore logic |
| `/root/arifFlow/src/governance/vault999.rs` | Vault999Sealer — seal per step |
| `/root/arifFlow/src/governance/kabarkan.rs` | KabarkanTracer — event emission |
| `/root/A-FORGE/domain/orchestration/arifFlow_adapter.py` | **THE FILE TO REWRITE** |

---

## What To Build

### 1. Python Bridge: `arifFlow_adapter.py` (REWRITE)

Replace the existing stub implementation with a real subprocess bridge.

**Architecture:**
```
arifFlow_adapter.py
├── spawn_ariflow() → subprocess.Popen
│   Rust binary compiled from /root/arifFlow/
│   stdin/stdout = JSON-RPC lines
│   stderr → Kabarkan
├── send_command(cmd: dict) → dict
│   JSON encode → stdin
│   JSON decode ← stdout
│   timeout: 30s per super-step
├── handle_verdict(verdict) → (verdict_id, class)
│   calls arif_judge(mode="intercept") via MCP
│   timeout → PENDING_VERDICT, retry 3x (500ms, 1s, 2s)
│   HOLD/VOID → stop scheduler, emit cooling receipt
│   SEAL → proceed
├── execute_nodes(node_list) → list[NodeResult]
│   maps each node to ACT 7-phase:
│     geox → act_phase(geox_mcp_tool)
│     wealth → act_phase(wealth_mcp_tool)
│     well → act_phase(well_mcp_tool)
│   returns receipt hashes
├── restore_from_checkpoint(path) → bool
│   load last envelope
│   call arif_judge(mode="validate")
│   if invalid → HOLD
│   if valid → rebuild state, resume step+1
└── mcp_ariflow_schedule() → MCP tool
```

**Message Protocol (stdin/stdout JSON-RPC):**

Python → Rust (command):
```json
{
  "cmd": "run_topology",
  "run_id": "run_<uuid>",
  "topology": "fan_out",
  "lease": {
    "lease_id": "lease_001",
    "actor_id": "arif",
    "constitutional_chain_id": "cc_abc",
    "scope": ["geox", "wealth", "well"]
  },
  "config": {
    "max_iterations": 10,
    "max_concurrency": 3,
    "merge_strategy": "ordered_concat"
  },
  "nodes": [
    {"id": "geox", "tool": "geox_basin", "params": {"mode": "profile", "name": "Malay"}},
    {"id": "wealth", "tool": "capital_health", "params": {"mode": "runway"}},
    {"id": "well", "tool": "well_assess_homeostasis", "params": {"mode": "fatigue"}}
  ]
}
```

Rust → Python (after each super-step):
```json
{
  "type": "checkpoint",
  "run_id": "run_abc",
  "step_index": 0,
  "state_root": "blake3hex...",
  "channel_roots": {"ch_geox": "h1", "ch_wealth": "h2", "ch_well": "h3"},
  "lease_id": "lease_001",
  "verdict": {"status": "PENDING", "chain_id": "cc_abc"}
}
```

Python → Rust (execution results):
```json
{
  "type": "node_results",
  "run_id": "run_abc",
  "step_index": 0,
  "results": [
    {"node_id": "geox", "status": "ok", "receipt_hash": "h1"},
    {"node_id": "wealth", "status": "ok", "receipt_hash": "h2"}
  ]
}
```

---

### 2. Add `mode="chain"` to `arif_forge`

In `/root/arifOS/arifosmcp/tools/forge.py` (or wherever `arif_forge` is dispatched):

```python
# New mode: arif_forge(mode="chain")
# Input: topology + nodes + config
# Output: SuperStepResult[]
# Calls arifFlow_adapter.mcp_ariflow_schedule()
```

This must go through the **existing 12-stage forge_preflight** — do NOT bypass it.

---

### 3. Verdict Timeout + Retry Policy

```python
VERDICT_POLICY = {
    "max_retries": 3,
    "backoff_ms": [500, 1000, 2000],
    "per_call_timeout_s": 10,
    "on_final_failure": "HOLD_LANE",
    "log_channel": "kabarkan"
}

def _call_judge_with_retry(state_root: bytes, lease_id: str) -> tuple[str, str]:
    for attempt, timeout in enumerate(VERDICT_POLICY["backoff_ms"]):
        try:
            resp = call_mcp("arif_judge", {
                "mode": "intercept",
                "actor": lease_id,
                "evidence": [{"state_root": state_root.hex()}]
            })
            return resp["verdict_id"], resp["verdict_class"]
        except TimeoutError:
            tracer.emit("VerdictTimeout", {"attempt": attempt, "lease_id": lease_id})
            sleep(timeout / 1000)
    tracer.emit("VerdictFailed", {"lease_id": lease_id})
    return ("timeout_fatal", "HOLD")
```

---

### 4. Crash Recovery Test Suite

Create `/root/arifFlow/tests/test_crash_recovery.py` (pytest):

```python
# Test 1: Rust core crash mid-run
#   → Kill arifFlow subprocess
#   → Verify last checkpoint exists
#   → Restore from checkpoint
#   → Verify authority chain intact
#   → Resume step+1

# Test 2: Python adapter crash
#   → Simulate adapter kill
#   → On restart, scan for orphan checkpoints
#   → Re-verify each via arif_judge(mode="validate")
#   → Resume or HOLD

# Test 3: Checkpoint with invalidated chain
#   → Load checkpoint whose chain was VOID post-hoc
#   → Verify restoration is REJECTED
#   → Verify HOLD emitted
```

---

## Critical Rules (DO NOT VIOLATE)

1. **A1 — Constitutional-First:** Never execute a super-step without a live lease from arifOS. If `arif_judge` is unreachable, HOLD — do not auto-proceed.

2. **A2 — Plane-Isolated:** Node results pass through content-hashed channels only. No shared memory between intelligence plane (LLM) and execution plane (scheduler + tools).

3. **A3 — Checkpoint-with-Verdict:** Every super-step writes an immutable checkpoint BEFORE the next step begins. Crash recovery must re-verify authority.

4. **A4 — Verifiable-Reduction:** Merge functions are deterministic. If `FanOutTopology.verify_merge()` fails → emit `DIVERGING` → HOLD.

5. **A5 — Metabolic-Closure:** Every run MUST end with a cooling receipt. No orphaned channels, no dangling leases.

6. **Do NOT bypass forge_preflight.** The 12-stage preflight in `/root/arifOS/arifosmcp/runtime/forge_preflight.py` (session validation → authority recompute → judge-state verification → vault check → dry-run → execute) is mandatory for any `arif_forge` call. The chain mode is an ADDITION, not a bypass.

7. **Do NOT call LLMs from within arifFlow.** arifFlow is a scheduler. It schedules nodes. It does not reason, generate, or infer. LLM calls go through arif_think → arif_judge → arif_forge like everything else.

---

## Success Criteria

| Criterion | How to Verify |
|---|---|
| Rust subprocess starts and accepts commands | `subprocess.Popen` with pipe I/O |
| Super-step cycles complete | 3-step fan-out returns 3 checkpoints |
| Verdict from live arif_judge | `curl :8088/health` shows judge calls |
| HOLD stops scheduler | Submit HOLD-worthy state → step count stops |
| Crash recovery restores correctly | Kill process → restart → verify resumption |
| Cooling receipt written | VAULT999 contains final envelope |
| All A1–A5 pass | Test suite green |

---

## Files You Will Create/Modify

| File | Action |
|---|---|
| `/root/A-FORGE/domain/orchestration/arifFlow_adapter.py` | **REWRITE** — real bridge |
| `/root/arifFlow/tests/test_crash_recovery.py` | **CREATE** — 3 recovery tests |
| `/root/arifOS/arifosmcp/tools/forge.py` | **MODIFY** — add `mode="chain"` |
| `/root/arifOS/arifosmcp/runtime/tools.py` | **MODIFY** — register chain mode |
| `/root/arifFlow/src/bridge/arifos_governance.rs` | **UPGRADE** — replace stubs with FFI |
| `/root/arifFlow/src/bridge/aforge_executor.rs` | **UPGRADE** — replace stubs with FFI |

---

## Do NOT Do

- ❌ Do not containerize arifFlow (Docker Doctrine: organs are bare-metal)
- ❌ Do not add new MCP tools (use mode="chain" on existing arif_forge)
- ❌ Do not skip forge_preflight for performance
- ❌ Do not auto-seal without 888_HOLD clearance
- ❌ Do not call LLMs from inside arifFlow

---

## Final Test

After all changes:

```bash
# Rust core still works
cd /root/arifFlow && cargo test

# Python adapter unit tests
cd /root/A-FORGE && python -m pytest domain/orchestration/test_arifFlow_adapter.py -v

# Integration: 3-node fan-out with live arif_judge
cd /root/A-FORGE && python -c "
from domain.orchestration.arifFlow_adapter import mcp_ariflow_schedule
result = mcp_ariflow_schedule(
    topology='fan_out',
    nodes=[{'id':'geox','tool':'geox_basin','params':{}}],
    actor_id='arif',
    max_iterations=3
)
print('PASS' if result['success'] else 'FAIL')
"

# Crash recovery test
pytest /root/arifFlow/tests/test_crash_recovery.py -v
```

---

> **DITEMPA BUKAN DIBERI — Forged, Not Given**
> **Kau dah forge spec. Sekarang hantar ke OpenCode.**
