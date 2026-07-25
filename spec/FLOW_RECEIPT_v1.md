# Flow Receipt v1 — Spec

> **The unit atom of governed flow.**
> Every hop, every execute, every verify, every cool — recorded.
>
> `v2026.07.25` · Part of arifFlow · FLOW layer of arifOS Federation

---

## 1. Purpose

A Flow Receipt is the **canonical record of one atomic step** in the arifOS
federation's execution. It is:

- **Immutability anchor** — each receipt chains to the previous via hash
- **Flow measurement instrument** — timestamps and costs allow real-time
  calculation of Flow Quotient (FQ)
- **Governance artifact** — carries floor verdicts, cooling decisions, and
  epistemic labels
- **Witness container** — aggregates tri-witness votes into one envelope
- **Merkle leaf** — batch-anchored into VAULT999 for civilizational truth

---

## 2. Receipt Chain

Every receipt links to the previous receipt in an execution trace:

```
[flow_start]
  |
  v
[receipt_1] ──previous_hash──→ [receipt_2] ──previous_hash──→ [receipt_3] ...
```

A **flow session** begins when `arif_init` emits the first receipt and ends
when `arif_seal` emits the final receipt. The chain is linear within one
session; multiple sessions produce parallel chains that merge only via
barrier or merge steps.

---

## 3. Fields

### 3.1 Identity

| Field | Type | Description |
|-------|------|-------------|
| `receipt_id` | UUID v4 | Globally unique receipt identifier |
| `previous_receipt_hash` | Option\<SHA3-256 hex\> | Hash of the previous receipt in this flow chain. `None` for the first receipt in a session. |
| `created_at` | DateTime\<Utc\> | Nanosecond-precision timestamp |

### 3.2 Actor

| Field | Type | Description |
|-------|------|-------------|
| `actor_id` | String | The agent or human who performed this step |
| `session_id` | String | Governing session (from `arif_init`) |
| `session_token` | Option\<String\> | SCT if governed by arifOS |

### 3.3 Flow Step

| Field | Type | Description |
|-------|------|-------------|
| `step_type` | StepType | What kind of step was this |
| `topology_id` | Option\<String\> | Which topology (fan-out/pipeline/cascade) |
| `lane_id` | Option\<u32\> | Which parallel lane within a topology |
| `step_number` | u64 | Monotonic step number within this session |

**StepType enum:**

| Variant | Meaning |
|---------|---------|
| `Execute` | Actual work — computation, forge, deploy |
| `Verify` | Verification, audit, floor check |
| `Cool` | Cooling queue action — hold, clamp, bypass |
| `Seal` | VAULT999 seal — irreversible commit |
| `Barrier` | Parallel barrier — wait for N lanes |
| `Merge` | Merge step — combine N lane outputs |
| `Route` | Routing — dispatch to another organ |

### 3.4 Cost

| Field | Type | Description |
|-------|------|-------------|
| `cost_ns` | u64 | Wall-clock duration of this step in nanoseconds |
| `preceding_verify_cost_ns` | Option\<u64\> | Total verification cost that led to this step (cumulative) |

The **Flow Quotient** is computed from these fields over a window:

```
FQ = Σ(Execute.cost_ns) / Σ(Verify.cost_ns + preceding_verify_cost_ns)
```

### 3.5 Epistemic

| Field | Type | Description |
|-------|------|-------------|
| `epistemic_label` | EpistemicLabel | Truth status of this step's output |

**EpistemicLabel enum:**

| Variant | Code | Meaning |
|---------|------|---------|
| `Observation` | OBS | Direct sensed reality |
| `Derivation` | DER | Logical deduction from evidence |
| `Interpretation` | INT | Inference under uncertainty |
| `Specification` | SPEC | Plan or intended action |
| `Seal` | SEAL | Irreversible commitment |

Mapping from arifOS epistemic tags (F2/F7 compliant).

### 3.6 Governance

| Field | Type | Description |
|-------|------|-------------|
| `floor_verdict` | FloorVerdict | F1–F13 verdict for this step |
| `cooling_decision` | CoolingDecision | Cooling queue action |

**FloorVerdict enum:**

| Variant | Meaning |
|---------|---------|
| `Pass` | All applicable floors satisfied |
| `Caution` | Soft floor tension (F5/F6) — proceed with awareness |
| `Hold` | Hard floor violation — 888_HOLD |
| `Void` | Critical violation — blocked permanently |

**CoolingDecision enum:**

| Variant | Meaning |
|---------|---------|
| `None` | No cooling needed |
| `Hold` | Cool down — pause execution |
| `Clamp` | Reduce intensity/speed |
| `Bypass` | Expedite — skip cooling queue |

### 3.7 Witness

| Field | Type | Description |
|-------|------|-------------|
| `tri_witness_votes` | Option\<TriWitnessVotes\> | Aggregated witness scores |

**TriWitnessVotes:**

| Field | Type | Range | Meaning |
|-------|------|-------|---------|
| `human` | f64 | 0.0–1.0 | Human witness confidence |
| `ai` | f64 | 0.0–1.0 | AI witness confidence |
| `earth` | f64 | 0.0–1.0 | Earth/data witness confidence |

### 3.8 Merkle

| Field | Type | Description |
|-------|------|-------------|
| `merkle_root` | Option\<String\> | Root hash of Merkle tree this receipt belongs to |
| `merkle_inclusion_proof` | Option\<String\> | Inclusion proof path (hex-encoded) |

Every 100 receipts (batch) are Merkle-anchored. The root is written to
VAULT999.

### 3.9 Payload

| Field | Type | Description |
|-------|------|-------------|
| `payload` | Option\<Value\> | Flexible JSON payload — step-specific data, errors, intermediates |

---

## 4. Flow Quotient (FQ)

The FQ is the primary metric for measuring whether an agent is **in flow**
or trapped in self-monitoring.

### 4.1 Computation

```
FQ = cost_of_execution_steps / cost_of_verification_steps
```

Where:
- **cost_of_execution_steps** = sum of `cost_ns` for all `StepType::Execute`
  in the window
- **cost_of_verification_steps** = sum of `cost_ns` for all `StepType::Verify`
  in the window + their `preceding_verify_cost_ns`

### 4.2 Thresholds

| FQ Range | Verdict | Meaning |
|----------|---------|---------|
| > 3.0 | `Optimal` | Agent in flow. Governance in the architecture. |
| 1.0 – 3.0 | `Balanced` | Healthy verification. Self-monitoring supports execution. |
| 0.5 – 1.0 | `Watching` | Agent spends as much time verifying as executing. Caution. |
| < 0.5 | `Stuck` | Self-monitoring has become the task. mPFC takeover. |

### 4.3 Window

FQ is computed over a sliding window of the last N receipts (default N=20).
This gives a real-time measure vs. a session-level average.

---

## 5. Serialization

Receipts are serialized as JSON for:
- Kabarkan span ingestion
- VAULT999 sealing
- Debug/audit inspection

Binary encoding (bincode) for internal channel passing.

---

## 6. Validation Rules

1. `receipt_id` must be unique within a session
2. `previous_receipt_hash` must match the SHA3-256 of the previous receipt
   (except for the first receipt)
3. `cost_ns` must be >= 0
4. `human` + `ai` + `earth` in TriWitnessVotes must each be 0.0–1.0
5. `merkle_root` and `merkle_inclusion_proof` must be present together or
   both absent
6. `created_at` must be monotonically increasing within a session

---

## 7. Receipt Chain Integrity

The chain is verified by recomputing `previous_receipt_hash` across all
receipts in a session:

```rust
fn verify_chain(receipts: &[FlowReceipt]) -> bool {
    for i in 1..receipts.len() {
        let expected = hash(&receipts[i - 1]);
        if receipts[i].previous_receipt_hash != Some(expected) {
            return false;
        }
    }
    true
}
```

Where `hash()` is SHA3-256 of the canonical JSON serialization of the
receipt.

---

## 8. Example

```json
{
  "receipt_id": "b6186ba2-7542-45fd-bb36-d41d8cd98f00",
  "previous_receipt_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "created_at": "2026-07-25T12:34:56.789012345Z",
  "actor_id": "a-forge-executor",
  "session_id": "b6186ba2-7542-45fd-aaaa",
  "session_token": null,
  "step_type": "Execute",
  "topology_id": "fanout:build-deploy",
  "lane_id": 2,
  "step_number": 47,
  "cost_ns": 3420000000,
  "preceding_verify_cost_ns": 150000000,
  "epistemic_label": "OBS",
  "floor_verdict": "Pass",
  "cooling_decision": "None",
  "tri_witness_votes": {
    "human": 0.95,
    "ai": 0.88,
    "earth": 1.0
  },
  "merkle_root": null,
  "merkle_inclusion_proof": null,
  "payload": {
    "action": "deploy:forge_work/2026-07-25/receipt.rs",
    "target": "A-FORGE mcp",
    "result": "deployed"
  }
}
```

---

## 9. Related Specs

- Kabarkan span protocol (observability ingestion)
- VAULT999 sealing protocol (Merkle batch anchoring)
- Cooling queue spec (hold/clamp/bypass decisions)
- arifOS epistemic tags (F2/F7 compliance)
