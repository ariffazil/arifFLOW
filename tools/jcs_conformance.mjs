/**
 * jcs_conformance.mjs — the JCS cross-language conformance harness.
 *
 * One corpus, three runtimes, byte-equality asserted pairwise AND against
 * the corpus-embedded expected values. This is deliberately NOT "each
 * implementation passes its own tests" — it is "all three emit identical
 * bytes for the same input", which is the property that outlives sessions
 * and survives silent refactors.
 *
 * Run from repo root:
 *   node --experimental-strip-types tools/jcs_conformance.mjs
 *
 * Artifacts: tests/fixtures/jcs_conformance_report.json (machine-readable)
 * Exit: 0 = CONFORMANT, 1 = DIVERGENCE (CI gate).
 */
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { canonicalize, jcsSha3Hex } from "../src/ts/arifflow/jcs.ts";

const corpusPath = new URL("../tests/fixtures/jcs_golden_v1.json", import.meta.url);
const corpusRaw = readFileSync(corpusPath);
const corpus = JSON.parse(corpusRaw.toString("utf8"));
const corpusSha = createHash("sha3-256").update(corpusRaw).digest("hex");

// ── Node arm (reference runtime — computed in-process) ──
const nodeResults = [];
corpus.fixtures.forEach((fx, idx) => {
  nodeResults.push({
    kind: "fixture",
    idx,
    canonical: canonicalize(fx.value),
    sha3_256: jcsSha3Hex(fx.value),
  });
});
corpus.float_edge_sweep.forEach((c, idx) => {
  nodeResults.push({ kind: "float", idx, canonical: canonicalize(c.value) });
});

// ── Python arm ──
const py = spawnSync(
  "python3",
  [
    new URL("./jcs_runner.py", import.meta.url).pathname,
    corpusPath.pathname,
    new URL("../src/py/", import.meta.url).pathname,
  ],
  { encoding: "utf8", timeout: 120_000 },
);
if (py.status !== 0) {
  console.error("python arm failed:", py.stderr?.slice(0, 500));
  process.exit(1);
}
const pyResults = py.stdout.trim().split("\n").map((l) => JSON.parse(l));

// ── Rust arm ──
const rust = spawnSync(
  "cargo",
  ["run", "--quiet", "--bin", "jcs_conformance", "--", corpusPath.pathname],
  { encoding: "utf8", timeout: 600_000 },
);
if (rust.status !== 0) {
  console.error("rust arm failed:", rust.stderr?.slice(0, 500));
  process.exit(1);
}
const rustResults = rust.stdout.trim().split("\n").map((l) => JSON.parse(l));

// ── Pairwise + expected comparison ──
const mismatches = [];
const expected = new Map();
corpus.fixtures.forEach((fx, idx) =>
  expected.set(`fixture:${idx}`, { canonical: fx.canonical, sha3_256: fx.sha3_256 }),
);
corpus.float_edge_sweep.forEach((c, idx) =>
  expected.set(`float:${idx}`, { canonical: c.canonical }),
);

function toMap(rows, runtime) {
  const m = new Map();
  for (const r of rows) m.set(`${r.kind}:${r.idx}`, r);
  if (m.size !== expected.size) {
    mismatches.push({
      key: "*",
      runtime,
      issue: `emitted ${m.size} cases, corpus has ${expected.size}`,
    });
  }
  return m;
}

const arms = {
  node: toMap(nodeResults, "node"),
  python: toMap(pyResults, "python"),
  rust: toMap(rustResults, "rust"),
};

for (const key of expected.keys()) {
  const want = expected.get(key);
  for (const [runtime, map] of Object.entries(arms)) {
    const got = map.get(key);
    if (!got) {
      mismatches.push({ key, runtime, issue: "missing case" });
      continue;
    }
    if (got.canonical !== want.canonical) {
      mismatches.push({
        key,
        runtime,
        issue: "canonical bytes differ",
        expected: want.canonical,
        got: got.canonical,
      });
    }
    if ("sha3_256" in want && got.sha3_256 !== want.sha3_256) {
      mismatches.push({
        key,
        runtime,
        issue: "sha3-256 differs",
        expected: want.sha3_256,
        got: got.sha3_256,
      });
    }
  }
  // pairwise cross-runtime (redundant with expected-match, but states the
  // property directly: all three identical)
  const c =
    arms.node.get(key)?.canonical === arms.python.get(key)?.canonical &&
    arms.python.get(key)?.canonical === arms.rust.get(key)?.canonical;
  if (!c) mismatches.push({ key, runtime: "*", issue: "cross-runtime divergence" });
}

const report = {
  schema: "arifflow.spec/jcs-conformance-report/v1",
  generated_utc: new Date().toISOString(),
  corpus: {
    path: "tests/fixtures/jcs_golden_v1.json",
    sha3_256: corpusSha,
    fixtures: corpus.fixtures.length,
    float_edges: corpus.float_edge_sweep.length,
  },
  runtimes: {
    node: process.version,
    python: spawnSync("python3", ["--version"], { encoding: "utf8" }).stdout.trim(),
    rust: spawnSync("cargo", ["--version"], { encoding: "utf8" }).stdout.trim(),
  },
  checks: {
    canonical_bytes_identical_across_runtimes: mismatches.length === 0,
    sha3_256_identical_across_runtimes: mismatches.length === 0,
    matches_corpus_expected: mismatches.length === 0,
  },
  verdict: mismatches.length === 0 ? "CONFORMANT" : "DIVERGENT",
  mismatches,
};

const reportPath = new URL(
  "../tests/fixtures/jcs_conformance_report.json",
  import.meta.url,
);
writeFileSync(reportPath, JSON.stringify(report, null, 2) + "\n");
console.log(`corpus sha3-256: ${corpusSha}`);
console.log(`cases: ${expected.size} (${corpus.fixtures.length} fixtures + ${corpus.float_edge_sweep.length} float edges)`);
console.log(`verdict: ${report.verdict}`);
if (mismatches.length) {
  console.log(`mismatches: ${mismatches.length}`);
  for (const m of mismatches.slice(0, 5)) console.log("  ", JSON.stringify(m).slice(0, 160));
  process.exit(1);
}
