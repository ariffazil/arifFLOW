# ROLLBACK.md — RG-BLUEPRINT-INIT-TO-SEAL-v1
# Generated: 2026-09-12T10:39:00Z

## P2 — Fold-1 EscalationTarget fix (arifFlow/src/topology/controlled_cycle.rs)

This is the only production code change in this forge round.
All other changes are evidence package files (new, not modifying existing code).

### Rollback command

```bash
# From arifFlow repo root, after P2 is committed:
git revert <P2-commit-hash>

# This reverts:
# - EscalationTarget: Judge888 + SovereignF13 back to Sovereign888
# - for_exit() mappings back to original
# - requires_888_hold() back to Sovereign888
# - removes requires_sovereign_f13() method
# - test_judge888_cannot_ratify_f13 removed
# - display strings back to SOVEREIGN_888
```

### Side effects of rollback

- Fold-1 constitutional violation reintroduced
- test_judge888_cannot_ratify_f13 removed from test suite
- Any code that pattern-matches on Judge888/SovereignF13 must update to Sovereign888
- Any code that calls requires_sovereign_f13() will not compile

### Prior to rollback, verify

- No production receipts reference Judge888 or SovereignF13 as escalation_target
- No downstream consumers (arifOS, A-FORGE, AAA) have serialized these enum values
- Since P2 is not deployed, no VAULT999 entries reference the new variants

### Rollback decision authority

- Operator: may initiate rollback if P2 causes compilation failures
- JUDGE_888: may hold P2 pending further review
- SOVEREIGN_F13: not required for this rollback (P2 is reversible, no irreversible state)

## P1 — lineage.rs commit (pending)

If P1 commit is reverted:
```bash
git revert <P1-commit-hash>
# Removes lineage.rs from index
# Reverts mod.rs to pre-lineage state
# arifFlow compiles without RG-2 module
```

Side effects: RG-2 capability returns to CODE_ABSENT state.

## Evidence package files

Evidence package files (INIT.yaml, OBSERVATIONS.md, etc.) are in
release-candidate/RG-BLUEPRINT-INIT-TO-SEAL-v1/ — they are documentation only,
not production code. They may be removed without affecting runtime.
