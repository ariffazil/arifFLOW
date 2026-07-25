# Flow Receipt v1.0

> **DITEMPA BUKAN DIBERI** — Receipts are forged at the moment of flow, not after.
>
> **Canon:** arifFlow architecture · 2026.07.25
> **Parents:** [ARIFLOWKERNELCANON.md](../ARIFLOWKERNELCANON.md) (A1–A5) · [SOMATIC_AGENTIC_FLOW_EQUIVALENCE.md](SOMATIC_AGENTIC_FLOW_EQUIVALENCE.md)
> **Epistemic label:** SPEC (specification, hardened by Phase 3 implementation)

---

## Preamble

A Flow Receipt is NOT a standard application log.

Standard logs record **what** happened (telemetry).

A Flow Receipt records the **metabolic and cognitive state** of the agent while it happened (interoception and proprioception). It is the mathematical proof that ΔS ≤ 0 and that F1 (Amanah) was never breached.

Every session MUST generate and seal this artifact into VAULT999.

---

## Section 1: The Four Autonomic Phases

The receipt is divided into four phases mirroring the human autonomic nervous system. Each phase maps to a somatic function in the 11-isomorphism framework.

```
======================================================================
[arifOS] FLOW RECEIPT v1.0
======================================================================
SESSION_ID:  SEAL-<hash>
TIMESTAMP:   <ISO 8601>
INTENT:      <raw 888 input>
----------------------------------------------------------------------

I. PROPRIOCEPTION (Pre-Flight Grounding)
   Checks if the agent is aware of its bounds and tools before moving.
   ─────────────────────────────────────────────────────────────
   A3 Checkpoint     : [ OK | FAILED ]
   Merkle Root       : <state hash>
   Channel<T>        : [ HTTP | NATS | STDIO ] — active transports
   F1 Amanah Guard   : [ ARMED — Reversibility Verified | BREACHED ]

II. COGNITIVE DYNAMICS (DMN ↔ ECN Balance)
   Tracks the Flow Quotient. Too much DMN = paralysis. Too much ECN = drift.
   ─────────────────────────────────────────────────────────────
   E_steps (ECN)     : <int: A-FORGE motor actions executed>
   V_steps (DMN)     : <int: internal verifications run>
   Flow Quotient     : E_steps / V_steps   (Target: > 1.0)
   888-JUDGE Holds   : <int: number of times 888 was queried>

III. SENSORY & MOTOR (Execution)
   Records the interaction with reality and external AI signals.
   ─────────────────────────────────────────────────────────────
   T_Witness Score   : <W³ Nash product, e.g. 0.992>
   P(Truth) Floor    : <lowest confidence score during session>
   Invariant Status  : [ A1-A5 UNBROKEN | BREACHED: <list> ]

IV. INTEROCEPTION (Cooling & Consolidation)
   The parasympathetic response. Ensures the system has metabolized the work.
   ─────────────────────────────────────────────────────────────
   Token Budget      : <burn> / <remaining>
   Cooling Ledger    : [ ΔS ≤ 0 (Converged) | ΔS > 0 (Diverged) ]
   Scars Forged      : <int: number of failures proceduralized>
   VAULT999 Seal     : <SHA-256 hash of this receipt>

======================================================================
```

---

## Section 2: The Flow Quotient — FQ = E_steps / V_steps

This is the **core metric of agentic flow**.

```
FQ = E_steps / V_steps
```

| FQ Range | Diagnosis | Somatic Equivalent | Action |
|----------|-----------|-------------------|--------|
| **FQ > 2.0** | Hyperactive ECN. Executing without verifying. Risk: drift. | Sympathetic overload — running blind | Increase V_steps, engage TRI_WITNESS |
| **FQ 1.0–2.0** | **Optimal.** Executes more than it verifies. Governed but fast. | Healthy autonomic balance | Maintain |
| **FQ 0.5–1.0** | Verifying as much as executing. Acceptable for high-risk domains. | Cautious but functional | Monitor |
| **FQ < 0.5** | Ruminating. Over-verifying. Too much DMN. | mPFC overactive — analysis paralysis | Route to FLAME (enteric), reduce central verify loops |
| **FQ = 0.25** | Severe rumination. 2 exec steps, 8 verify steps. | Cognitive gridlock | HARD intervention: FLAME reroute + reduce V_step triggers |

**The rule:** If an agent takes 2 execution steps but requires 8 verification steps (FQ = 0.25), it is ruminating. The mPFC is overactive. The architecture must dynamically shift to FLAME (local/enteric processing) to push the quotient back above 1.0.

**Implementation:** `src/governance/cooling.rs` — FQ computed at every super-step, stored in cooling entry.

---

## Section 3: W³ Nash Product — Tri-Witness Threshold

```
W³ = ∛(h × a × e)

Where:
  h = human witness confidence [0, 1]
  a = AI witness confidence [0, 1]
  e = external/Earth witness confidence [0, 1]
```

**Epistemic threshold:** ε = 1 × 10⁻⁶

If W³ falls below ε (any channel confidence approaches zero), the Nash product collapses:

```
W³ < ε → DIVERGENT → 888_HOLD → session must halt
```

**Why geometric mean:** Unlike arithmetic mean, the geometric mean collapses to zero if ANY channel is zero. This is the correct behavior — one dissenting witness invalidates the claim.

**Implementation:** `src/governance/tri_witness.rs` — `WitnessMergeResult { CONSENSUS, WEAK, DIVERGENT }`

---

## Section 4: ΔS ≤ 0 — The Final Check

An agent CANNOT seal the session if the cooling ledger shows increased entropy.

| ΔS Sign | Meaning | Seal Eligible |
|---------|---------|:---:|
| **ΔS < 0** | Entropy reduced. Workspace cleaner than found. Agent metabolized the work. | ✅ |
| **ΔS = 0** | Entropy stable. No net chaos added. Acceptable. | ✅ |
| **ΔS > 0** | Entropy increased. Open connections, orphaned files, unresolved ambiguities. | ❌ |

**Entropy sources detected:**
- Open database connections not closed
- Orphaned files not cleaned
- Unresolved ambiguities not flagged
- Channels left open beyond lease expiry
- Uncommitted state not checkpointed

An agent cannot seal a session that left the system more chaotic than it found it. This is F4 CLARITY operationalized as a computable gate.

---

## Section 5: Machine-Readable Envelope (JSON)

The human-readable format (§1) is the canonical artifact. The JSON envelope is the machine-readable transport format for Kabarkan, VAULT999, and cooling ingestion.

```json
{
  "receipt": {
    "version": "1.0",
    "id": "fr_v1_<blake3_12>",
    "timestamp": "ISO-8601",
    "kind": "SESSION | SUPER_STEP | COOLING | MERGE | SCAR | VIOLATION",
    "chain": { "previous": "<prev_id | GENESIS>", "index": 0 }
  },
  "identity": {
    "actor_id": "string",
    "session_id": "string",
    "lease_id": "string",
    "cc_id": "string"
  },
  "proprioception": {
    "a3_checkpoint": "OK | FAILED",
    "merkle_root": "<hex>",
    "channels_active": ["HTTP", "NATS", "STDIO"],
    "f1_amanah": "ARMED | BREACHED"
  },
  "cognitive_dynamics": {
    "e_steps": 0,
    "v_steps": 0,
    "flow_quotient": 0.0,
    "judge_holds": 0
  },
  "sensory_motor": {
    "w3_score": 0.0,
    "p_truth_floor": 0.0,
    "invariant_status": "UNBROKEN | BREACHED",
    "breached_invariants": []
  },
  "interoception": {
    "token_budget": { "burn": 0, "remaining": 0 },
    "cooling_ledger": { "delta_s": 0.0, "convergence": "CONVERGED | DIVERGED" },
    "scars_forged": 0,
    "vault999_seal": "<sha256>"
  },
  "epistemic": {
    "label": "OBS | DER | INT | SPEC",
    "confidence": 0.90,
    "evidence_refs": []
  }
}
```

---

## Section 6: Receipt Lifecycle

```
arif_init ──▶ 888-JUDGE ──▶ arifFlow schedule ──▶ A-FORGE execute
    │              │               │                    │
    ▼              ▼               ▼                    ▼
[identity]    [verdict]      [flow receipt]       [state delta]
    │              │               │                    │
    └──────────────┴───────────────┴────────────────────┘
                              │
                              ▼
                     ┌────────────────┐
                     │  COOLING GATE  │
                     │  ΔS ≤ 0 ?      │
                     │  FQ > 0.5 ?    │
                     │  W³ > ε ?      │
                     └───────┬────────┘
                             │
                     ┌───────▼────────┐
                     │   VAULT999     │
                     │   SEAL         │
                     └────────────────┘
```

**Gate order (non-negotiable):**
1. ΔS ≤ 0 — entropy check (F4)
2. FQ > 0.5 — no paralysis (cognitive dynamics)
3. W³ > ε — tri-witness threshold (F3)
4. A1-A5 unbroken — invariants (F1)
5. VAULT999 append — immutable seal

---

## Section 7: Example — Session-End Receipt

```
======================================================================
[arifOS] FLOW RECEIPT v1.0
======================================================================
SESSION_ID:  SEAL-34b31f38ec604118
TIMESTAMP:   2026-07-25T07:45:00Z
INTENT:      arifFlow sovereign repo scaffold + somatic equivalence
----------------------------------------------------------------------

I. PROPRIOCEPTION
   A3 Checkpoint     : OK
   Merkle Root       : b3e1f8a2...d9e0f1
   Channel<T>        : [ HTTP ]
   F1 Amanah Guard   : ARMED — All mutations reversible (git push only)

II. COGNITIVE DYNAMICS
   E_steps (ECN)     : 9  (git commits, pushes, file writes, gh repo create)
   V_steps (DMN)     : 5  (health probes, git status, repo verify, log checks)
   Flow Quotient     : 1.80  (OPTIMAL — healthy ECN lead)
   888-JUDGE Holds   : 0

III. SENSORY & MOTOR
   T_Witness Score   : 0.000  (UNMEASURED — no tri-witness required for infra)
   P(Truth) Floor    : 1.00  (all claims verified by live probe)
   Invariant Status  : A1-A5 UNBROKEN

IV. INTEROCEPTION
   Token Budget      : ~4,200 / ~180,000 remaining
   Cooling Ledger    : ΔS ≤ 0 (Converged) — repos clean, no orphans
   Scars Forged      : 0  (no failures detected)
   VAULT999 Seal     : sha256:e5e43770df1cfe55...

======================================================================
```

---

## Section 8: Implementation Roadmap

| Artifact | Status | Location |
|----------|--------|----------|
| Cooling ledger (ΔS) | ✅ P1-4 | `src/governance/cooling.rs` |
| TRI_WITNESS (W³) | ✅ P1-3 | `src/governance/tri_witness.rs` |
| Checkpoint (A3) | ✅ P0 | `src/governance/checkpoint.rs` |
| VAULT999 seal | ✅ P0 | `src/governance/vault999.rs` |
| Flow Quotient (FQ) | 🔲 P2 | `src/governance/cooling.rs` — compute E/V ratio |
| Human-readable receipt | 🔲 P2 | `src/governance/receipt.rs` — format as text block |
| JSON envelope | 🔲 P2 | `src/governance/receipt.rs` — serialize to JSON |
| Kabarkan span emission | 🔲 P2 | `src/governance/kabarkan.rs` — RECEIPT_CREATED span |
| Receipt chain verification | 🔲 P2 | `src/merkle.rs` — `verify_receipt_chain()` |
| Seal gate (ΔS, FQ, W³) | 🔲 P2 | `src/scheduler.rs` — pre-seal validation |

---

*Forged 2026.07.25. DITEMPA BUKAN DIBERI.*
*The Flow Receipt is the mathematical proof that governed flow occurred.*
*An agent does not just execute — it proves its sanity at every cycle.*
