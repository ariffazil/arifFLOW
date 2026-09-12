# RG2 JCS Hash Contract v1 — cross-language canonical receipt hashing

schema: arifflow.spec/jcs-hash-contract/v1
status: CANDIDATE SPEC (OL-003 resolution path — implementation deferred to forge P4-JCS)
work_order: RG-BLUEPRINT-INIT-TO-SEAL-v1 → RG-2-FIX-002
authored: 2026-09-12, fi-003-qwen (session SEAL-74f9a613aa6f47cc)

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
