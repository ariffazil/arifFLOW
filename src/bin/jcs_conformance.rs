//! jcs_conformance — cross-language conformance runner (Rust arm).
//!
//! Reads the golden fixture corpus and emits one JSON line per case,
//! keyed by corpus index (all three runtimes read the same file, so index
//! pairing is stable):
//!   {"kind":"fixture","idx":N,"canonical":"...","sha3_256":"..."}
//!   {"kind":"float","idx":N,"canonical":"..."}
//!
//! Orchestrator: tools/jcs_conformance.mjs (compares Rust vs Python vs Node
//! byte-for-byte and against the corpus-embedded expected values).

use arifflow::jcs::{canonicalize, jcs_sha3_hex};
use serde_json::Value;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: jcs_conformance <fixtures.json>");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for (idx, fx) in doc["fixtures"].as_array().unwrap().iter().enumerate() {
        println!(
            "{}",
            serde_json::json!({
                "kind": "fixture",
                "idx": idx,
                "canonical": canonicalize(&fx["value"]).unwrap(),
                "sha3_256": jcs_sha3_hex(&fx["value"]).unwrap(),
            })
        );
    }
    for (idx, case) in doc["float_edge_sweep"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        println!(
            "{}",
            serde_json::json!({
                "kind": "float",
                "idx": idx,
                "canonical": canonicalize(&case["value"]).unwrap(),
            })
        );
    }
}
