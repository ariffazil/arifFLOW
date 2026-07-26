# FLOW_OWNERSHIP_LEDGER — P1 Receipt Layer Extraction

> **DITEMPA BUKAN DIBERI**
> **Forged:** 2026-07-26 by OpenCode (333-AGI) under F13 authorization
> **Status:** CANONICAL — every moved function has one destination declared before extraction begins
> **Authority:** Arif (F13 SOVEREIGN) — GO signal given

## Purpose

Before any code extraction begins, this ledger declares EXACTLY which component owns what.
This eliminates interpretation during migration — each line is a binding contract.

## Receipt Ownership Transfer

| Component | Current Owner | Future Owner | Transfer Type |
|-----------|--------------|--------------|---------------|
| `mint_pai_receipt()` | arifOS | arifOS (KEEPS — canonical schema) | NONE — stays |
| `PAIReceipt` schema | arifOS | arifOS (KEEPS — canonical definition) | NONE — stays |
| `emitReceipt()` (AAA) | AAA/operation-bus.ts | arifFLOW | MIGRATE → becomes caller |
| `validateReceipt()` (A-FORGE) | A-FORGE/forge.ts | arifFLOW | MIGRATE → becomes caller |
| `ReceiptEvent` type (AAA) | AAA/operation-bus.ts | arifFLOW/receipt.rs | MIGRATE — schema unification |
| `ExecutorReceipt` type (A-FORGE) | A-FORGE/forge.ts | arifFLOW/receipt.rs | MIGRATE — schema unification |
| `CoolingReceipt` (A-FORGE) | A-FORGE/coolingVerbs.ts | arifFLOW/receipt.rs | MIGRATE |
| `APEXRuntimeReceipt` (A-FORGE) | A-FORGE | arifFLOW/receipt.rs | MIGRATE |
| MCP tool call audit (arifOS) | arifOS MCP runtime | arifFLOW | MIGRATE → caller |
| Observability receipts (all 3) | arifOS/AAA/A-FORGE | arifFLOW | MIGRATE → caller |
| VAULT999 seal receipts (arifOS) | arifOS | arifOS (KEEPS — seal is kernel) | NONE — stays |
| Receipt query/read | AAA/operation-bus.ts | arifFLOW | MIGRATE |
| Receipt storage | Scattered (files/DB) | arifFLOW (via /receipt endpoint) | MIGRATE |

## Non-Receipt Ownership (Declared for Clarity)

| Component | Owner | Reason |
|-----------|-------|--------|
| Verdict generation | arifOS | Constitutional — 888_JUDGE |
| Execution | A-FORGE | Physical mutation |
| Session deliberation | AAA | State/routing plane |
| Session continuity | arifFLOW (P2) | State spine |
| Job queue | arifFLOW (P4) | Orchestration ownership |
| Intent routing | arifFLOW (P3) | Router engine |
| Agent schema definitions | web-canon | Canonical schema owner |
| Human narratives | arif-fazil.com | Human surface |

## Post-P1 Architecture

```
BEFORE P1 (Today):
  AAA ──emitReceipt()──▶ receipt log
  A-FORGE ──validateReceipt()──▶ receipt log
  arifOS ──mint_pai_receipt()──▶ VAULT999

AFTER P1 (Target):
  AAA ──POST /receipt──▶ arifFLOW ──emit()──▶ unified receipt store
  A-FORGE ──POST /receipt──▶ arifFLOW ──emit()──▶ unified receipt store
  arifOS ──POST /receipt (via bridge)──▶ arifFLOW ──emit()──▶ unified receipt store
  
  arifFLOW ──emit()──▶
    ├── ReceiptStore (Rust in-memory + persistent)
    ├── Merkle anchor (every N receipts)
    ├── FQ update (execute vs verify ratio)
    └── VAULT999 (T4+ receipts via arif_seal bridge)
```

## Migration Sequence

1. **Add `POST /receipt` endpoint to arifFLOW daemon** (this commit)
2. **Build `arifflow-client` Python library** (arifOS bridge)
3. **Build `@arifflow/client` TypeScript library** (AAA/A-FORGE bridge)
4. **Wire AAA `emitReceipt()` → `POST /receipt`** (proof of concept)
5. **Wire A-FORGE `validateReceipt()` → `POST /receipt`**
6. **Wire arifOS MCP audit → `POST /receipt`**
7. **Deprecate independent receipt generation** (flag old functions, don't delete)

## Reversibility

Every step is reversible:
- Old receipt functions are DEPRECATED (flag + warning), not deleted
- arifFLOW receipt store is ADDITIVE only (new receipts, old logs untouched)
- `POST /receipt` endpoint is ADDED (new feature, existing surfaces unchanged)
- Rollback: stop calling arifFLOW, resume calling old receipt functions

## Verification

- [ ] `POST /receipt` returns 200 with receipt_id
- [ ] Receipt is stored in arifFLOW ReceiptStore
- [ ] Merkle anchor is updated
- [ ] FQ is recomputed after receipt
- [ ] Old receipt function still works (deprecation flag only)
- [ ] No session disruption during migration
