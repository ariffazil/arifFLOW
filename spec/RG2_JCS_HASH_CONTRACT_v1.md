# RG2 JCS Hash Contract v1 — cross-language canonical receipt hashing

schema: arifflow.spec/jcs-hash-contract/v1
status: IMPLEMENTED (P4-JCS landed 2026-09-13 — see Implementation section at end)
work_order: RG-BLUEPRINT-INIT-TO-SEAL-v1 → RG-2-FIX-002 → P4-JCS
authored: 2026-09-12, fi-003-qwen (session SEAL-74f9a613aa6f47cc)
implemented: 2026-09-13, fi-003-qwen (F13 directive "sambung P4-JCS"; assignment moved FI-009 → FI-003)

## ⚠ SCOPE OF PROTECTION — READ FIRST

`FlowReceipt::hash()` is **UNCHANGED** by P4-JCS. The live seal chain
(`arifflow_sealed.jsonl`, `previous_receipt_hash`, seal checkpoints) still uses
the pre-JCS serializations and receives **zero protection from this work**
until a separately-gated migration lands (F13 decision required). JCS exists
**beside** the chain, not under it. "Canonicalization shipped" ≠ "the chain is
canonical." Anyone reading "JCS landed" must NOT infer the seal chain gained
cross-language verifiability — the three-names interim rule remains in force.

## Problem (disk-witnessed)

Receipt hashing is currently DIVERGENT BY DESIGN-ACCIDENT across the three language
surfaces:

| Surface | Algorithm | Canonicalization |
|---|---|---|
| Rust `FlowReceipt::hash()` (receipt.rs) | SHA3-256 | `serde_json::to_string` — struct field order |
| Python `receipt/__init__.py` | SHA-256 | `json.dumps(sort_keys=True)` |
| TypeScript `engine.ts` | SHA-256 | sorted stringify (own impl) |
| Bridge envelope | SHA-256 | envelope serialization |

Consequence: `client_payload_hash ≠ rust_receipt_hash ≠ seal_checkpoint_hash`. The
blueprint's three-names interim rule applies — the hashes are NOT comparable values
until golden fixtures prove parity.

## Contract (RFC 8785 — JCS)

Canonical receipt encoding: `arifflow-jcs-v1`

- standard: RFC 8785 (JSON Canonicalization Scheme)
- encoding: UTF-8, no BOM, no whitespace
- json profile: I-JSON (RFC 7493) — no NaN/Infinity, no duplicate keys
- object key order: JCS (UTF-16 code-unit sort)
- strings: ECMAScript minimal escaping
- numbers: ECMAScript serialization (IEEE 754 double, shortest round-trip)
- optional fields: omit when absent; explicit null only when semantic
- array order: preserved
- hash algorithm: SHA3-256 (FIPS 202), hex-encoded

## Schema-level number discipline (hard prerequisite)

Cross-language parity is unachievable by serializer discipline alone — the SCHEMA must
constrain numbers so all three runtimes agree bit-for-bit:

1. Timestamps: RFC 3339 strings — never numeric epochs.
2. Integers: bounded to the I-JSON safe range (≤ 2^53). Counters u32-bounded.
3. No arbitrary-precision numerics in canonicalized fields (Python int is unbounded;
   Rust u64 exceeds double; JS Number IS double — divergence is structural otherwise).
4. Floats: avoid where possible; when required, values must be exactly representable
   as IEEE 754 doubles.

## Interim rule (until P4-JCS lands)

The three names stay distinct and are never compared:
`client_payload_hash`, `rust_receipt_hash`, `seal_checkpoint_hash`.
Binding verification happens INSIDE one runtime only (Rust: seal checkpoint vs
receipt body hash — `verify_receipt_seal_binding`).

## Golden fixtures (P4-JCS acceptance)

Minimum fixture set, each materialized in Rust/TS/Python with identical canonical
bytes + identical SHA3-256:

1. minimal root receipt
2. single-parent receipt
3. multi-parent receipt
4. unicode receipt (CJK + emoji + combining marks)
5. absent optional fields
6. explicit null where semantically allowed
7. ordered arrays (order preservation)
8. high-precision RFC3339 timestamp string
9. negative + large (≤2^53) integer edge cases
10. classification-redacted receipt (opaque fields)

Acceptance: `cross_language_canonical_bytes_match` AND `cross_language_sha3_256_matches`
for every fixture, in CI, on all three runtimes.

## Scope boundary

This spec does NOT implement JCS. Implementation (Rust JCS serializer, TS
json-canon, Python jcs) + fixtures + CI wiring = forge order **P4-JCS**, to be
scheduled after RG-2 core verification completes.

## Implementation record (P4-JCS, 2026-09-13)

Landed as three dependency-free canonicalizers + one golden fixture file:

| Surface | Module | Notes |
|---|---|---|
| Rust | `src/jcs.rs` | manual escaper; UTF-16 key cmp via `encode_utf16`; ECMAScript f64 formatting over `{:e}` shortest digits (property unit-tested with 10k round-trip sweep); safe-integer guard |
| Python | `src/py/arifflow/jcs.py` | stdlib only; `Decimal(repr(x))` for shortest digits (`{:e}` is 6-digit — insufficient); UTF-16 key sort via `utf-16-be` bytes; safe-integer + lone-surrogate guards |
| TypeScript | `src/ts/arifflow/jcs.ts` | REFERENCE (ECMAScript semantics native); lone-surrogate detection must scan `\udXXX` escape sequences (ES2019 well-formed stringify emits them escaped, not raw) |
| Fixtures | `tests/fixtures/jcs_golden_v1.json` | 10 fixtures + 9 float edges; expected bytes/hashes computed by the Node reference; regen: `node --experimental-strip-types tools/gen_jcs_fixtures.mjs` |

Conformance: Rust 6/6 (suite 198/0), Python 6/6, TS 5/5 — identical canonical
bytes AND identical SHA3-256 across all three runtimes.

Divergence traps caught during the forge (each now guarded):

1. **UTF-16 vs code-point key order** — astral keys (U+10000) sort BEFORE
   BMP-high keys (U+FFFD) under UTF-16; the opposite under code points.
   Fixture #4 proves the flip in canonical bytes.
2. **Integer-looking literals > 2^53** — JS `JSON.parse` silently rounds
   them to doubles; Python parses arbitrary-precision ints. The canonicalizers
   REJECT ints outside the safe range instead of diverging silently.
   (The fixture generator itself was bitten: `cost_ns: 1.789e18` was rounded
   by Node before the guard existed.)
3. **ES2019 well-formed stringify** — lone surrogates surface as `\udXXX`
   escapes, not raw units; TS detection scans escape sequences.
4. **Python `{:e}` 6-digit default** — not shortest round-trip; `Decimal(repr())`
   is.

`FlowReceipt::hash()` is UNCHANGED — the live seal chain keeps its existing
serialization. Migration of receipt hashing onto `arifflow-jcs-v1` is a
separate, explicitly-gated decision (three-names interim rule remains in
force until then).
