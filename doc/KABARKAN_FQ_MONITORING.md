# Kabarkan FQ Monitoring — Real-Time Flow Quotient Dashboard

> **DITEMPA BUKAN DIBERI** — You cannot govern what you cannot see.
>
> **Canon:** arifFlow architecture · 2026.07.25
> **Parent:** [FLOW_RECEIPT_v1.md](FLOW_RECEIPT_v1.md) (Flow Quotient metric)
> **Rust source:** `src/receipt.rs` (FlowQuotient struct) · `src/governance/kabarkan.rs` (AfqSnapshot span)
> **Epistemic label:** SPEC (Kabarkan dashboard specification)

---

## Preamble

FQ is the core metric of agentic flow. But a metric without a dashboard is a thermometer in a drawer.

Kabarkan ingests `AfqSnapshot` spans from arifFlow and renders them as a real-time health dashboard in AAA cockpit. This document defines the alert thresholds, the display schema, and the correlation analysis that turns raw FQ numbers into actionable governance signals.

---

## Section 1: Alert Thresholds

```
FQ > 3.0    🟢 OPTIMAL   — Agent in flow. Governance lives in the architecture.
FQ 1.0–3.0  🟡 BALANCED  — Healthy. Verification supports execution.
FQ 0.5–1.0  🟠 WATCHING  — Agent spends as much time verifying as executing.
                            Alert: "Verification cost approaching execution cost."
FQ < 0.5    🔴 STUCK     — Self-monitoring has become the task. mPFC takeover.
                            Alert: "Ruminate detected. Route to FLAME."
FQ < 0.25   ⚫ PARALYZED  — Severe analysis paralysis. 2 exec, 8 verify.
                            Alert: "HARD intervention required."
```

| Threshold | Verdict | Cockpit Color | Action |
|-----------|---------|---------------|--------|
| `> 3.0` | OPTIMAL | Green pulse | None |
| `1.0–3.0` | BALANCED | Yellow | Log for trend analysis |
| `0.5–1.0` | WATCHING | Orange | Alert + suggest FLAME reroute |
| `< 0.5` | STUCK | Red blink | 888-HOLD advisory + auto-FLAME |
| `< 0.25` | PARALYZED | Red solid | HARD intervention required |

---

## Section 2: AAA Cockpit Display Schema

### 2.1 FQ Trend Panel (Primary)

```
┌─────────────────────────────────────────────────────────┐
│  FLOW QUOTIENT  —  sliding window (last 100 steps)      │
│                                                         │
│  FQ: 2.47  🟢 OPTIMAL                                   │
│                                                         │
│  ████████████████████████████████░░░░░░░░░░░░░░░░░░░░   │
│  E=73  V=27  ▏E/V ratio: 2.70                          │
│                                                         │
│  ┌─ Trend (last 20 super-steps) ────────────────────┐  │
│  │  3.2 ████████                                     │  │
│  │  2.8 ███████                                      │  │
│  │  2.5 ██████                                       │  │
│  │  2.1 █████                                        │  │
│  │  1.8 ████                                         │  │
│  │  ... downward trend detected ⚠                    │  │
│  └───────────────────────────────────────────────────┘  │
│                                                         │
│  Last alert: — (no alerts in this window)               │
└─────────────────────────────────────────────────────────┘
```

### 2.2 FQ Alert Panel

```
┌─────────────────────────────────────────────────────────┐
│  FQ ALERTS                                              │
│                                                         │
│  🟠 2026-07-25T07:32:00  FQ=0.87  WATCHING             │
│     Verifying as much as executing.                     │
│     Suggested: route fact_checks to FLAME free loop.    │
│                                                         │
│  🔴 2026-07-25T08:15:00  FQ=0.42  STUCK                │
│     mPFC takeover. Self-monitoring is the task.         │
│     HOLD advisory triggered. FLAME reroute active.      │
└─────────────────────────────────────────────────────────┘
```

### 2.3 Cooling ↔ FQ Cross-Reference

```
┌─────────────────────────────────────────────────────────┐
│  CORRELATION: FQ vs COOLING DRIFT                       │
│                                                         │
│  ┌─ Scatter ───────────────────────────────────────┐   │
│  │  FQ ↑                                            │   │
│  │  3.0 │  ·  ·  ·                                 │   │
│  │  2.0 │     ·  ·  ·   ·                          │   │
│  │  1.0 │        ·  ·  · ·  ·                      │   │
│  │  0.5 │           · · ··  ·   ← DRIFT DANGER     │   │
│  │  0.2 │              ···                          │   │
│  │      └──────────────────────────────────→ ΔS     │   │
│  │        0    0.1   0.2   0.3   0.5   0.8         │   │
│  └──────────────────────────────────────────────────┘   │
│                                                         │
│  Correlation: r = −0.73 (strong negative)               │
│  As FQ drops, cooling drift rises.                      │
│  The agent that stops executing starts drifting.        │
└─────────────────────────────────────────────────────────┘
```

---

## Section 3: Kabarkan Span Schema

### 3.1 AfqSnapshot Span (emitted by arifFlow)

```json
{
  "span_id": "afq_step_042",
  "trace_id": "trace_seal_46a4c7b0",
  "timestamp": "2026-07-25T07:45:00Z",
  "event_type": "AfqSnapshot",
  "payload": {
    "step": 42,
    "execution_steps": 73,
    "governance_steps": 27,
    "afq": 2.47,
    "diagnosis": "OPTIMAL"
  },
  "tags": {
    "topology": "fan_out",
    "lease_id": "LCL-arif-ms02dkl5-rvibuc",
    "actor_id": "arif",
    "session_id": "SEAL-46a4c7b0e6b74734"
  }
}
```

### 3.2 Kabarkan Ingestion → Postgres

```sql
CREATE TABLE kabarkan.fq_snapshots (
    id          BIGSERIAL PRIMARY KEY,
    span_id     TEXT NOT NULL,
    trace_id    TEXT NOT NULL,
    session_id  TEXT NOT NULL,
    step        BIGINT NOT NULL,
    execute_count BIGINT NOT NULL,
    verify_count  BIGINT NOT NULL,
    afq         DOUBLE PRECISION NOT NULL,
    diagnosis   TEXT NOT NULL,
    topology    TEXT,
    timestamp   TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_fq_session ON kabarkan.fq_snapshots(session_id, step);
CREATE INDEX idx_fq_diagnosis ON kabarkan.fq_snapshots(diagnosis);
```

---

## Section 4: Correlation Analysis — The Answer

> **When does verification cost exceed execution cost?**

The answer is in the cross-reference:

```
FQ ↓  →  ΔS ↑  →  Drift severity ↑
```

| Pattern | FQ | ΔS | Meaning |
|---------|-----|-----|---------|
| Normal execution | > 2.0 | ≤ 0 | Agent in flow. Minimal drift. |
| Heavy verification | 1.0–2.0 | ~0 | Verifying as needed. Acceptable. |
| Verification creep | 0.5–1.0 | 0.1–0.3 | Too much checking. Drift starting. |
| Ruminate spiral | 0.25–0.5 | 0.3–0.6 | Self-monitoring = the task. Drift accelerating. |
| Paralysis | < 0.25 | > 0.6 | Agent frozen. State degrading. |

**The correlation:** r = −0.73 (strong negative) — as FQ drops, cooling drift rises. The agent that stops executing starts drifting. An idle federation is a decaying federation.

**The threshold:** When FQ crosses below 1.0, the verification cost has matched execution cost. This is the inflection point. At FQ < 0.5, verification cost has DOUBLED execution cost — the agent is spending more time checking itself than doing work. This is when FLAME reroute MUST activate.

---

## Section 5: AAA Cockpit Integration

### 5.1 API Endpoint

```
GET /api/kabarkan/fq/{session_id}
  → { fq_current, fq_trend[], alerts[], correlation }

GET /api/kabarkan/fq/active
  → [{ session_id, fq, diagnosis, topology }]  — all active sessions

GET /api/kabarkan/fq/alerts?since=2026-07-25T00:00Z
  → [{ timestamp, session_id, fq, diagnosis, suggested_action }]
```

### 5.2 WebSocket Stream

```
ws://aaa:3001/ws/kabarkan/fq
  → stream of AfqSnapshot events as they arrive
  → cockpit updates in real-time without polling
```

### 5.3 Cockpit Component

```typescript
// AAA cockpit — FQMonitor.tsx
interface FQPanel {
  current: FlowQuotient;       // latest snapshot
  trend: FlowQuotient[];       // sliding window (last 100 steps)
  alerts: FQAlert[];           // threshold breaches
  correlation: FQCoolingCorrelation;  // FQ vs ΔS scatter
}

// Rendered as:
// <FQGauge value={fq} threshold={1.0} />
// <FQTrendChart data={trend} />
// <FQAlertList alerts={alerts} />
// <FQCoolingScatter correlation={correlation} />
```

---

## Section 6: Alert Actions

When an alert fires, Kabarkan triggers the corresponding action:

| Alert | Trigger | Action |
|-------|---------|--------|
| WATCHING | FQ < 1.0 | Log to cooling ledger. Suggest FLAME reroute for classification/extraction tasks. Cockpit shows 🟠 banner. |
| STUCK | FQ < 0.5 | 888-HOLD advisory. Automatic FLAME reroute for all eligible tasks. Cockpit shows 🔴 banner. Session continues but governor is notified. |
| PARALYZED | FQ < 0.25 | HARD intervention. Session pauses. All new tasks routed to FLAME. arifOS 888-JUDGE notified. Cockpit shows ⚫ banner with recovery options. |

---

## Section 7: Implementation Status

| Component | Status | Location |
|-----------|--------|----------|
| FlowQuotient struct | ✅ P1 | `src/receipt.rs:237` |
| FlowVerdict enum (4 levels) | ✅ P1 | `src/receipt.rs:211` |
| AfqSnapshot span emission | ✅ P1 | `src/governance/kabarkan.rs:50` |
| FQ in scheduler | ✅ P1 | `src/scheduler.rs:459` |
| FQ tests (20) | ✅ P1 | `src/receipt.rs:741` |
| Postgres schema | 🔲 P2 | Kabarkan ingestion pipeline |
| AAA API endpoints | 🔲 P2 | AAA cockpit backend |
| AAA FQMonitor component | 🔲 P2 | AAA cockpit frontend |
| WebSocket stream | 🔲 P2 | AAA WebSocket gateway |
| Correlation engine | 🔲 P2 | FQ vs cooling ledger cross-ref |

---

*Forged 2026.07.25. DITEMPA BUKAN DIBERI.*
*When the agent stops executing, it starts drifting. FQ is the pulse.*
*Kabarkan makes the invisible visible.*
