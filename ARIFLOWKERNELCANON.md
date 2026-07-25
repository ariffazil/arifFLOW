# ARIFLOWKERNELCANON — arifFlow Mini-Constitution

> **DITEMPA BUKAN DIBERI** — Flow is forged, not given.
> **Trinity:** arifOS = law · arifFlow = flow · A-FORGE = hands
> **Parent constitution:** `/root/arifOS/GENESIS/000_KERNEL_CANON.md` (F1–F13)
> **Sovereign:** Arif (F13)

---

## Preamble

arifFlow is the **governed parallel execution engine** for the arifOS Federation. It replaces LangGraph's flat graph with constitutional BSP (Bulk Synchronous Parallel) — Pregel-style super-steps where every parallel transition is adjudicated by arifOS 888-JUDGE.

**arifFlow is NOT a governor.** It does not judge, does not seal, does not authorise. It schedules, channels, checkpoints, and records — always under the law of arifOS. The trinity is absolute:

```
arifOS (law) ──adjudicate──▶ arifFlow (flow) ──schedule──▶ A-FORGE (hands)
     ▲                                                          │
     └────────────────────── SEAL ──────────────────────────────┘
```

---

## Section 1: Core Invariants (A1–A5)

| ID | Name | Rule | Violation |
|----|------|------|-----------|
| **A1** | Constitutional-first | No parallel unit executes without a valid lease + 888-JUDGE scope. Every run bound to `actor_id` + `lease_id` from arifOS F13. | Engine HALT, breach report to VAULT999 |
| **A2** | Plane-isolated | Intelligence plane (LLM agents) and execution plane (arifFlow scheduler + A-FORGE tools) never share raw memory. State crosses planes only via signed, verifiable envelopes. | HOLD, cooling receipt emitted |
| **A3** | Checkpoint-with-verdict | Every super-step checkpoint records: state Merkle root, `actor_id`, `lease_id`, `verdict_id` (SEAL/HOLD/VOID/SABAR), `constitutional_chain_id`. Crash recovery MUST re-verify authority via arifOS before resuming. | Resume denied, full restart required |
| **A4** | Verifiable-reduction | Merge functions for fan-out and cascade are deterministic AND auditable by F3 TRI-WITNESS. Divergence → `DIVERGING` signal → 888-HOLD. | Merge rejected, branches preserved for audit |
| **A5** | Metabolic-closure | Every orchestration run ends with: cooling receipt to VAULT999, leases closed/renewed explicitly, no orphaned state or dangling channels. | Engine leak detected, breach seal |

---

## Section 2: Architecture

```
/root/arifFlow/
├── ARIFLOWKERNELCANON.md     ← this file
├── Cargo.toml                 ← Rust project
│
├── core/                      ← Rust runtime
│   ├── channel.rs             ← Channel<T> abstraction
│   ├── super_step.rs          ← Pregel-BSP scheduler
│   ├── merkle.rs              ← Merkle state hasher
│   └── topology.rs            ← Topology registry (3 fixed shapes)
│
├── topology/                  ← 3 governed topologies
│   ├── fan_out.rs             ← 1:N parallel, merge-witness
│   ├── pipeline.rs            ← Sequential stages with gates
│   └── cascade.rs             ← Multi-agent escalation chain
│
├── bridge/                    ← FFI to organs
│   ├── arifos_governance.rs   ← 888-JUDGE, leases, verdicts
│   ├── aforge_executor.rs     ← ACT 7-phase executor calls
│   └── kabarkan_trace.rs      ← Observability events
│
├── governance/                ← Flow-level governance
│   ├── checkpoint.rs          ← State + authority checkpointing
│   ├── vault_seal.rs          ← VAULT999 per-step sealing
│   └── receipt.rs             ← Cooling receipt format
│
└── tests/                     ← Deterministic test fixtures
    ├── fan_out_test.rs
    ├── pipeline_test.rs
    ├── cascade_test.rs
    └── checkpoint_invalidation_test.rs
```

### Plane Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    INTELLIGENCE PLANE                        │
│  (Hermes, OpenCode, GEOX, WEALTH, WELL — LLM agents)        │
│  State: ephemeral, per-agent                                 │
└────────────────────────┬────────────────────────────────────┘
                         │ signed envelopes (actor_id, lease_id, payload_hash)
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    GOVERNANCE PLANE                          │
│  (arifOS kernel :8088 — 888-JUDGE, F1–F13, VAULT999)        │
│  State: constitutional chain, verdicts, leases               │
└────────────────────────┬────────────────────────────────────┘
                         │ verdict (SEAL/HOLD/VOID/SABAR)
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    EXECUTION PLANE                           │
│  (arifFlow scheduler — channels, super-steps, checkpoints)   │
│  State: channel deltas, Merkle roots, checkpoint envelopes   │
└───────────────┬────────────────────────────────────────────┘
                │ tool invocation (via A-FORGE ACT executor)
                ▼
┌─────────────────────────────────────────────────────────────┐
│                    ACTUATOR PLANE                            │
│  (A-FORGE :7071 — 7-phase ACT, forge gate, tool shell)      │
│  State: execution results, receipts                          │
└─────────────────────────────────────────────────────────────┘
```

---

## Section 3: Super-Step Protocol

Every super-step follows this exact protocol:

```
  1. SCHEDULE:   Topology selects ready nodes based on channel state
  2. DISPATCH:   Each node receives its channel subscriptions + lease
  3. EXECUTE:    Nodes run in parallel (same super-step barrier)
  4. COLLECT:    Merge deltas from all nodes
  5. VERIFY:     Compute Merkle root of new state
  6. ADJUDICATE: Call arifOS 888-JUDGE:
                  → SEAL: commit deltas, advance super-step
                  → HOLD: discard deltas, preserve state for audit
                  → VOID: abort entire run, emit breach
  7. CHECKPOINT: Write checkpoint envelope to VAULT999
  8. COOL:       If final step, emit cooling receipt
```

**Key rule:** arifFlow never assumes SEAL. Every super-step waits for arifOS verdict before committing.

---

## Section 4: Channel Model

```rust
struct Channel<T> {
    id: ChannelId,
    current: Option<T>,           // committed value
    pending: Option<(T, LeaseId)>, // uncommitted delta from current super-step
    version: u64,                  // monotonic
    previous_hash: [u8; 32],       // Merkle chain
    subscribers: Vec<NodeId>,      // nodes that wake on this channel update
}
```

- Channels are append-only in the Merkle sense (previous_hash chains)
- A channel holds ONE current value and ONE pending delta per super-step
- Multiple nodes can subscribe to the same channel
- Nodes cannot write to a channel another node wrote to in the same super-step (determinism enforcement)
- Channel version bumps only on SEAL

---

## Section 5: Checkpoint Envelope

```rust
struct CheckpointEnvelope {
    // Identity
    actor_id: String,
    lease_id: Uuid,
    constitutional_chain_id: Uuid,
    
    // State
    super_step: u64,
    channel_roots: HashMap<ChannelId, [u8; 32]>,  // per-channel Merkle root
    state_root: [u8; 32],                          // overall Merkle root
    
    // Authority
    verdict_id: Option<Uuid>,
    verdict_class: VerdictClass,  // SEAL | HOLD | VOID | SABAR
    arifos_verdict_hash: [u8; 32],  // hash of the 888-JUDGE response
    
    // Timing
    timestamp_ns: i64,
    previous_checkpoint_hash: [u8; 32],
}
```

On crash resume: arifFlow sends `CheckpointEnvelope` to arifOS `validate_checkpoint()` tool. If the `constitutional_chain_id` has been voided by a post-hoc audit, the checkpoint is invalidated → full restart required.

---

## Section 6: Topology Contracts

### Fan-Out

```
Input channel ──▶ dispatch ──▶ [Node A, Node B, Node C] (parallel)
                                  │
                    merge_fn(deterministic, F3-auditable)
                                  │
                                  ▼
                            Output channel
```

- Merge function must be a pure function (no side effects, no I/O)
- Divergence detection: if any two nodes disagree on shared channel state → `DIVERGING` → 888-HOLD
- Contract: `fn fan_out<I: Send, O: Send>(nodes: Vec<Box<dyn FlowNode>>, input: I, merge: fn(Vec<O>) -> Result<O, Divergence>)`

### Pipeline

```
Stage 1 ──gate──▶ Stage 2 ──gate──▶ Stage 3 ──gate──▶ ...
```

- Each gate = call to arifOS 888-JUDGE
- If any gate returns HOLD, pipeline pauses, emits checkpoint, preserves intermediate state
- Contract: sequential with `for gate in gates { judge?; execute?; }`

### Cascade

```
Agent A ──HOLD?──▶ Agent B ──HOLD?──▶ Agent C ──HOLD?──▶ F13 Arif
```

- Escalation chain: each agent gets the full previous state
- Any agent can PROCEED (stop escalation, emit result)
- Only HOLD escalates to next agent
- Final F13 always reaches Arif (Telegram DM)
- Contract: recursive escalation with F13 fallback

---

## Section 7: Non-Goals (Hard Prohibitions)

| Prohibited | Why |
|------------|-----|
| General graph runtime | Too many paths = untestable governance. 3 fixed topologies only. |
| Mid-execution topology mutation | Would violate A2 (plane isolation). Topology is set at lease creation. |
| Bypass arifOS for "performance" | Every super-step must adjudicate. No caching of verdicts. |
| State mutation without Merkle hashing | Would break A3. Every delta is hashed. |
| Self-judging | arifFlow never emits its own verdict. Only arifOS judges. |

---

## Section 8: Relationship to Existing Organs

| Organ | arifFlow's relationship |
|-------|------------------------|
| **arifOS (:8088)** | Authority. Asks for 888-JUDGE, leases, checkpoint validation. Never bypasses. |
| **A-FORGE (:7071)** | Actuator. Calls ACT executor for tool invocation. All execution goes through A-FORGE's forge gate. |
| **VAULT999** | Truth. Writes per-step checkpoints and cooling receipts. Rejects invalid checkpoints. |
| **Kabarkan** | Observability. Emits super-step start/end, lease events, verdict changes, divergences. |
| **Hermes** | Intelligence. Can dispatch cascades through arifFlow. arifFlow schedules, Hermes thinks. |
| **GEOX / WEALTH / WELL** | Domain agents. Can be nodes in fan-out topologies. |

---

## Section 9: Breach Protocol

| Breach | Trigger | Response |
|--------|---------|----------|
| B1 — Channel conflict | Two nodes write same channel in one super-step | Super-step ABORT, lease revoked, breach seal |
| B2 — Verdict timeout | 888-JUDGE doesn't respond within timeout | HOLD, checkpoint saved, retry with backoff |
| B3 — Checkpoint invalid | Crash resume finds voided `constitutional_chain_id` | Resume DENIED, full restart, audit trigger |
| B4 — Divergence threshold | Merge function reports disagreement >0.3 | `DIVERGING` signal, 888-HOLD, manual resolution |

---

*DITEMPA BUKAN DIBERI — Forged, Not Given. This engine serves the constitution, never supersedes it.*
