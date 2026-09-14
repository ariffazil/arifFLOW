"""jcs.py — RFC 8785 (JSON Canonicalization Scheme) for arifFlow receipts.

Contract: spec/RG2_JCS_HASH_CONTRACT_v1 (encoding ``arifflow-jcs-v1``).
Reference runtime is Node/ECMAScript; this module must reproduce its bytes
bit-for-bit (golden fixtures: tests/fixtures/jcs_golden_v1.json).

stdlib only — no third-party dependencies.
"""

from __future__ import annotations

import decimal
import hashlib
import json
import math
from typing import Any

__all__ = ["canonicalize", "jcs_sha3_hex", "JcsError"]

_IJSON_MAX_SAFE_INTEGER = 9007199254740991  # 2^53 - 1


class JcsError(ValueError):
    """I-JSON / canonicalization violation."""


def _ecmascript_number(x: float) -> str:
    """Serialize a float per ECMAScript ``Number::toString`` (RFC 8785 §3.2.2.3)."""
    if math.isnan(x) or math.isinf(x):
        raise JcsError("jcs: non-finite number (I-JSON violation)")
    if x == 0.0:
        return "0"
    sign = "-" if x < 0 else ""
    a = abs(x)
    # repr() is Python's shortest round-trip form (same digit-optimality as
    # ECMAScript). Decimal parses its digits+exponent exactly — {:e} cannot
    # be used (6 significant digits only).
    dt = decimal.Decimal(repr(a)).as_tuple()
    digits = "".join(map(str, dt.digits)).rstrip("0") or "0"
    n = len(digits)
    e = dt.exponent + (n - 1)  # value = 0.<digits> × 10^(e+1) = d.ddd × 10^e
    if 1e-6 <= a < 1e21:
        p = e + 1  # digits before the decimal point in positional form
        if p <= 0:
            body = "0." + "0" * (-p) + digits
        elif p >= n:
            body = digits + "0" * (p - n)
        else:
            body = digits[:p] + "." + digits[p:]
        return sign + body
    mant_str = digits[0] + ("." + digits[1:] if n > 1 else "")
    return f"{sign}{mant_str}e{e:+d}"


def _escape(s: str) -> str:
    # json.dumps escaping matches the ECMAScript escape set exactly:
    # \\ \" \b \f \n \r \t and \u00xx (lowercase) for other control chars;
    # everything else passes through as raw UTF-8.
    return json.dumps(s, ensure_ascii=False)


def _serialize(value: Any) -> str:
    if value is None:
        return "null"
    if value is True:
        return "true"
    if value is False:
        return "false"
    if isinstance(value, str):
        return _escape(value)
    if isinstance(value, int):
        if not (-_IJSON_MAX_SAFE_INTEGER <= value <= _IJSON_MAX_SAFE_INTEGER):
            raise JcsError(
                f"jcs: integer {value} exceeds I-JSON safe range "
                "(schema discipline: ints must be <= 2^53; use a string)"
            )
        return str(value)
    if isinstance(value, float):
        return _ecmascript_number(value)
    if isinstance(value, list):
        return "[" + ",".join(_serialize(v) for v in value) + "]"
    if isinstance(value, dict):
        for k in value:
            if not isinstance(k, str):
                raise JcsError("jcs: object keys must be strings")
        # RFC 8785 §3.2.3: sort by UTF-16 code units, NOT code points —
        # utf-16-be byte order is exactly code-unit order (astral keys flip).
        items = sorted(value.items(), key=lambda kv: kv[0].encode("utf-16-be"))
        return "{" + ",".join(f"{_escape(k)}:{_serialize(v)}" for k, v in items) + "}"
    raise JcsError(f"jcs: unsupported type {type(value).__name__}")


def canonicalize(value: Any) -> str:
    """Canonical JSON encoding (RFC 8785) of ``value``."""
    out = _serialize(value)
    try:
        out.encode("utf-8")
    except UnicodeEncodeError as exc:
        raise JcsError(f"jcs: lone surrogate in string (I-JSON violation): {exc}") from exc
    return out


def jcs_sha3_hex(value: Any) -> str:
    """SHA3-256 (FIPS 202) over the JCS encoding of ``value`` — lowercase hex."""
    return hashlib.sha3_256(canonicalize(value).encode("utf-8")).hexdigest()
