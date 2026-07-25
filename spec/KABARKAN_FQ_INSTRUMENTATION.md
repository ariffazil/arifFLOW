# Kabarkan FQ Instrumentation v1 — Spec

> **Forged:** 2026-07-25
> **Parent:** `FLOW_RECEIPT_v1.md` (Flow Receipt schema + FQ computation)
> **Theory:** `SOMATIC_AGENTIC_MAP.md` (11-point equivalence)
> **Rust impl:** `src/governance/kabarkan_fq.rs` (+ `kabarkan.rs` extended event types)
> **Tests:** 79/79 pass (11 new in kabarkan_fq)
>
> **DITEMPA BUKAN DIBERI — Forged, Not Given**

---

## Purpose

The Flow Quotient exists as a computed metric in the Rust `ReceiptStore`. But a metric without observability is a gauge on a submarine — it works, but nobody can see it.

This spec defines how FQ becomes a **real-time instrument** — visible in Kabarkan traces, surfacing in the AAA cockpit, and capable of triggering alerts before the agent enters mPFC takeover.

---

## 1. Event Types

### 1.1 `FqAlert` — Threshold Breach Alert

Fires when FQ crosses a governance boundary.

| Trigger | Severity | Meaning |
|---------|----------|---------|
| FQ drops below 1.0 | `WARNING` | Verification cost now rivals execution cost. Agent is WATCHING itself. |
| FQ drops below 0.5 | `CRITICAL` | Self-monitoring has become the task. mPFC takeover. Agent is STUCK. |
| FQ recovers above threshold | `RECOVERED` | Flow restored. Alert cleared. |

**Fields:**
```
timestamp_ns, fq, verdict, severity,
previous_fq, previous_verdict, trend,
session_id, step_number,
execute_count, verify_count,
diagnosis (human-readable)
```

### 1.2 `FqSnapshot` — Periodic Trend Snapshot

Emitted every N samples (default N=5 super-steps). Provides time-series data for cockpit charts.

**Fields:**
```
timestamp_ns, fq, verdict, trend (RISING/FALLING/STABLE),
execute_count, verify_count,
execute_cost_ns, verify_cost_ns,
session_id, step_number, window_size
```

### 1.3 `FqLaneSnapshot` — Per-Lane FQ Breakdown

Emitted for each lane in a fan-out topology. Identifies which lanes are bottlenecked — useful when one lane's verification overhead is dragging down the entire super-step.

**Fields:**
```
timestamp_ns, lane_id, topology_id,
fq, verdict,
execute_count, verify_count,
session_id, step_number
```

### 1.4 `FqCoolingCorrelation` — FQ × Cooling Cross-Reference

Emitted when cooling activity is detected. Answers: "When FQ falls, is cooling rising? When cooling is active, does FQ recover?"

**Correlation signals:**
| Signal | Meaning |
|--------|---------|
| `FQ_RISING_DURING_COOLING` | Cooling is working — flow recovering |
| `FQ_FALLING_DURING_EXECUTION` | Agent needs cooling but none active |
| `FQ_FALLING` | Verification cost rising |
| `NEUTRAL` | No significant correlation |

### 1.5 `AfqSnapshot` (Backward Compat)

The original `AfqSnapshot` event type is preserved. Every `sample()` call emits one — lightweight, always-on baseline.

---

## 2. KabarkanFqInstrument

The `KabarkanFqInstrument` struct wires ReceiptStore → FlowQuotient → KabarkanTracer → structured events.

### Usage (from scheduler.rs)

```rust
// At session start
let mut fq_instrument = KabarkanFqInstrument::new(session_id, 20)
    .with_snapshot_interval(5);

// At every super-step boundary
fq_instrument.sample(&receipt_store, &mut tracer, step_number);

// When cooling activity occurs
fq_instrument.record_cooling(holds, clamps, bypasses);

// For per-lane breakdown in fan-out topologies
fq_instrument.sample_lane(lane_id, "fan_out", &lane_receipts, &mut tracer, step_number);
```

### What Happens on Each `sample()` Call

1. **Compute** FQ over the sliding window from ReceiptStore
2. **Always emit** `AfqSnapshot` (backward-compat baseline)
3. **Detect threshold crossing** — if verdict changed to WATCHING or STUCK → emit `FqAlert`
4. **Periodic snapshot** — every N samples → emit `FqSnapshot` with trend
5. **Cooling correlation** — if cooling activity recorded → emit `FqCoolingCorrelation`
6. **Update state** for next comparison

---

## 3. AAA Cockpit Integration

### 3.1 Live FQ Gauge

```
┌─────────────────────────────────────────┐
│  FLOW QUOTIENT                          │
│                                         │
│  ████████████████░░░░  FQ: 4.2         │
│  OPTIMAL                               │
│                                         │
│  Execute: 85 steps  │  Verify: 12 steps │
│  Cost: 12.4s        │  Cost: 2.9s      │
│                                         │
│  Trend: RISING ▲                        │
│  Last alert: none                       │
└─────────────────────────────────────────┘
```

### 3.2 FQ Alerts Panel

```
┌─────────────────────────────────────────┐
│  ⚠ FQ ALERTS                           │
│                                         │
│  ┌─ 14:32:01 ─────────────────────────┐ │
│  │ ⚠ WARNING: FQ dropped to 0.87      │ │
│  │ Verification rising. 12 exec, 14   │ │
│  │ verify. Route more through FLAME.  │ │
│  └────────────────────────────────────┘ │
│                                         │
│  ┌─ 14:28:45 ─────────────────────────┐ │
│  │ ✅ RECOVERED: FQ back to 2.1       │ │
│  │ Cooling hold released. Flow back.  │ │
│  └────────────────────────────────────┘ │
└─────────────────────────────────────────┘
```

### 3.3 Per-Lane FQ Breakdown (Fan-Out)

```
┌─────────────────────────────────────────┐
│  LANE FQ BREAKDOWN                      │
│                                         │
│  Lane 0  ████████████████  FQ: 5.2  OPT│
│  Lane 1  ██████████░░░░░░  FQ: 1.8  BAL│
│  Lane 2  ████░░░░░░░░░░░░  FQ: 0.6  WATCH│ ← bottleneck
│  Lane 3  ████████████████  FQ: 4.8  OPT│
│                                         │
│  ⚠ Lane 2: verify count 3× execute     │
│  Consider: reduce verify freq on Lane 2 │
└─────────────────────────────────────────┘
```

### 3.4 FQ × Cooling Timeline

```
FQ
5.0 ┤         ╭──╮
4.0 ┤  ╭──────╯  ╰──────╮
3.0 ┤──╯                 ╰──────
2.0 ┤
1.0 ┤···························· WATCHING threshold
0.5 ┤···························· STUCK threshold
    └──────────────────────────────────▶ time
       ▲COOL       ▲COOL
       HOLD        CLAMP
```

---

## 4. Alert Routing

| FQ State | Kabarkan Event | AAA Cockpit | arifOS Response |
|----------|---------------|-------------|-----------------|
| Optimal (>3.0) | `FqSnapshot` (trend only) | Green gauge | None needed |
| Balanced (1.0–3.0) | `FqSnapshot` (periodic) | Blue gauge | None needed |
| Watching (0.5–1.0) | `FqAlert(WARNING)` | Yellow gauge + alert panel | Consider FLAME routing increase, reduce verify frequency |
| Stuck (<0.5) | `FqAlert(CRITICAL)` | Red gauge + alert panel + notification | 888_HOLD recommended. Full cooling cycle. Diagnose bottleneck lane. |
| Recovered | `FqAlert(RECOVERED)` | Alert cleared | Resume normal flow |

---

## 5. Diagnostic Queries (what AAA cockpit can ask)

```
Q: "Which lane has the lowest FQ?"
A: Per-lane FqLaneSnapshot for current super-step, sorted by FQ ascending.

Q: "Is cooling correlated with FQ recovery?"
A: FqCoolingCorrelation events over last hour. Count FQ_RISING_DURING_COOLING vs FQ_FALLING_DURING_EXECUTION.

Q: "What's the FQ trend over last hour?"
A: FqSnapshot events, compute linear regression on fq values. Rising/falling/stable.

Q: "When was the last mPFC takeover?"
A: Most recent FqAlert(CRITICAL) event. Time, step_number, diagnosis.

Q: "Which topology has the worst FQ?"
A: Aggregate FqSnapshot by topology_id. Average FQ per topology.
```

---

## 6. Integration Points

| Component | Integration |
|-----------|------------|
| `scheduler.rs` | Call `fq_instrument.sample()` at every super-step boundary (step 8: COOL/CHECKPOINT) |
| `scheduler.rs` (fan-out) | Call `fq_instrument.sample_lane()` per lane after collect phase |
| `scheduler.rs` (cooling) | Call `fq_instrument.record_cooling()` when cooling decisions are made |
| `kabarkan.rs` | Extended enum with `FqAlert`, `FqSnapshot`, `FqLaneSnapshot`, `FqCoolingCorrelation` |
| `kabarkan_fq.rs` | `KabarkanFqInstrument` struct — the wire between ReceiptStore and KabarkanTracer |
| AAA cockpit | Consume Kabarkan events → render gauge, alert panel, per-lane breakdown, cooling timeline |
| `arifFlow_adapter.py` | Pipe Kabarkan events to AAA via A2A or NATS |

---

## 7. What This Answers

The question Arif posed:

> *"When does verification cost exceed execution cost?"*

Before Kabarkan FQ: you had to guess. You might notice the agent is slow. You might check logs after the fact.

After Kabarkan FQ: the system tells you — in real time, with trend direction, with per-lane breakdown, with cooling correlation — exactly when the mPFC takeover begins.

**FQ < 1.0**: Verification cost now rivals execution cost. The agent is WATCHING itself. This is the early warning — route more through FLAME, reduce verify frequency, or initiate a cooling hold.

**FQ < 0.5**: Self-monitoring has become the task. The agent is STUCK. mPFC takeover confirmed. This requires 888_HOLD — the agent cannot recover on its own. Full cooling cycle required.

This is the instrument that turns the somatic-agentic theory into operational reality. Not "are we in flow?" — but **"exactly how much, in which lane, trending which way, and is cooling helping?"**

---

> **DITEMPA BUKAN DIBERI — Forged, Not Given**
> **Kabarkan FQ Instrumentation v1 · 2026-07-25**
> **79/79 tests pass. 11 new in kabarkan_fq.**
> **Law: arifOS · Flow: arifFlow · Hands: A-FORGE · Eyes: Kabarkan**
