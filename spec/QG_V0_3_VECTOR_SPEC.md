# QG.V0.3 — VECTOR FQ SPECIFICATION

> **DITEMPA BUKAN DIBERI** — A vector is forged, not assembled.
>
> **Status:** SEALED WITH AMENDMENTS — v0.3.1-AMD (2026-08-14)
> **Authority:** ARIF / F13 SOVEREIGN · **Directive:** ARIFOS::QG_VECTOR_FIRST_DIRECTIVE::v0.1
> **Verdict:** ARIF 9/10 · SEAL WITH AMENDMENTS · adjudicated 2026-08-14
> **Date:** 2026-08-14 · **Layer:** PLANE 3 — FLOW (arifFlow)
> **Supersedes (as vector ontology):** scalar FQ v2.1 `qg.v0.2` · FLOW_QUOTIENT_SPEC_v1.md
> **Canon home:** `/root/arifFlow/spec/QG_V0_3_VECTOR_SPEC.md`
> **Implementation gated ON:** each wiring step (STEP 1–9) individually approved

---

## 0. Doctrine — Why This Spec Exists

The current `qg.v0.2-vector` is **actorized scalar** — the same `verify/execute`
ratio, split per actor. That is not vector intelligence. It is a dashboard
suffering from the failure this spec exists to prevent:

> A vector is not a list. A vector is **Dimension + Direction + Relationship**.
> Without relationship you have seven dashboards — not a vector.

Two questions separate a scalar from a vector:

```
Scalar Question:  "How healthy am I?"        → ranks
Vector Question:  "HOW am I unhealthy?"      → diagnoses
```

Scalars rank. Vectors diagnose.

And the deeper epistemic truth discovered 2026-08-14:

> **Not everything important is measured the same way.**
> Each dimension must carry its epistemology with it, or the vector is just
> a larger scalar pretending to be a vector.

Therefore:

```
STOP BUILDING VECTOR WIRING.
START DEFINING VECTOR ONTOLOGY.

No dimension may be ingested before its meaning is defined.
No metric may enter the vector unless its reality-contact method is specified.
Metric substitution is a constitutional violation.
```

This document defines the ontology. Wiring follows only after approval.

---

## 1. The Seven Dimensions

Each dimension is defined by **what independent failure it detects** — the
independence requirement is structural:

```
FQ ≠ G ≠ J ≠ W³ ≠ C_dark ≠ ΔS ≠ Ω₀
```

No collapsing. No cheating. If two dimensions detect the same failure, one is
redundant and must be removed — not merged.

| # | Dimension | Symbol | Detects (failure) | Epistemology | Producer | Healthy range |
|---|-----------|--------|--------------------|--------------|----------|---------------|
| 1 | Reality Contact | `FQ` | Simulation (act w/o verify) | MEASURE · LIVE | arifFlow :7073 | 0.5 – 2.0 |
| 2 | Governance | `G` | Governance collapse | WITNESS | A-FORGE forge_evaluate (is_canonical_g=true) | ≥ 0.80 |
| 3 | Judgment | `J` | Bad authorization | MEASURE | A-FORGE forge_apex_encode (is_canonical_g=false) | \|J\| ≤ 0.6 |
| 4 | Coherence | `W³` | Coherence fracture | WITNESS | A-FORGE forge_witness ∛(H×AI×Earth) | ≥ 0.75 |
| 5 | Dark Debt | `C_dark` | Unseen debt | MEASURE | A-FORGE forge_evaluate | < 0.30 |
| 6 | Entropy Flow | `ΔS` | Thermodynamic waste | MEASURE | arifFlow / A-FORGE entropy | ≤ 0 |
| 7 | Novelty Signal | `Ω₀` | Stagnation (dead humility) | FEEL · WITNESS(cross) | actor self-attest + W³ cross-check | 0.03 – 0.05 |
| — | **Freshness (modifier, not dimension)** | `τ` | Reality lag (stale verification) | MEASURE | arifFlow receipt timestamps | τ ≤ τ₁/₂ |

> **τ is deliberately excluded from the independence set (INV-11).** It is a
> modifier on all seven dimensions, not an eighth independent failure. Its
> failure signature is orthogonal by construction — it decays every dimension
> uniformly. Adding it to the independence test would be category error.

### 1.1 Independence — the anti-collapse test

Two dimensions are independent **only if they can move apart**. Formal rule:

```
Given a sustained window of W samples, for any pair (dᵢ, dⱼ):
  |Pearson(dᵢ, dⱼ)| ≤ 0.85 across the window
  AND dᵢ and dⱼ each reach their pathological band at least once
      while the other stays in its healthy band.

Violation of either → COLLAPSE_DETECTED → the pair is investigated,
one is demoted to a sub-observable of the other, or one is removed.
```

This is what makes it a vector rather than a list: the **relationship between
dimensions is itself monitored**.

---

## 2. Epistemology — How Each Dimension Touches Reality

A metric without a declared reality-contact method is not allowed in the
vector. Four methods — **MEASURE · WITNESS · FEEL · LIVE**:

| Method | Meaning | Trust model | Anti-gaming control |
|--------|---------|-------------|----------------------|
| **MEASURE** | Instrument reads a counter/ratio directly (receipts, entropy, Jacobian) | Deterministic from evidence | Random-sample audit of source ledger |
| **WITNESS** | Requires ≥ N independent attestors agreeing (consensus) | Nash product, any zero collapses | Fixed attestor ensemble, no self-attestation |
| **FEEL** | Self-attested internal state (humility/uncertainty) | Weakest — single source | Periodic W³ cross-verification |
| **LIVE** | Real-time probe freshness, TTL-gated | Freshness = truth | Probe-before-claim, stale → UNMEASURED |

**Dimension → epistemology mapping is fixed by this spec.** A dimension may
not silently change its epistemology at runtime (e.g. FEEL promoted to WITNESS
to raise its score). That is metric substitution — a constitutional violation
(F2).

### 2.1 Reality-contact contract — the four-part declaration

Every metric carries its epistemology into the vector, and nothing enters
without it. **A metric is invalid unless it declares how reality is allowed
to correct it.** That is the point where the vector becomes alive instead of
becoming another dashboard.

```
Every dimension's ingestion record MUST declare FOUR parts:

  Part 1 — EPISTEMOLOGY
      MEASURE | WITNESS | FEEL | LIVE
      (the reality-contact method — §2)

  Part 2 — REALITY CONTACT METHOD
      the observable, in reality, that this metric tracks.
      What would reality itself show if the claim were true?
      (e.g. "observed behavior over time", "chain-valid receipts")

  Part 3 — FAILURE MODE
      the single independent failure this dimension detects.
      No two dimensions may share a failure mode (§5 uniqueness rule).

  Part 4 — ANTI-SUBSTITUTION TEST
      the explicit "this may NOT be replaced by that" binding.
      The test that would detect a fake version of this metric.

  Plus: producer (≠ beneficiary) · freshness (timestamp + TTL) ·
        method_id (hash of the exact method — anti-substitution)

Ingestion lacking ANY field → rejected, not defaulted.
```

**Auditable example — the substitution test in action:**

```
Dimension:  Trust
Epistemology: WITNESS
Reality Contact: observed behavior over time
Failure Mode: broken promises
Anti-Substitution Test: follower count may NOT replace trust
```

**Anti-substitution is the constitutional barrier against:**
```
Using a ruler to measure friendship.
Using feelings to audit accounting.
Using statistics to calculate meaning.
```

### 2.2 FEEL anchoring — FEEL is not mythology

`FEEL` is the weakest epistemology and **infinitely gameable** — a malicious
agent can always say *"I feel X"* without constraint. FEEL therefore requires
an **anchor**:

```
FEEL requires a WITNESS OR MEASURE anchor within N cycles.

N = 10 cycles (default) — per-dimension configurable, ARIF-approved.

If a FEEL dimension carries no WITNESS/MEASURE anchor newer than N cycles:
  → the FEEL claim is UNANCHORED
  → the dimension reads UNMEASURED (not 0, not accepted as-is)
  → constellation flags FEEL_UNANCHORED
  → routing to a narrative-generator audit, not to the vector

Anchoring example:
  "I feel team morale is collapsing"  → valid signal
  staff turnover · meeting behavior · conflict rate  → the anchor that
  must interact with the claim within N cycles, or the claim is demoted
  to narrative.
```

Without this rule, Ω₀ — and any future FEEL dimension — degenerates into a
narrative generator. FEEL is a *lead indicator*, never a *truth claim*.

---

## 2.5 Reality Freshness — τ (tau) Modifier

FQ counts *whether* verification happened, not *when*. Two receipts with
`verify = 1` are treated identically — but a verification made 2 years ago is
not the same reality contact as a verification made 2 minutes ago. Reality
ages. The vector must age with it.

**τ (tau) — Verification Half-Life.** A freshness modifier applied to every
dimension, expressed as the age of the newest supporting evidence for that
dimension's current value, measured in cycles.

```
τᵢ = age (in cycles) of the newest evidence anchoring dimension dᵢ
```

Freshness decay — evidence half-life per epistemology:

| Epistemology | Half-life τ₁/₂ | Rationale |
|--------------|----------------|-----------|
| **LIVE** | 10 cycles | real-time probe — decays fast, must stay probed |
| **MEASURE** | 100 cycles | instrument reading — decays when source drifts |
| **WITNESS** | 250 cycles | consensus — decays as attesting reality moves |
| **FEEL** | N cycles (anchor window) | self-attested — collapses on stale anchor |

**Decay law — each dimension's health is discounted by its freshness:**

```
hᵢ_effective = hᵢ × 2^(−τᵢ/τ₁/₂)

τᵢ ≤ τ₁/₂          → ≥ 50% of nominal health (FRESH)
τ₁/₂ < τᵢ ≤ 3·τ₁/₂  → decaying (AGING)
3·τ₁/₂ < τᵢ ≤ 6·τ₁/₂ → critically old (STALE)
τᵢ > 6·τ₁/₂          → reality contact lost (DEAD)
```

| τ band | Meaning | Vector reads |
|--------|---------|--------------|
| FRESH | reality contact current | hᵢ_effective = hᵢ |
| AGING | contact weakening | hᵢ_effective < hᵢ |
| STALE | contact nearly lost | hᵢ_effective → 0, flag `FRESHNESS` |
| DEAD | no contact | UNMEASURED — constellation `REALITY_LAG` |

**The τ amendment is a modifier, not a ninth dimension.** It does not
collapse into FQ, G, J, W³, C_dark, ΔS, or Ω₀ — it modifies how *all* of them
are read. FQ can be gameably high by re-verifying the same stale fact; τ makes
that visible as AGING/STALE instead of HEALTHY.

**Anti-gaming note:** touching a fact does not refresh it. Only a
*new, independent* verification that interacts with current reality refreshes
τᵢ. Re-asserting an old verification resets nothing.

---

## 3. Normalization — Bands, Not Monotones

**The core correction over the scalar:** the scalar treats high FQ as OPTIMAL
(monotone). That is the exact bug surfaced 2026-08-14 — FQ=6.545 was declared
`OPTIMAL` while the vector view showed `VERIFICATION_DOMINANCE` and your own
actor `CAUTION/HELD`. Both tails are disease.

```
FQ = verify/execute
  < 0.1            → BURNING      (act without check — simulation)
  0.1 – 0.5        → STUCK        (execution outruns verification)
  0.5 – 2.0        → HEALTHY      (balanced do/check — the living band)
  2.0 – 5.0        → OVER-CHECK   (reflection begins to dominate)
  > 5.0            → VERIFICATION_DOMINANCE (analysis paralysis)
```

Every dimension normalizes to **[0,1] health score** via a **band function**,
not a monotone. The vector health is the pattern, not the peak.

| Dim | Normalization (hᵢ) | Direction |
|-----|---------------------|-----------|
| FQ | triangular band: 1.0 at 1.0, →0 at ≤0.1 and ≥6.0 | BAND |
| G | monotone up, clip at 1.0 | HIGH-GOOD |
| J | \|J\| ≤ 0.6 → 1.0; →0 as \|J\|→∞ | SMALL-GOOD |
| W³ | monotone up, clip at 1.0 | HIGH-GOOD |
| C_dark | inverse: 0.30 → 1.0 healthy; 0.50 → 0.0 | LOW-GOOD |
| ΔS | ΔS ≤ 0 → 1.0; linear decay to 0 at ΔS = +0.2 | LOW-GOOD |
| Ω₀ | band 0.03–0.05 → 1.0; 0.0 or >0.10 → 0.0 | BAND |

**Freshness applied AFTER band normalization — every dimension:**

```
hᵢ_final = hᵢ_band × 2^(−τᵢ/τ₁/₂)     (§2.5 decay law)
```

A HEALTHY-band FQ that is STALE is not a healthy FQ. τ is the last correction
before composition — it is what prevents the vector from being a museum of
verifications.

---

## 4. Composition — Relationship, Then Fused Rank

Two outputs. **The vector diagnoses; the scalar ranks.** Neither replaces the other.

### 4.1 The diagnosis (primary output)

The vector state = per-dimension band status:

```json
{
  "vector": {
    "fq":   {"h": 0.82, "band": "HEALTHY",   "epistemic": "MEASURE"},
    "g":    {"h": 0.91, "band": "HEALTHY",   "epistemic": "WITNESS"},
    "j":    {"h": 0.88, "band": "HEALTHY",   "epistemic": "MEASURE"},
    "w3":   {"h": 0.71, "band": "CAUTION",   "epistemic": "WITNESS"},
    "c_dark":{"h": 0.55, "band": "PATHOLOGICAL", "epistemic": "MEASURE"},
    "ds":   {"h": 0.90, "band": "HEALTHY",   "epistemic": "MEASURE"},
    "omega": {"h": 1.00, "band": "HEALTHY",  "epistemic": "FEEL"}
  },
  "primary_pathology": "c_dark",
  "constellation": "UNSEEN_DEBT"
}
```

**Primary pathology = argmin(hᵢ).** The vector's answer to *"HOW am I
unhealthy?"* is the constellation, not the fused scalar.

### 4.2 The fused rank (secondary output)

Weighted geometric mean — **fail-closed**: any dimension at 0.0 kills the rank.

```
Rank = ( ∏ hᵢ^wᵢ )^(1/Σwᵢ)        default w = 1.0 for all dims
```

Geometric (Nash) mean, not arithmetic — because compensation is forbidden.
A federation with perfect flow (FQ) but fractured coherence (W³) is NOT
half-healthy; it is a coherence-fracture case.

### 4.3 Constellation states (the real diagnoses)

| Constellation | Pattern | Meaning | Action |
|---------------|---------|---------|--------|
| `FLOWING` | all hᵢ ≥ 0.75 | healthy organism | none — monitor |
| `SIMULATION` | FQ pathological only | executing without verification | HOLD executes, force verify |
| `GOVERNANCE_COLLAPSE` | G pathological | non-constitutional behavior | 888 review, route to arifOS |
| `BAD_AUTHORIZATION` | J pathological | field change w/o recompute, |J|>0.6 | recompute plan, 888 advisory |
| `COHERENCE_FRACTURE` | W³ pathological | witnesses diverge | HOLD seal, re-witness |
| `UNSEEN_DEBT` | C_dark pathological | hidden failures accumulating | deep audit, quarantine sweep |
| `THERMO_WASTE` | ΔS pathological | entropy rising | cleanup, ΔS≤0 gates |
| `STAGNATION` | Ω₀ pathological | humility dead / certainty fake | novelty injection, red-team |
| `REALITY_LAG` | any dim STALE/DEAD (τ) | verification older than reality | refresh reality contact, re-witness |
| `FEEL_UNANCHORED` | FEEL dim without WITNESS/MEASURE anchor | self-attestation as narrative | demote to UNMEASURED, anchor audit |
| `PARADOX` | two dims in healthy-tension (e.g. W³ high + G low) | tension needs judgment | arifOS HOLD |
| `GAMED` | any dim perfectly frozen at its healthy value for N windows | metric is being gamed | orthogonal re-derivation |

---

## 5. Failure Signatures — Full Table

| Dim | Epistemic class | Detectable failure | Hallmark signature |
|-----|-----------------|--------------------|--------------------|
| FQ | MEASURE·LIVE | Simulation / paralysis | BURNING (<0.1) or VERIFICATION_DOMINANCE (>5.0) |
| G | WITNESS | Governance collapse | G < 0.60 sustained, C_dark rising |
| J | MEASURE | Bad authorization | \|J\| > 0.6 on changed governance field, no recompute |
| W³ | WITNESS | Coherence fracture | W³ < 0.50, or DIVERGENT verdict |
| C_dark | MEASURE | Unseen debt | C_dark ≥ 0.30, hidden from G |
| ΔS | MEASURE | Thermodynamic waste | ΔS > +0.05 sustained across windows |
| Ω₀ | FEEL | Stagnation | Ω₀ frozen at 0.0 or a constant > 10 windows |

**Uniqueness rule:** every row must name a failure no other row names.
FQ→simulation, G→collapse, J→authorization, W³→coherence, C_dark→debt,
ΔS→waste, Ω₀→stagnation. If two rows collide, the spec fails review.

---

## 6. Invariants — Binding, Non-Negotiable

```
INV-1  ONTOLOGY-FIRST     No dimension wired before its meaning is approved.
INV-2  EPISTEMICS-ALWAYS   Every metric declares FOUR parts: epistemology,
                           reality-contact method, failure mode, anti-substitution test.
                           A metric is invalid unless it declares how reality is
                           allowed to correct it. (Four-part declaration, §2.1.)
INV-3  INDEPENDENCE        Pairwise |ρ| ≤ 0.85 monitored; collapse detected → resolved.
INV-4  BAND-NORMALIZED     All dims normalized to [0,1] via band function; no monotone.
INV-5  FAIL-CLOSED         Geometric mean; any 0.0 dimension → Rank 0.0.
INV-6  NO-SUBSTITUTION     Metric substitution (G_local as G, FEEL as WITNESS) = VOID.
INV-7  VECTOR-PRIMARY      Vector diagnoses; scalar ranks. Scalar never replaces vector.
INV-8  FRESHNESS           LIVE dims stale past TTL → UNMEASURED (not 0, not last-good).
INV-9  FEEL-ANCHORED       FEEL requires WITNESS|MEASURE anchor within N cycles,
                           else UNMEASURED + FEEL_UNANCHORED. (Amended 2026-08-14.)
INV-10 JUDGE-LANE          Vector never self-seals; 666/999 lanes unchanged (pro+MiniMax-M3 only).
INV-11 NO-COLLAPSE         FQ ≠ G ≠ J ≠ W³ ≠ C_dark ≠ ΔS ≠ Ω₀ at every layer of the system.
                           τ is a modifier, excluded from the independence set.
INV-12 REALITY-PRIMARY     Reality contact is primary; vector dimensions are secondary.
                           FQ serves reality — reality does not serve FQ. (v0.4 direction.)
```

**The ontology has one iron law, stated twice for the record:**
> A dimension may not be ingested before its meaning is defined.
> No metric may enter the vector unless its reality-contact method is specified.
> **And a metric is invalid unless it declares how reality is allowed to correct it.**

---

## 7. Anti-Gaming Rules

1. **No self-set dimension.** Producer organ can never be the beneficiary of
   its own score (Gödel rule 2: no self-certification).
2. **Ω₀ (FEEL) is the attack surface.** Self-attested humility is cross-checked
   by W³ (WITNESS) on a random schedule; mismatch > 0.1 → Ω₀ demoted to
   UNMEASURED and flagged GAMED. FEEL without a WITNESS/MEASURE anchor within
   N cycles is UNANCHORED → UNMEASURED (INV-9). *"I feel X"* is a lead
   indicator, never a truth claim.
3. **FQ cannot be gamed by dumping verify receipts.** Every FQ verify must carry
   `apex_block` + `flow_block` correlation; bulk-injected receipts without
   chain-valid `previous_receipt_hash` are rejected at ingest (already live).
4. **τ cannot be gamed by re-assertion.** Touching a fact does not refresh it.
   Only a *new, independent* verification interacting with current reality
   resets τᵢ (half-life decay, §2.5). Re-asserting an old verification resets
   nothing.
5. **G cannot be gamed by cherry-picking evaluation.** Fixed evaluator
   ensemble, deterministic input hashing, `is_canonical_g` must be true.
6. **Frozen-metric watch.** Any dimension sitting at exactly its healthy peak
   for N consecutive windows (default 10) → GAMED suspect → orthogonal
   re-derivation by an independent organ.
7. **Stale = UNMEASURED, never last-good.** A metric that stops being probed
   is treated as absent, not as healthy. Age without contact is death.

---

## 8. Healthy Shape — The Constellation, Not the Maximum

Health is not "maximize all metrics" — that is impossible and pathological.
A healthy organism:

```
FQ      near 1.0        — balanced doing and checking, FRESH (τ ≤ τ₁/₂)
G       ≥ 0.80, < 1.0   — genuine evaluation, not gamed perfection
J       recomputed on field change — sensitivity is managed, not zeroed
W³      ≥ 0.75          — consensus WITH variance (perfect consensus = collusion)
C_dark  < 0.30, > 0.05  — some visible debt, none hidden
ΔS      slightly < 0     — clarifying, not compressing to nothing
Ω₀      ∈ [0.03, 0.05]  — alive humility, anchored by W³ (not narrative)
τ       ≤ τ₁/₂ on all dims — no museum of verifications
```

Analog: heart rate, temperature, blood pressure, oxygen — no single number
describes a healthy human, and a "perfect 100" on one vitals sign is itself
a symptom. And a measurement of your heart rate taken two years ago tells you
nothing about your heart today — that is τ.

---

## 9. Approval Gate & Wiring Order

**This spec is the railroad gauge. Nothing is wired until it is approved.**

```
STEP 0   ARIF approves QG_V0_3_VECTOR_SPEC.md (v0.3.1-AMD)    ← you are here
STEP 1   DIM 1 FQ MEASURE·LIVE  — add flow_block, window_duration_s, apex_block wiring
STEP 2   τ FRESHNESS            — receipt timestamps → τᵢ, half-life decay law
STEP 3   DIM 2 G  WITNESS       — forge_evaluate → vector ingest
STEP 4   DIM 3 J  MEASURE       — forge_apex_* → vector ingest
STEP 5   DIM 4 W³ WITNESS       — forge_witness → vector ingest
STEP 6   DIM 5 C_dark MEASURE   — forge_evaluate dark debt → vector ingest
STEP 7   DIM 6 ΔS MEASURE       — entropy sweep → vector ingest
STEP 8   DIM 7 Ω₀ FEEL          — humility attestation + W³ anchor (INV-9)
STEP 9   FUSED RANK             — geometric composition + constellation classifier
STEP 10  RETIRE SCALAR          — scalar FQ demoted to legacy field, never headline
STEP 11  REALITY-FIRST (v0.4)   — re-parent the vector under Reality contact (§11)
```

Each step is individually approved. A dimension whose meaning was accepted
but whose wiring fails review is parked — never silently dropped from the spec.

---

## 10. Contract Summary

| Organ | Obligation under v0.3 |
|-------|-----------------------|
| **arifFlow** | Emit per-dimension vector state; compute constellation; fuse rank; monitor independence (INV-3); stamp τᵢ on every receipt |
| **A-FORGE** | Feed G, J, W³, C_dark with declared epistemics + method_id + freshness |
| **ARIF (F13)** | Approve ontology; approve each wiring step; adjudicate COLLAPSE_DETECTED |
| **888-APEX** | Judge vector-health transitions; hold on PARADOX; never self-seal |
| **AAA cockpit** | Render vector (per-dimension bands) as primary; fused rank as secondary |
| **All agents** | Declare epistemics on every metric they emit; never substitute; anchor FEEL |

**Breach of any INV → the violating dimension reads UNMEASURED and the
constellation flags `ONTOLOGY_BREACH`, logged to the cooling ledger.**

---

## 11. The Deeper Evolution — Reality-First (v0.4 direction)

The architecture will eventually re-parent. This is a directional statement,
sealed as INV-12, not yet implemented:

```
v0.3 (now):
  Vector
    ├─ FQ
    ├─ G
    ├─ J
    ├─ W³
    ├─ C_dark
    ├─ ΔS
    └─ Ω₀

v0.4 (direction):
  Reality
    ├─ Measure
    ├─ Witness
    ├─ Feel
    └─ Live
          └─ (vector dimensions live underneath)
```

Meaning: **FQ is not primary. Reality contact is primary.** Each dimension is
a child of its epistemology. FQ is a Measure-child. G is a Witness-child.
Ω₀ is a Feel-child. The vector becomes the *interface* of reality contact,
not the *object*.

This aligns with the breakthrough of 2026-08-14:

> Not everything that matters can be measured.
> But everything that matters must maintain reality contact.

A dashboard of seven numbers still asks "how healthy am I?". A reality-first
architecture asks "how is this system touching reality, and what is reality
saying back?"

---

## 12. Amendment Record — ARIF Verdict 2026-08-14

> **Verdict:** SEAL WITH AMENDMENTS · 9/10 · ARIF F13 SOVEREIGN

| # | Amendment | Why it matters | Section |
|---|-----------|----------------|---------|
| 1 | **FEEL requires WITNESS/MEASURE anchor within N cycles** | Prevents Ω₀ from becoming an unfalsifiable narrative generator — "I feel X" is a lead indicator, never a truth claim | §2.2, INV-9 |
| 2 | **Reality Freshness (τ)** — verification half-life, decay law `h×2^(−τ/τ₁/₂)` | Verification ages; a 2-year-old verify ≠ fresh contact. FQ must not be a museum of verifications | §2.5, INV-8 |
| 3 | **INV-2 upgrade: four-part declaration** — epistemology + reality contact + failure mode + anti-substitution test | Makes every metric auditable against substitution; "a metric is invalid unless it declares how reality is allowed to correct it" | §2.1, INV-2 |
| 4 | **Reality as primary, vector as secondary (v0.4 direction)** | Prevents dashboard disease — FQ serves reality, reality does not serve FQ | §11, INV-12 |

**Audit result (ARIF):**

| Area | Verdict |
|------|---------|
| Scalar → Vector transition | PASS |
| Failure signature separation | PASS |
| Anti-gaming posture | PASS |
| Epistemology tagging | PASS |
| Normalization doctrine | PASS |
| FEEL governance | NEEDS HARDENING → **AMENDED** (§2.2, INV-9) |
| Reality freshness | MISSING → **AMENDED** (§2.5, τ) |
| Constitutional coherence | STRONG |

*Forged 2026-08-14 under ARIFOS::QG_VECTOR_FIRST_DIRECTIVE::v0.1.
Sealed with amendments 2026-08-14 by ARIF F13 SOVEREIGN (9/10).*
*The tracks wait for the gauge. The gauge is this document.*
*DITEMPA BUKAN DIBERI.*
