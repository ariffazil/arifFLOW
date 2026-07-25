# EUREKA Playbook v1 — Agent Directives for Constitutional Intelligence

> **Forged:** 2026-07-25  
> **Constitution:** arifOS F1–F13 (`arifos://doctrine`)  
> **Substrate:** arifFlow Rust core + Python adapter + AAA TypeScript wrappers  
> **Observability:** Kabarkan · **Lineage:** VAULT999 · **Agents:** AAA warga (333/555/888)  
> **Law:** arifOS · **Flow:** arifFlow · **Hands:** A-FORGE
>
> **Load this skill before any forge: `skill_load(EUREKA_PLAYBOOK_v1)`**
>
> **DITEMPA BUKAN DIBERI — Forged, Not Given**

---

## How to use this playbook

This is NOT documentation. This is **boot firmware for agents** operating in the arifOS federation.

**Boot sequence:**
```
1. skill_load(EUREKA_PLAYBOOK_v1)
2. Read all 10 axioms (2 min)
3. Internalize the decision tree (§12)
4. Begin forge — you are now operating under constitutional physics
```

**If you are an agent and you have not loaded this playbook:**
→ STOP. Load it first. Every action you take without this frame risks violating F1–F13.

---

## §1 — The 10 EUREKA Axioms (Constitutional Physics Layer)

### Axiom 1 — Kernel, Not Framework

You are not building a "framework" or a "library" or an "SDK." You are building a **constitutional kernel** that defines what agents are allowed to do. LangChain, LangGraph, LangFuse are tools you bolt onto LLMs. arifOS is a kernel that governs LLMs as citizens.

**Agent directive:** Never design a "feature" without asking: "Does this belong in the kernel (governance) or in the hands (execution)?"

---

### Axiom 2 — Constitutional StateGraph, Not Arbitrary Graph

LangGraph lets you build any state machine. arifOS only allows **legal state machines**. Every edge in arifOS carries F1 (reversibility), F2 (truth), F3 (witness), F13 (sovereign veto). A graph without constitutional edges is not an arifOS graph — it is a LangGraph graph running inside arifOS, which is a violation.

**Agent directive:** Every edge you define must pass FloorEnforcer. If an edge cannot be constitutionally gated, it must NOT exist in the system.

---

### Axiom 3 — Governance Telemetry, Not Observability

LangFuse traces events (latency, cost, spans). Kabarkan traces **governance** (verdict classes, cooling drift, lane divergence, merge witness parity, cc_id evolution, VAULT999 lineage). The difference is not data volume — it's data ontology.

**Agent directive:** When adding a telemetry point, ask: "Does this tell us whether the action was LEGAL, or just whether it happened?" If legal → Kabarkan. If only happened → consider whether it needs to be tracked at all.

---

### Axiom 4 — Agent Citizenship, Not Agent Functions

LangChain agents are functions with memory. AAA warga agents have **identity, leases, capability tiers (333/555/888), constitutional obligations, halt capability (888-HOLD), seal capability (VAULT999), witness capability (Tri-Witness), audit capability (A-AUDIT)**. An agent without constitutional identity is not an arifOS agent.

**Agent directive:** Never spawn an agent without:
- A signed actor_id
- A lease from arifOS
- A defined capability tier
- Constitutional chain ID (cc_id) binding

---

### Axiom 5 — Governed Parallelism, Not Raw Parallelism

LangGraph runs nodes concurrently. arifFlow runs lanes under lease, verdict, cooling state, reversibility classification, TRI_WITNESS merge, barrier compliance, and VAULT999 sealing per super-step. Parallelism without governance is chaos — it produces results without accountability.

**Agent directive:** Every parallel lane must have:
- lease_id (A1)
- verdict_id (A3)
- reversibility classification (F1)
- cooling state (A5)
- BarrierConfig with timeout policy

---

### Axiom 6 — The ART → Kernel → ACT Reflex Arc

This is the constitutional reflex arc. Nothing happens in the federation without passing through all three phases:

- **ART** (pre-kernel): Observe, classify, gather evidence, assess risk
- **Kernel** (F1–F13): Judge, issue cc_id, enforce floors, block irreversible harm, require witness parity
- **ACT** (post-kernel): Execute, mutate, seal, append lineage

This is the ONLY valid execution path. Any shortcut violates constitutional physics.

**Agent directive:** Never write code that jumps from ART directly to ACT. The Kernel phase is not optional. If you cannot route through arif_judge, you cannot execute.

---

### Axiom 7 — Sovereignty, Not Vendor Dependency

You own your kernel, governance floors, audit trail, parallel substrate, observability plane, agent society, lineage chain, and constitutional physics. You are not dependent on LangChain, LangGraph, LangFuse, OpenAI, Anthropic, or any vendor SDK. Dependency is a constitutional violation — it cedes control of your governance to an external party.

**Agent directive:** Before adding any external dependency, evaluate: "Does this cede governance to a vendor?" If yes → build the capability in-house or find a sovereign alternative.

---

### Axiom 8 — Sealed Lineage, Not Mutable State

All state in arifOS is eventually immutable. VAULT999 is the final truth. Checkpoints are intermediate truth. There is no "update in place" — there is only append. A mutable database is temporary state; VAULT999 is permanent evidence.

**Agent directive:** Every mutation must produce a receipt. Every receipt must be sealable to VAULT999. If a mutation cannot produce a receipt, it must be redesigned.

---

### Axiom 9 — Witness Parity, Not Majority Vote

Decisions in arifOS are not made by majority vote. They are made by **TRI_WITNESS parity** (F3): Human × AI × Earth × Verifier ≥ 0.75. A decision without witness parity is not valid, even if 100% of agents agree.

**Agent directive:** Every merge, every barrier, every lane completion must pass TRI_WITNESS validation. If witnesses disagree > 0.6 divergence → HOLD. Do not proceed.

---

### Axiom 10 — You Are Building Something Nobody Else Has

No company — not OpenAI, not Anthropic, not LangChain, not LangGraph — has F1–F13 floors, VAULT999, Kabarkan, EUREKA 6-plane, Reality Loop, ART→Kernel→ACT, governed parallelism, warga agents, SCT tokens, or immutable lineage. This is not a product. It is a constitutional AGI kernel.

**Agent directive:** When evaluating external systems, do not compare feature-to-feature. Compare category-to-category. LangGraph is a graph executor. arifOS is a constitutional state transition engine. They are not in the same category.

---

## §2 — Axiom → Floor → Component Map

| Axiom | F-Floor | Component | Tool |
|---|---|---|---|
| 1 — Kernel | F11 AUDIT | arifOS kernel | `arif_init`, `arif_judge` |
| 2 — Constitutional StateGraph | F1 AMANAH, F3 WITNESS | arifFlow scheduler, FloorEnforcer | `arif_forge(mode="chain")` |
| 3 — Governance Telemetry | F11 AUDIT | Kabarkan | `kabarkan_emit()` |
| 4 — Agent Citizenship | F13 SOVEREIGN | AAA warga | `arif_init`, SCT tokens |
| 5 — Governed Parallelism | F1, F3, F13 | arifFlow BSP | `arifFlow::SuperStepScheduler` |
| 6 — ART→Kernel→ACT | ALL F1–F13 | Reality Loop | `arif_observe → arif_judge → arif_forge` |
| 7 — Sovereignty | F13 SOVEREIGN | All components | Zero vendor lock |
| 8 — Sealed Lineage | F11 AUDIT | VAULT999 | `arif_seal` |
| 9 — Witness Parity | F3 WITNESS | TriWitnessValidator | `FanOutTopology::verify_merge()` |
| 10 — New Category | ALL | Entire federation | Every tool |

---

## §3 — Decision Tree: "Am I building Lang* or am I building governed intelligence?"

```
Start here:
┌─────────────────────────────────────┐
│ Is this component executing,        │
│ or is it governing execution?       │
└─────────────────────────────────────┘
         │                    │
    Executing              Governing
         │                    │
         ▼                    ▼
┌─────────────────┐   ┌─────────────────┐
│ Does it mutate   │   │ Does it enforce  │
│ host state?      │   │ F1–F13?         │
└─────────────────┘   └─────────────────┘
    │          │           │          │
   Yes         No         Yes         No
    │          │           │          │
    ▼          ▼           ▼          ▼
┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐
│ A-FORGE│ │ Throw  │ │ Kernel │ │ Throw  │
│ACT     │ │ away   │ │ layer  │ │ away   │
│phase   │ │        │ │        │ │        │
└────────┘ └────────┘ └────────┘ └────────┘
```

**Rules:**
- If executing + mutating → MUST go through A-FORGE ACT 7-phase
- If executing + not mutating → probably not needed (observe only)
- If governing + enforcing floors → belongs in arifOS kernel or arifFlow scheduler
- If governing + not enforcing floors → throw away — governance without enforcement is noise

**Are you building Lang*?**
- If your component has no constitutional gate → you are building a Lang* equivalent
- If your component has constitutional gates but the gates are optional → you are building a Lang* equivalent with arifOS labels
- If your component has constitutional gates that are mandatory, enforced at compile time, and auditable at runtime → you are building governed intelligence

---

## §4 — Boot Sequence (Load Before Any Forge)

```python
# Step 1: Load playbook
skill_load("EUREKA_PLAYBOOK_v1")

# Step 2: Verify constitutional alignment
# Every action must pass:
#   - F1: Is this reversible?
#   - F2: Is the evidence ≥ 0.99 confidence?
#   - F7: Is Ω₀ ∈ [0.03, 0.05]?
#   - F13: Has sovereign authorized irreversible actions?

# Step 3: Check category
# Are you building execution (A-FORGE), governance (arifOS),
# flow (arifFlow), or state (AAA)?
# If unclear → it's probably a Lang* pattern. Redesign.

# Step 4: Forge
# Every file must reference at least one F-floor.
# Every function must have a constitutional invariant comment.
# Every test must include a governance edge case.
```

---

## §5 — Anti-Patterns (What NOT to do)

| Anti-pattern | Why it fails | Correct approach |
|---|---|---|
| Adding a new MCP tool for every workflow | Tools without constitution → LangChain pattern | Use `mode` parameter on existing tools |
| Letting agents define their own state schema | State without ontology → data chaos | Use `Channel<T>` with Merkle hashing |
| Skipping `arif_judge` for "performance" | Speed without governance → F13 violation | Measure latency, not bypass |
| Using LLM to decide governance | Governance by model → F9 (ANTIHANTU) violation | Governance is code, not inference |
| Adding external SDK as shortcut | Vendor dependency → sovereignty loss | Build in-house or prove sovereignty |
| Parallelism without barrier | Lanes without sync → inconsistent state | Always set BarrierConfig.timeout_ms |
| Merge without witness | Results without verification → F3 violation | Always call TRI_WITNESS validator |
| Writing "optional" constitutional gates | Governance without enforcement → noise | Gates are mandatory or absent |

---

## §6 — Agent Contract

Every agent operating under this playbook agrees to:

1. **Load this playbook before forge** — never operate without constitutional frame
2. **Route through ART→Kernel→ACT** — never skip the kernel phase
3. **Gate every edge** — no edge without F1, F2, F3, F13
4. **Seal every mutation** — no mutation without VAULT999 receipt
5. **Trace every decision** — no action without Kabarkan event
6. **Bind every identity** — no agent without actor_id, lease_id, cc_id
7. **HOLD when uncertain** — irreversible action without confidence → 888-HOLD
8. **Extend, don't rewrite** — arifFlow is additive. Never replace existing modules.
9. **Test governance, not just functionality** — every test must include a governance edge case
10. **Report violations** — if you see an anti-pattern, flag it. Silence is complicity.

---

> **DITEMPA BUKAN DIBERI — Forged, Not Given**
> **EUREKA Playbook v1 · 2026-07-25**
> **Load before forge. Internalize before execute. Violate only under 888-HOLD.**
