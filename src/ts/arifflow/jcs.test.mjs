/**
 * jcs.test.mjs — golden-fixture conformance for the TS reference itself.
 *
 * Run: node --experimental-strip-types --test src/ts/arifflow/jcs.test.mjs
 * (plain `node --experimental-strip-types <file>` also works — asserts exit non-zero on failure)
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { canonicalize, jcsSha3Hex } from "./jcs.ts";

const doc = JSON.parse(
  readFileSync(
    new URL("../../../tests/fixtures/jcs_golden_v1.json", import.meta.url),
    "utf8",
  ),
);

let failures = 0;
function check(name, fn) {
  try {
    fn();
    console.log(`  PASS ${name}`);
  } catch (err) {
    failures += 1;
    console.log(`  FAIL ${name}: ${err.message}`);
  }
}

check("fixtures_canonical_bytes_stable", () => {
  for (const fx of doc.fixtures) {
    if (canonicalize(fx.value) !== fx.canonical) {
      throw new Error(`fixture #${fx.id}`);
    }
  }
});

check("fixtures_sha3_256_stable", () => {
  for (const fx of doc.fixtures) {
    if (jcsSha3Hex(fx.value) !== fx.sha3_256) {
      throw new Error(`fixture #${fx.id}`);
    }
  }
});

check("float_edge_sweep_stable", () => {
  for (const c of doc.float_edge_sweep) {
    if (canonicalize(c.value) !== c.canonical) {
      throw new Error(`value ${c.value}`);
    }
  }
});

check("lone_surrogate_rejected", () => {
  let threw = false;
  try {
    canonicalize({ bad: "\ud800" });
  } catch {
    threw = true;
  }
  if (!threw) throw new Error("expected TypeError for lone surrogate");
});

check("unsafe_integer_not_representable_upstream", () => {
  // JSON.parse silently rounds ints > 2^53 — the divergence the schema
  // discipline forbids. Documented here; enforcement is schema-level for JS.
  const rounded = JSON.parse("9007199254740993");
  if (rounded !== 9007199254740992) throw new Error("JS no longer rounds?");
});

if (fileURLToPath(import.meta.url) === process.argv[1]) {
  console.log(failures ? `${failures} FAILURES` : "ALL PASS");
  process.exit(failures ? 1 : 0);
}
export { failures };
