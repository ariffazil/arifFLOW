"""Golden-fixture conformance for arifflow.jcs (P4-JCS).

Asserts bit-for-bit parity with the Node/ECMAScript reference:
tests/fixtures/jcs_golden_v1.json — canonical bytes AND SHA3-256.

Run: python3 -m pytest src/py/arifflow/test_jcs.py -q
  or: python3 src/py/arifflow/test_jcs.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

from arifflow.jcs import JcsError, canonicalize, jcs_sha3_hex

FIXTURES = (
    Path(__file__).resolve().parents[3] / "tests" / "fixtures" / "jcs_golden_v1.json"
)


def _load():
    doc = json.loads(FIXTURES.read_text(encoding="utf-8"))
    assert doc["canonicalization"].startswith("arifflow-jcs-v1")
    return doc


def test_fixtures_canonical_bytes_match_reference():
    for fx in _load()["fixtures"]:
        assert canonicalize(fx["value"]) == fx["canonical"], f"fixture #{fx['id']}"


def test_fixtures_sha3_256_matches_reference():
    for fx in _load()["fixtures"]:
        assert jcs_sha3_hex(fx["value"]) == fx["sha3_256"], f"fixture #{fx['id']}"


def test_float_edge_sweep_matches_reference():
    for case in _load()["float_edge_sweep"]:
        assert canonicalize(case["value"]) == case["canonical"], case


def test_utf16_key_order_not_codepoint_order():
    # U+10000 (surrogate pair, first unit 0xD800) sorts BEFORE U+FFFD under
    # UTF-16 code-unit order — the opposite of code-point order.
    value = {"\ufffd": 1, "\U00010000": 2}
    assert canonicalize(value).index("\U00010000") < canonicalize(value).index("\ufffd")


def test_unsafe_integer_rejected():
    try:
        canonicalize({"too_big": _IJSON_MAX + 1})
    except JcsError as exc:
        assert "safe range" in str(exc)
    else:
        raise AssertionError("expected JcsError for int > 2^53")

    try:
        canonicalize({"too_small": -_IJSON_MAX - 1})
    except JcsError:
        pass
    else:
        raise AssertionError("expected JcsError for int < -2^53")


def test_lone_surrogate_rejected():
    try:
        canonicalize({"bad": "\ud800"})
    except JcsError:
        pass
    else:
        raise AssertionError("expected JcsError for lone surrogate")


_IJSON_MAX = 9007199254740991


if __name__ == "__main__":
    failures = 0
    for name, fn in sorted({k: v for k, v in globals().items() if k.startswith("test_")}.items()):
        try:
            fn()
            print(f"  PASS {name}")
        except AssertionError as exc:
            failures += 1
            print(f"  FAIL {name}: {exc}")
    sys.exit(1 if failures else 0)
