# FEDERATION_MAP.md — arifFlow

```yaml
layer: L1
role: GOVERNANCE
function: Coordination
status: ACTIVE
canon: (internal — no public surface)

identity:
  repository: ariffazil/arifFlow
  organ: arifFlow Coordination Fabric
  floor_range: F1–F13 (via arifOS)

function: |
  arifFlow is the parallel execution and workflow coordination fabric
  of the arifOS Federation. It owns: task routing, parallel orchestration,
  dependency resolution, and workflow state management.

  arifFlow coordinates. It does NOT execute directly.
  Execution is A-FORGE's domain. Flow ensures the right thing
  happens in the right order with the right authority.

upstream:
  - ariffazil/arifos       # L0 — constitutional kernel

peers:
  - ariffazil/AAA          # L1 — control plane
  - ariffazil/APEX         # L1 — judgment engine
  - ariffazil/A-FORGE      # L1 — execution shell

downstream:
  - ariffazil/geox         # L2 — earth intelligence
  - ariffazil/wealth       # L2 — capital intelligence
  - ariffazil/well         # L2 — human readiness
  - ariffazil/HERMES       # L2 — multi-modal bridge
```

**DITEMPA BUKAN DIBERI — Forged, Not Given.**
