#!/usr/bin/env python3
"""jcs_runner.py — Python arm of the JCS cross-language conformance harness.

Usage: jcs_runner.py <fixtures.json> <src_py_dir>
Emits one JSON line per corpus case (same key scheme as the Rust bin and
the Node orchestrator): {"kind":"fixture"|"float","idx":N,...}.
"""

import json
import sys

sys.path.insert(0, sys.argv[2])
from arifflow.jcs import canonicalize, jcs_sha3_hex  # noqa: E402

doc = json.load(open(sys.argv[1], encoding="utf-8"))
for idx, fx in enumerate(doc["fixtures"]):
    print(
        json.dumps(
            {
                "kind": "fixture",
                "idx": idx,
                "canonical": canonicalize(fx["value"]),
                "sha3_256": jcs_sha3_hex(fx["value"]),
            }
        )
    )
for idx, case in enumerate(doc["float_edge_sweep"]):
    print(
        json.dumps(
            {"kind": "float", "idx": idx, "canonical": canonicalize(case["value"])}
        )
    )
