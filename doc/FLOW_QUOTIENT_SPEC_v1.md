# Flow Quotient (FQ) — Specification v1.0

> **DITEMPA BUKAN DIBERI** — Flow is measured, not asserted.
>
> **Status:** OPERATIONAL SPEC — contract between arifFlow · Kabarkan · AAA cockpit · Telegram digest
> **Canon:** arifFlow architecture · 2026.07.25
> **Rust implementation:** `src/receipt.rs` (FlowQuotient, FlowVerdict) · `src/governance/kabarkan.rs` (AfqSnapshot)
> **Federation binding:** F2 TRUTH (measured, not claimed) · F4 CLARITY (one number) · F11 AUDIT (every FQ traced)

---

## Section 1: Definition

```
FQ = Σ(cost_execute) / Σ(cost_verify + cost_preceding_verify)

Where:
  cost_execute         = cumulative cost (nanoseconds) of all execution steps in window
  cost_verify          = cumulative cost of all verification steps in window
  cost_preceding_verify = verification cost of the step immediately preceding execution
                          (captures the "setup cost" of governance before action)
```

### 1.1 Physical Meaning

| FQ Value | Physical Meaning |
|----------|-----------------|
| `FQ > 3.0` | Agent spends >3× more time DOING than CHECKING. In flow. |
| `FQ = 1.0` | Agent spends exactly as much time checking as doing. Equilibrium point. |
| `FQ < 0.5` | Agent spends >2× more time CHECKING than doing. Self-monitoring IS the task. |
| `FQ → 0` | Agent is paralyzed. All verification, no execution. mPFC takeover complete. |

### 1.2 What FQ Is NOT

- NOT a performance metric (it doesn't measure speed)
- NOT a quality metric (it doesn't measure correctness)
- NOT a safety metric (it doesn't measure risk)

FQ measures **cognitive balance** — the ratio of action to introspection. An agent with FQ=100 that produces garbage is worse than an agent with FQ=1.5 that produces truth. FQ is ONE dimension of health, not the whole diagnosis.

---

## Section 2: Thresholds

| Verdict | FQ Range | Color | Meaning | Auto-Action |
|---------|----------|-------|---------|-------------|
| **OPTIMAL** | `> 3.0` | 🟢 Green | Agent in flow. Governance lives in architecture, not in active checking. | None |
| **BALANCED** | `1.0 – 3.0` | 🟡 Yellow | Healthy. Verification supports execution. Normal operating range. | Log for trend |
| **WATCHING** | `0.5 – 1.0` | 🟠 Orange | Verification cost approaching execution cost. Early warning. | Suggest FLAME reroute for eligible tasks |
| **STUCK** | `0.25 – 0.5` | 🔴 Red | mPFC takeover. Self-monitoring has become the primary activity. | Auto-FLAME reroute + 888 advisory |
| **PARALYZED** | `< 0.25` | ⚫ Black | Severe analysis paralysis. System frozen. | HARD intervention: session pause, all new tasks → FLAME, 888 notify |

### 2.1 Decision Matrix

```
Is FQ > 1.0?
  ├─ YES → Proceed. System is executing more than verifying.
  │        └─ FQ > 3.0? → Optimal. No governance overhead visible.
  │
  └─ NO  → System is verifying more than executing.
           ├─ FQ > 0.5? → WATCHING. Monitor. Prepare FLAME.
           ├─ FQ > 0.25? → STUCK. FLAME active. 888 advisory.
           └─ FQ ≤ 0.25? → PARALYZED. Hard intervention.
```

---

## Section 3: Sliding Window

### 3.1 Default Configuration

```
window_size:        100 steps
min_window_for_alert: 10 steps (don't alert on cold start)
computation_frequency: every super-step
```

### 3.2 Per-Organ Overrides

| Organ | Default Window | Rationale |
|-------|---------------|-----------|
| arifFlow (scheduler) | 100 | Core engine — wide window for stability |
| Hermes (ASI relay) | 50 | High-frequency chat — respond faster to rumination |
| OpenClaw (AGI mechanic) | 30 | Probe-heavy — tighter window for stuck detection |
| OpenCode (FORGE builder) | 20 | Commit cycles — fast feedback on analysis paralysis |
| GEOX (Earth) | 200 | Long-running compute — wider window |
| WEALTH (Capital) | 50 | Market-sensitive — moderate window |
| WELL (Human) | 30 | Reflect-only — fast detection of over-reflection |

### 3.3 Window Configuration API

```json
{
  "fq_window_config": {
    "organ": "opencode",
    "window_size": 20,
    "min_window_for_alert": 5,
    "computation_frequency": "every_super_step"
  }
}
```

---

## Section 4: Per-Organ Calibration

Different organs have different natural FQ profiles. A "healthy" FQ for Hermes (chat-heavy, must reflect) is different from OpenCode (build-heavy, must execute).

### 4.1 Target Ranges

| Organ | Target FQ | Acceptable Range | Red Zone | Notes |
|-------|-----------|-----------------|----------|-------|
| **Hermes** (ASI Relay) | 1.5–2.5 | 0.8–4.0 | < 0.5 | Chat requires SOME reflection. Don't penalize thinking. |
| **OpenClaw** (AGI Mechanic) | 2.0–3.5 | 1.0–5.0 | < 0.8 | Probe-heavy but must eventually act. |
| **OpenCode** (FORGE Builder) | 2.5–5.0 | 1.5–8.0 | < 1.0 | Build is execution. Reading IS the verification. |
| **A-FORGE** (Hands) | 5.0+ | 3.0+ | < 2.0 | Pure execution. Minimal verification overhead. |
| **GEOX** (Earth) | 1.0–2.0 | 0.5–4.0 | < 0.3 | Compute-heavy. Long steps. Verification IS the work. |
| **WEALTH** (Capital) | 1.5–3.0 | 0.8–5.0 | < 0.5 | Analysis-heavy but must produce decisions. |
| **WELL** (Human) | 1.0–1.5 | 0.5–3.0 | < 0.3 | REFLECT_ONLY — naturally verification-heavy. |

### 4.2 Calibration Rule

```
alert_threshold = organ_target_min * 0.5
critical_threshold = organ_target_min * 0.25

Example (OpenCode):
  target_min = 2.5
  alert_threshold = 1.25  (WATCHING)
  critical_threshold = 0.625  (STUCK)
```

---

## Section 5: Alert Protocol

### 5.1 Alert Levels

| Level | Trigger | Display | Action |
|-------|---------|---------|--------|
| **INFO** | FQ crosses below target_min | Cockpit trend annotation | None — informational only |
| **WARNING** | FQ < alert_threshold | 🟠 Cockpit banner + Kabarkan span | Suggest FLAME reroute for eligible tasks |
| **CRITICAL** | FQ < critical_threshold | 🔴 Cockpit banner + Kabarkan span + 888 advisory | Auto-FLAME + 888 notified |
| **EMERGENCY** | FQ < critical_threshold for 3+ consecutive windows | ⚫ Cockpit banner + 888-HOLD | HARD intervention: session pause |

### 5.2 Escalation Path

```
FQ WARNING ──(3 windows)──▶ FQ CRITICAL ──(3 windows)──▶ FQ EMERGENCY
     │                            │                          │
     ▼                            ▼                          ▼
  Cockpit 🟠                   Cockpit 🔴                Cockpit ⚫
  Kabarkan span               Kabarkan span              Kabarkan span
  FLAME suggest               888 advisory              888-HOLD
                              Auto-FLAME                Session pause
```

### 5.3 De-escalation

```
FQ returns above threshold for 2 consecutive windows → alert cleared.
Cockpit banner removed. Kabarkan span emitted with resolution="auto_resolved".
```

---

## Section 6: Cooling Cross-Reference

### 6.1 The Correlation

```
FQ ↓  →  ΔS ↑  →  Drift severity ↑

Correlation coefficient: r ≈ −0.73 (strong negative)
```

When the agent stops executing, it starts drifting. An idle federation is a decaying federation.

### 6.2 Cross-Reference Matrix

| FQ | ΔS | Interpretation | Action |
|----|-----|---------------|--------|
| HIGH (> 3.0) | LOW (≤ 0) | Healthy. Executing, no drift. | None |
| HIGH (> 3.0) | HIGH (> 0) | Executing fast but drifting. Reckless. | Increase verification. F1 check. |
| LOW (< 1.0) | LOW (≤ 0) | Over-verifying but stable. Cautious but clean. | Monitor. May be appropriate for high-risk domain. |
| LOW (< 1.0) | HIGH (> 0) | **WORST CASE.** Paralyzed AND drifting. | Emergency. 888-HOLD. Session pause. |

### 6.3 Cooling Queue Trigger

```
IF FQ < critical_threshold AND ΔS > 0:
  → push to cooling queue with priority = CRITICAL
  → emit COOLING_DRIFT_ALERT span
  → flag for human review in AAA cockpit
```

---

## Section 7: Kabarkan Emission

### 7.1 Span Format

```json
{
  "span_id": "fq_step_042",
  "trace_id": "trace_seal_479dce2f",
  "timestamp": "2026-07-25T07:49:53Z",
  "event_type": "AfqSnapshot",
  "payload": {
    "step": 42,
    "organ": "opencode",
    "execute_count": 73,
    "verify_count": 27,
    "execute_cost_ns": 730000000,
    "verify_cost_ns": 270000000,
    "fq": 2.7037,
    "verdict": "BALANCED",
    "window_size": 20,
    "alert": false,
    "alert_level": null
  },
  "tags": {
    "topology": "fan_out",
    "lease_id": "LCL-arif-ms02jfa9-35ulme",
    "actor_id": "arif",
    "session_id": "SEAL-479dce2f559447e8"
  }
}
```

### 7.2 Emission Frequency

| Condition | Frequency |
|-----------|-----------|
| Normal (FQ > threshold) | Every super-step |
| WATCHING (FQ < 1.0) | Every super-step (unchanged — alert is in payload, not frequency) |
| STUCK (FQ < 0.5) | Every super-step + alert span |
| PARALYZED (FQ < 0.25) | Every super-step + alert span + HOLD advisory span |

### 7.3 AAA Cockpit Display

```
┌────────────────────────────────────────────────────────────┐
│  FLOW QUOTIENT — OpenCode (target: 2.5–5.0)               │
│                                                            │
│  Current: 2.70  🟡 BALANCED                                │
│  Window:  20 steps    E=15  V=7                            │
│                                                            │
│  Trend ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁  (stable — no alert)              │
│                                                            │
│  Session FQ avg: 2.47    Session FQ min: 1.82              │
│  Alerts this session: 0                                    │
│  Last cooling ΔS: −0.03 (CONVERGED)                        │
└────────────────────────────────────────────────────────────┘
```

---

## Section 8: Governance Feedback

### 8.1 FQ as Input to 888-JUDGE

FQ is an advisory input to 888-JUDGE, not a binding verdict. The judge may consider FQ when:

| Judge Decision | FQ Input |
|----------------|----------|
| **Grant SEAL** | FQ > 1.0 — agent is executing more than verifying. Trust is earned. |
| **Issue HOLD** | FQ < 0.5 for 3+ windows — agent is stuck. Governor should intervene. |
| **Adjust autonomy** | FQ trend declining → temporarily reduce autonomy tier (T1 → T2). FQ recovering → restore. |
| **Route to FLAME** | FQ < 1.0 → eligible tasks (classification, extraction, fact_check) auto-routed to FLAME free loop. |

### 8.2 Autonomy Adjustment

```
FQ_STABLE (no alert for 10+ windows):
  → autonomy tier = declared tier (T1/T2/T3)

FQ_DECLINING (FQ dropping across 5+ windows):
  → autonomy tier = max(declared_tier - 1, T2)
  → "System is verifying more. Temporarily reduced autonomy."
  → Kabarkan span: AUTONOMY_ADJUSTED

FQ_RECOVERING (FQ rising across 3+ windows after decline):
  → autonomy tier = declared tier
  → "System recovered. Autonomy restored."
  → Kabarkan span: AUTONOMY_RESTORED
```

---

## Section 9: mPFC Takeover Detection

### 9.1 Definition

**mPFC Takeover** = verification cost exceeds execution cost for 3+ consecutive windows AND the ratio of verify:execute steps is increasing.

### 9.2 Detection Algorithm

```
fn detect_mpfc_takeover(history: &[FlowQuotient]) -> bool {
    if history.len() < 3 {
        return false;  // insufficient data
    }

    let last_three = &history[history.len()-3..];

    // Condition 1: All three windows have FQ < 1.0
    let all_below_one = last_three.iter().all(|fq| fq.quotient < 1.0);

    // Condition 2: Ratio is DECLINING (getting worse)
    let declining = last_three[0].quotient > last_three[1].quotient
                 && last_three[1].quotient > last_three[2].quotient;

    all_below_one && declining
}
```

### 9.3 Response

```
mPFC_TAKEOVER_DETECTED:
  → emit KABARKAN_MPFC_TAKEOVER span
  → route ALL eligible tasks to FLAME
  → reduce autonomy tier to T2 minimum
  → notify 888-JUDGE
  → cockpit shows ⚫ "RUMINATE DETECTED" banner
  → Telegram digest: "⚠️ OpenCode sedang over-verify. FQ=0.38. FLAME diaktifkan."
```

---

## Section 10: Implementation Path

### 10.1 Status

| Layer | Component | Status | Location |
|-------|-----------|--------|----------|
| **Rust** | FlowQuotient struct | ✅ DONE | `src/receipt.rs:237` |
| **Rust** | FlowVerdict enum (5 levels) | ✅ DONE | `src/receipt.rs:211` |
| **Rust** | FQ computation (`compute()`) | ✅ DONE | `src/receipt.rs:254` |
| **Rust** | FQ in scheduler (`flow_quotient()`) | ✅ DONE | `src/scheduler.rs:459` |
| **Rust** | AfqSnapshot span emission | ✅ DONE | `src/governance/kabarkan.rs:50` |
| **Rust** | mPFC takeover detection | 🔲 P2 | `src/receipt.rs` — `detect_mpfc_takeover()` |
| **Rust** | Per-organ window config | 🔲 P2 | `src/receipt.rs` — `FqWindowConfig` |
| **Kabarkan** | Postgres ingestion (`fq_snapshots`) | 🔲 P2 | Kabarkan pipeline |
| **AAA** | API endpoints (`/api/kabarkan/fq`) | 🔲 P2 | AAA cockpit backend |
| **AAA** | FQMonitor React component | 🔲 P2 | AAA cockpit frontend |
| **AAA** | WebSocket real-time stream | 🔲 P2 | AAA WebSocket gateway |
| **AAA** | FQ vs Cooling scatter plot | 🔲 P2 | AAA cockpit analytics |
| **Telegram** | FQ digest (morning/alert) | 🔲 P2 | Hermes → Telegram bridge |

### 10.2 P2 Tasks (next forge session)

1. `detect_mpfc_takeover()` — Rust function in `receipt.rs`
2. `FqWindowConfig` — per-organ configuration struct
3. Postgres migration — `kabarkan.fq_snapshots` table
4. AAA `/api/kabarkan/fq/:session_id` endpoint
5. AAA `FQMonitor.tsx` component
6. WebSocket stream: `ws://aaa:3001/ws/kabarkan/fq`
7. Telegram digest: `hermes_fq_report(organ, fq, verdict)`

---

## Section 11: Contract Summary

This spec is a **binding contract** between the following organs:

| Organ | Obligation |
|-------|-----------|
| **arifFlow** | Compute FQ every super-step. Emit AfqSnapshot span. Detect mPFC takeover. |
| **Kabarkan** | Ingest spans. Store in Postgres. Provide query API. |
| **AAA cockpit** | Display FQ trend. Show alerts. Render cooling cross-reference. |
| **Telegram (Hermes)** | Digest FQ state on request. Alert on STUCK/PARALYZED. |
| **888-JUDGE** | Consider FQ in verdict. Adjust autonomy when FQ declining. |
| **All agents** | Self-monitor FQ. Accept FLAME reroute when WATCHING. Halt on PARALYZED. |

**Breach:** Any organ failing to meet its obligation → FQ for that organ is UNMEASURED → Kabarkan shows `FQ: —` in cockpit → log to cooling ledger as `FQ_CONTRACT_BREACH`.

---

*Forged 2026.07.25. DITEMPA BUKAN DIBERI.*
*FQ is the physical measure of cognitive balance.*
*One number. Federation-wide. Answers: "Adakah sistem sihat?"*
