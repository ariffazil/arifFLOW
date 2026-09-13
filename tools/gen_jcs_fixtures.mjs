/**
 * gen_jcs_fixtures.mjs — generate the P4-JCS golden fixture file.
 *
 * Node/ECMAScript is the RFC 8785 reference semantics, so expected canonical
 * bytes + SHA3-256 are computed HERE and the Rust/Python implementations
 * must reproduce them bit-for-bit.
 *
 * Run: node --experimental-strip-types tools/gen_jcs_fixtures.mjs
 */
import { writeFileSync, mkdirSync } from "node:fs";
import { createHash } from "node:crypto";
import { canonicalize } from "../src/ts/arifflow/jcs.ts";

const fixtures = [
  {
    id: 1,
    note: "minimal root receipt",
    value: {
      receipt_id: "00000000-0000-4000-8000-000000000001",
      created_at: "2026-09-13T00:00:00Z",
      actor_id: "fi-003-qwen",
      session_id: "SEAL-jcs-fixture-001",
      step_type: "Execute",
      step_number: 0,
      cost_ns: 0,
      epistemic_label: "Observation",
      floor_verdict: "Pass",
      parent_receipt_ids: [],
    },
  },
  {
    id: 2,
    note: "single-parent receipt",
    value: {
      receipt_id: "00000000-0000-4000-8000-000000000002",
      created_at: "2026-09-13T00:00:00.000001Z",
      actor_id: "qwen-code",
      session_id: "SEAL-jcs-fixture-001",
      step_type: "Verify",
      step_number: 1,
      cost_ns: 1200000,
      epistemic_label: "Observation",
      floor_verdict: "Pass",
      routed_organ: "AAA",
      parent_receipt_ids: ["00000000-0000-4000-8000-000000000001"],
    },
  },
  {
    id: 3,
    note: "multi-parent receipt (fan-out merge)",
    value: {
      receipt_id: "00000000-0000-4000-8000-000000000003",
      created_at: "2026-09-13T00:00:00.000002Z",
      actor_id: "a-forge",
      session_id: "SEAL-jcs-fixture-003",
      step_type: "Execute",
      step_number: 4,
      cost_ns: 987654321,
      epistemic_label: "Derivation",
      floor_verdict: "Pass",
      topology_id: "cascade-alpha",
      lane_id: 2,
      routed_organ: "A-FORGE",
      parent_receipt_ids: [
        "00000000-0000-4000-8000-000000000001",
        "00000000-0000-4000-8000-000000000002",
      ],
    },
  },
  {
    id: 4,
    note: "unicode + UTF-16 key-order proof (U+FFFD vs U+10000 flip)",
    value: {
      receipt_id: "00000000-0000-4000-8000-000000000004",
      created_at: "2026-09-13T00:00:00Z",
      actor_id: "信道試験-৩৩৩-AGI",
      session_id: "SEAL-jcs-fixture-004",
      step_type: "Execute",
      step_number: 0,
      cost_ns: 0,
      epistemic_label: "Observation",
      floor_verdict: "Pass",
      summary: "emoji 🇲🇾 + combining é̄ + CJK 漢字",
      extensions: {
        "\uFFFD": "replacement-char-key (BMP high)",
        "\uD800\uDC00": "linear-b-syllable-key (astral low)",
        "zz_last_by_points": "key order must be UTF-16, not code-point",
      },
      parent_receipt_ids: [],
    },
  },
  {
    id: 5,
    note: "absent optional fields omitted entirely",
    value: {
      receipt_id: "00000000-0000-4000-8000-000000000005",
      created_at: "2026-09-13T00:00:00Z",
      actor_id: "hermes-asi",
      session_id: "SEAL-jcs-fixture-005",
      step_type: "Execute",
      step_number: 2,
      cost_ns: 500,
      epistemic_label: "Observation",
      floor_verdict: "Pass",
      parent_receipt_ids: ["00000000-0000-4000-8000-000000000004"],
    },
  },
  {
    id: 6,
    note: "explicit null where semantically allowed",
    value: {
      receipt_id: "00000000-0000-4000-8000-000000000006",
      created_at: "2026-09-13T00:00:00Z",
      actor_id: "codex-startup",
      session_id: "SEAL-jcs-fixture-006",
      session_token: null,
      previous_receipt_hash: null,
      step_type: "Execute",
      step_number: 0,
      cost_ns: 0,
      epistemic_label: "Observation",
      floor_verdict: "Pass",
      parent_receipt_ids: [],
    },
  },
  {
    id: 7,
    note: "ordered arrays preserved (never sorted)",
    value: {
      receipt_id: "00000000-0000-4000-8000-000000000007",
      created_at: "2026-09-13T00:00:00Z",
      actor_id: "grok-build",
      session_id: "SEAL-jcs-fixture-007",
      step_type: "Cool",
      step_number: 9,
      cost_ns: 42,
      epistemic_label: "Observation",
      floor_verdict: "Pass",
      extensions: { sequence: [3, 1, 2], labels: ["c", "a", "b"] },
      parent_receipt_ids: [
        "00000000-0000-4000-8000-000000000003",
        "00000000-0000-4000-8000-000000000001",
        "00000000-0000-4000-8000-000000000002",
      ],
    },
  },
  {
    id: 8,
    note: "high-precision RFC3339 timestamp string",
    value: {
      receipt_id: "00000000-0000-4000-8000-000000000008",
      created_at: "2026-09-13T00:51:35.072989123Z",
      actor_id: "arif",
      session_id: "SEAL-d41edbd74936426e",
      step_type: "Seal",
      step_number: 36,
      cost_ns: 5129494123456789,
      epistemic_label: "Seal",
      floor_verdict: "Pass",
      genesis_anchor: "RCP-000/000/F13",
      parent_receipt_ids: [],
    },
  },
  {
    id: 9,
    note: "negative + max-safe (2^53-1) integer edges",
    value: {
      receipt_id: "00000000-0000-4000-8000-000000000009",
      created_at: "2026-09-13T00:00:00Z",
      actor_id: "kimi-code/FI-008",
      session_id: "SEAL-jcs-fixture-009",
      step_type: "Execute",
      step_number: 9007199254740991,
      cost_ns: 0,
      epistemic_label: "Observation",
      floor_verdict: "Pass",
      extensions: {
        neg_max_safe: -9007199254740991,
        max_safe: 9007199254740991,
        neg_one: -1,
        zero: 0,
      },
      parent_receipt_ids: [],
    },
  },
  {
    id: 10,
    note: "classification-redacted receipt (opaque fields)",
    value: {
      receipt_id: "00000000-0000-4000-8000-00000000000a",
      created_at: "2026-09-13T00:00:00Z",
      actor_id: "fi-003-qwen",
      session_id: "REDACTED:sha256:0f2a",
      step_type: "Execute",
      step_number: 1,
      cost_ns: 77,
      epistemic_label: "Observation",
      floor_verdict: "Pass",
      summary: "[REDACTED:classification]",
      extensions: {
        classification: "RESTRICTED",
        redacted_fields: ["session_id", "summary"],
      },
      parent_receipt_ids: ["REDACTED:parent"],
    },
  },
];

// Float serialization sweep — outside the numbered fixture set; schema
// discipline says floats are avoided, but serializers must still agree on
// the ECMAScript number grammar where they appear.
// NOTE: integer-looking literals beyond 2^53 (e.g. JSON.stringify(1e20) =
// "100000000000000000000") are deliberately EXCLUDED: JS reads them as
// doubles, Python as arbitrary-precision ints — the exact cross-language
// divergence the contract's number discipline forbids. Canonicalizers
// reject such ints (guarded in tests).
const floatSweep = [0.5, -2.25, 1e21, 1e-7, 1.5e-7, 123456.789, -0.0, 1e15, 0.0001];

const out = {
  schema: "arifflow.spec/jcs-golden-fixtures/v1",
  contract: "spec/RG2_JCS_HASH_CONTRACT_v1.md",
  canonicalization: "arifflow-jcs-v1 (RFC 8785)",
  hash: "sha3-256",
  reference_runtime: "node (ECMAScript semantics)",
  fixtures: fixtures.map((f) => {
    const canonical = canonicalize(f.value);
    return {
      id: f.id,
      note: f.note,
      value: f.value,
      canonical,
      sha3_256: createHash("sha3-256").update(canonical, "utf8").digest("hex"),
    };
  }),
  float_edge_sweep: floatSweep.map((v) => ({
    value: v,
    canonical: canonicalize(v),
  })),
};

mkdirSync(new URL("../tests/fixtures/", import.meta.url), { recursive: true });
const dest = new URL("../tests/fixtures/jcs_golden_v1.json", import.meta.url);
writeFileSync(dest, JSON.stringify(out, null, 2) + "\n");
console.log(
  `written: ${dest.pathname} — ${out.fixtures.length} fixtures + ${out.float_edge_sweep.length} float edges`,
);
for (const f of out.fixtures) {
  console.log(`  #${f.id} ${f.sha3_256.slice(0, 16)}… ${f.note}`);
}
