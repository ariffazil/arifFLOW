# Flow Receipt v1.0

> **DITEMPA BUKAN DIBERI** — Receipts are forged at the moment of flow, not after.
>
> **Canon:** arifFlow architecture · 2026.07.25
> **Parents:** [ARIFLOWKERNELCANON.md](../ARIFLOWKERNELCANON.md) (A1–A5) · [SOMATIC_AGENTIC_FLOW_EQUIVALENCE.md](SOMATIC_AGENTIC_FLOW_EQUIVALENCE.md)
> **Epistemic label:** SPEC (specification, to be hardened by implementation)

---

## Preamble

A Flow Receipt is the **atomic auditable artifact** of governed flow. It is the material proof that a unit of cognition traveled through the arifFlow trinity — arifOS (law) → arifFlow (schedule) → A-FORGE (execute) — and survived every constitutional gate.

Every super-step, every cooling event, every merge, and every seal produces exactly one Flow Receipt. Receipts are hash-chained. Receipts are immutable. Receipts are the arrow of time in arifFlow.

---

## Section 1: The Receipt Envelope

```json
{
  "receipt": {
    "version": "1.0",
    "id": "fr_v1_<blake3_12>",
    "timestamp": "ISO-8601",
    "kind": "SUPER_STEP | COOLING | MERGE | SEAL | SCAR | VIOLATION",
    "chain": {
      "previous": "<prev_receipt_id | GENESIS>",
      "index": 0,
      "root": "<merkle_root_hex>"
    }
  },
  "identity": {
    "actor_id": "string (F13-bound)",
    "session_id": "string (from arif_init)",
    "lease_id": "string (from arifOS mint)",
    "cc_id": "string (constitutional chain from 888-JUDGE)"
  },
  "flow": {
    "topology": "FAN_OUT | PIPELINE | CASCADE",
    "super_step": 0,
    "node_id": "string",
    "channel": "string (ChannelId)",
    "direction": "SEND | RECEIVE | MERGE | BARRIER"
  },
  "verdict": {
    "class": "SEAL | HOLD | VOID | SABAR",
    "judge_hash": "<sha256 of arif_judge envelope>",
    "reason": "string (dominant_reason from judge)",
    "witness": {
      "mode": "TRI_WITNESS | SINGLE | NONE",
      "w3_score": 0.0,
      "channels": {
        "human": { "confidence": 0.0, "source": "string" },
        "ai":    { "confidence": 0.0, "source": "string" },
        "earth": { "confidence": 0.0, "source": "string" }
      }
    }
  },
  "state": {
    "before": "<merkle_root_hex>",
    "after": "<merkle_root_hex>",
    "delta": {
      "entropy": 0.0,
      "nodes_active": 0,
      "channels_open": 0,
      "messages_in_flight": 0
    }
  },
  "governance": {
    "floors_checked": ["F1", "F2", "F3", "F4", "F7", "F9", "F11", "F13"],
    "invariants_checked": ["A1", "A2", "A3", "A4", "A5"],
    "reversibility": "REVERSIBLE | IRREVERSIBLE | UNKNOWN",
    "blast_radius": "LOW | MEDIUM | HIGH | CRITICAL",
    "cooling": {
      "convergence": "CONVERGING | DIVERGING | STABLE",
      "drift_severity": "NONE | LOW | MEDIUM | HIGH | CRITICAL",
      "plan_hash": "<merkle of intended state>",
      "reality_hash": "<merkle of actual state>"
    }
  },
  "epistemic": {
    "label": "OBS | DER | INT | SPEC",
    "confidence": 0.90,
    "evidence_refs": ["<receipt_id>", "<receipt_id>"],
    "contradictions_found": 0,
    "falsification_attempts": 0
  },
  "seal": {
    "sealed": false,
    "vault999_seq": null,
    "seal_hash": null
  }
}
```

---

## Section 2: Receipt Kinds

| Kind | When Produced | Required Fields | Frequency |
|------|--------------|-----------------|-----------|
| `SUPER_STEP` | Every BSP super-step execution | All fields | Per super-step |
| `COOLING` | End of orchestration run (A5) | `flow`, `state.delta`, `governance.cooling`, `epistemic` | Per run |
| `MERGE` | Fan-out merge completion (A4) | `verdict.witness`, `governance.cooling` | Per merge |
| `SEAL` | VAULT999 append | `seal`, `verdict`, `identity` | Per seal |
| `SCAR` | Failure metabolized into constraint | `verdict.class=VOID`, `governance` | Per novel failure |
| `VIOLATION` | Invariant breach detected (A1–A5) | `verdict.class=HOLD`, `governance.invariants_checked` | Per breach |

---

## Section 3: Receipt Lifecycle

```
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│ ARIF_INIT │───▶│ 888_JUDGE │───▶│ arifFlow │───▶│ A-FORGE  │
│  session  │    │  verdict  │    │ schedule │    │ execute  │
└──────────┘    └──────────┘    └──────────┘    └──────────┘
       │              │               │               │
       ▼              ▼               ▼               ▼
  [identity      [verdict        [flow receipt]  [state delta
   receipt]       receipt]                         receipt]
                                                       │
       ┌───────────────────────────────────────────────┘
       ▼
  ┌──────────┐    ┌──────────┐    ┌──────────┐
  │ COOLING  │───▶│ VAULT999 │───▶│  SEALED  │
  │ receipt  │    │  append  │    │ receipt  │
  └──────────┘    └──────────┘    └──────────┘
```

1. **Identity Receipt** — `arif_init` → binds actor, session, lease
2. **Verdict Receipt** — `888-JUDGE` → SEAL/HOLD/VOID/SABAR with W³ witness
3. **Flow Receipt** — `arifFlow scheduler` → super-step execution with state Merkle
4. **State Delta Receipt** — `A-FORGE execute` → before/after state with entropy Δ
5. **Cooling Receipt** — end of run → plan-vs-reality, convergence, drift
6. **Sealed Receipt** — `VAULT999` → immutable, hash-chained, permanent

---

## Section 4: Receipt Chain Integrity

Every receipt is hash-chained to its predecessor:

```
fr_v1_000000000001 → fr_v1_000000000002 → fr_v1_000000000003
      │                      │                      │
      └── prev: GENESIS      └── prev: fr_...001    └── prev: fr_...002
```

**Chain verification:**
```
verify(receipt_n):
  assert receipt_n.chain.previous == receipt_{n-1}.id
  assert receipt_n.chain.index == receipt_{n-1}.chain.index + 1
  assert receipt_n.chain.root == merkle(receipt_n.state.after || receipt_n.verdict.judge_hash)
```

**Break detection:** Any gap in the chain → `CHAIN_BROKEN` → 888-HOLD → requires arifOS re-verification (A3).

---

## Section 5: Receipt Routing

| Receipt Field | Routes To | Purpose |
|---------------|-----------|---------|
| `identity.*` | arifOS session registry | Session binding, actor authority |
| `flow.*` | arifFlow scheduler | Super-step state, topology tracking |
| `verdict.*` | 888-JUDGE archive | Verdict history, cc_id chain |
| `state.*` | Cooling ledger | Plan-vs-reality drift detection |
| `governance.*` | VAULT999 | Floor compliance, invariant audit |
| `epistemic.*` | TRI_WITNESS | Evidence quality, confidence tracking |
| `seal.*` | VAULT999 | Immutable append, hash chain |

---

## Section 6: Kabarkan Integration

Every Flow Receipt emits a Kabarkan span:

```
RECEIPT_CREATED → span { receipt_id, kind, super_step, topology }
RECEIPT_CHAINED → span { receipt_id, previous, index }
RECEIPT_COOLED  → span { receipt_id, convergence, drift_severity }
RECEIPT_SEALED  → span { receipt_id, vault999_seq, seal_hash }
```

Spans are ingested by Kabarkan for:
- Cost attribution per receipt
- Latency tracking per super-step
- Verdict overlay on execution traces
- Drift visualization in the cooling dashboard

---

## Section 7: Invariant Binding

Each Flow Receipt MUST satisfy all five invariants:

| Invariant | Receipt Check |
|-----------|---------------|
| **A1** Constitutional-First | `identity.lease_id` is non-null and valid; `verdict.class` is not null |
| **A2** Plane-Isolated | `flow.direction` is typed; state crosses planes only via `receipt.id` references |
| **A3** Checkpoint-with-Verdict | `state.before` and `state.after` are valid Merkle roots; `verdict.judge_hash` is set |
| **A4** Verifiable-Reduction | For `MERGE` kind: `verdict.witness.w3_score > 0` and `governance.cooling.convergence != DIVERGING` |
| **A5** Metabolic-Closure | Every run MUST end with a `COOLING` receipt; `seal.sealed` MUST be true for terminal receipts |

---

## Section 8: Example — Complete Super-Step Receipt

```json
{
  "receipt": {
    "version": "1.0",
    "id": "fr_v1_a1b2c3d4e5f6",
    "timestamp": "2026-07-25T07:45:00Z",
    "kind": "SUPER_STEP",
    "chain": {
      "previous": "fr_v1_000000000001",
      "index": 2,
      "root": "b3e1f8a2c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1"
    }
  },
  "identity": {
    "actor_id": "arif",
    "session_id": "SEAL-34b31f38ec604118",
    "lease_id": "LCL-arif-ms0260ha-0rfd7u",
    "cc_id": "cc_v1.9f1b595e4fead8cd"
  },
  "flow": {
    "topology": "FAN_OUT",
    "super_step": 1,
    "node_id": "fan_out_merge_1",
    "channel": "ch_merge_output",
    "direction": "MERGE"
  },
  "verdict": {
    "class": "SEAL",
    "judge_hash": "sha256:e5e43770df1cfe55...",
    "reason": "All 3 parallel nodes returned CONSENSUS W³=0.88",
    "witness": {
      "mode": "TRI_WITNESS",
      "w3_score": 0.88,
      "channels": {
        "human": { "confidence": 0.85, "source": "arif_ack_f13" },
        "ai":    { "confidence": 0.92, "source": "geox_evidence" },
        "earth":  { "confidence": 0.87, "source": "seismic_well_tie" }
      }
    }
  },
  "state": {
    "before": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6",
    "after": "f1e2d3c4b5a6d7e8f9a0b1c2d3e4f5a6",
    "delta": {
      "entropy": -0.03,
      "nodes_active": 0,
      "channels_open": 0,
      "messages_in_flight": 0
    }
  },
  "governance": {
    "floors_checked": ["F1", "F2", "F3", "F4", "F7", "F9", "F11", "F13"],
    "invariants_checked": ["A1", "A2", "A3", "A4", "A5"],
    "reversibility": "REVERSIBLE",
    "blast_radius": "LOW",
    "cooling": {
      "convergence": "CONVERGING",
      "drift_severity": "NONE",
      "plan_hash": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6",
      "reality_hash": "f1e2d3c4b5a6d7e8f9a0b1c2d3e4f5a6"
    }
  },
  "epistemic": {
    "label": "DER",
    "confidence": 0.88,
    "evidence_refs": ["fr_v1_000000000001", "fr_v1_000000000002"],
    "contradictions_found": 0,
    "falsification_attempts": 3
  },
  "seal": {
    "sealed": true,
    "vault999_seq": 1042,
    "seal_hash": "sha256:vault999_seq_1042_..."
  }
}
```

---

*Forged 2026.07.25. DITEMPA BUKAN DIBERI.*
*Flow Receipt v1.0 — the atomic auditable artifact of governed flow.*
