//! jcs.rs — RFC 8785 (JSON Canonicalization Scheme) for arifFlow receipts.
//!
//! Contract: `spec/RG2_JCS_HASH_CONTRACT_v1.md` (encoding `arifflow-jcs-v1`).
//! Reference runtime is Node/ECMAScript; this module must reproduce its
//! canonical bytes bit-for-bit (golden fixtures: `tests/fixtures/jcs_golden_v1.json`).
//!
//! NOTE: this does NOT replace `FlowReceipt::hash()` — the live seal chain
//! depends on the existing serialization. JCS is the cross-language
//! canonical hash that the three-names interim rule will converge on.

use serde_json::Value;
use sha3::{Digest, Sha3_256};

/// I-JSON (RFC 7493) safe integer bound: 2^53 − 1.
const IJSON_MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

#[derive(Debug, thiserror::Error)]
pub enum JcsError {
    #[error(
        "jcs: integer {0} exceeds I-JSON safe range (schema discipline: ints must be <= 2^53; use a string)"
    )]
    UnsafeInteger(i64),
    #[error("jcs: unsupported JSON value")]
    UnsupportedValue,
}

/// Compare strings by UTF-16 code units (RFC 8785 §3.2.3) — NOT by code
/// points. Encoded `utf16` sequences compare in exact code-unit order, and
/// astral-plane keys (surrogate pairs) flip position relative to BMP-high
/// keys compared to Rust's default `str` ordering.
fn utf16_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let ua = a.encode_utf16().collect::<Vec<u16>>();
    let ub = b.encode_utf16().collect::<Vec<u16>>();
    ua.cmp(&ub)
}

/// Serialize one string with ECMAScript `JSON.stringify` escaping:
/// `"` `\` plus `\b \t \n \f \r` and `\u00xx` (lowercase hex) for the
/// remaining control chars; everything else passes through raw UTF-8.
/// Rust `String`s are valid UTF-8, so lone surrogates cannot occur here.
fn escape_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{0009}' => out.push_str("\\t"),
            '\u{000a}' => out.push_str("\\n"),
            '\u{000c}' => out.push_str("\\f"),
            '\u{000d}' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Serialize an f64 per ECMAScript `Number::toString` (RFC 8785 §3.2.2.3):
/// shortest round-trip digits, positional notation for 1e-6 ≤ |x| < 1e21,
/// otherwise `d.ddd` `e` `±exp` with a signed exponent.
fn ecmascript_f64(x: f64, out: &mut String) {
    if x == 0.0 {
        out.push('0'); // covers -0.0 — JSON.stringify(-0) === "0"
        return;
    }
    let sign = if x < 0.0 { "-" } else { "" };
    let a = x.abs();
    // Rust's {:e} yields the shortest round-trip mantissa (unit-tested below).
    let exp_form = format!("{a:e}");
    let (mant, exp_str) = exp_form.split_once('e').expect("{:e} always has exponent");
    let e: i32 = exp_str.parse().expect("{:e} exponent is an integer");
    let mut digits: String = mant.chars().filter(|c| *c != '.').collect();
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }
    let n = digits.len() as i32;
    out.push_str(sign);
    if (1e-6..1e21).contains(&a) {
        let p = e + 1; // digits before the decimal point in positional form
        if p <= 0 {
            out.push_str("0.");
            for _ in 0..-p {
                out.push('0');
            }
            out.push_str(&digits);
        } else if p >= n {
            out.push_str(&digits);
            for _ in 0..(p - n) {
                out.push('0');
            }
        } else {
            out.push_str(&digits[..p as usize]);
            out.push('.');
            out.push_str(&digits[p as usize..]);
        }
    } else {
        out.push_str(&digits[..1]);
        if n > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        // ECMAScript prints a signed exponent with no zero padding: e+21, e-7.
        if e >= 0 {
            out.push('+');
        }
        out.push_str(&e.to_string());
    }
}

fn serialize(value: &Value, out: &mut String) -> Result<(), JcsError> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::String(s) => escape_string(s, out),
        Value::Number(num) => {
            if let Some(i) = num.as_i64() {
                if !(-IJSON_MAX_SAFE_INTEGER..=IJSON_MAX_SAFE_INTEGER).contains(&i) {
                    return Err(JcsError::UnsafeInteger(i));
                }
                out.push_str(&i.to_string());
            } else if let Some(u) = num.as_u64() {
                if u > IJSON_MAX_SAFE_INTEGER as u64 {
                    return Err(JcsError::UnsafeInteger(u as i64));
                }
                out.push_str(&u.to_string());
            } else if let Some(f) = num.as_f64() {
                ecmascript_f64(f, out);
            } else {
                return Err(JcsError::UnsupportedValue);
            }
        }
        Value::Array(items) => {
            out.push('[');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                serialize(item, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| utf16_cmp(a, b));
            out.push('{');
            for (idx, key) in keys.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                escape_string(key, out);
                out.push(':');
                serialize(&map[key.as_str()], out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// Canonical JSON encoding (RFC 8785) of a serde_json value.
pub fn canonicalize(value: &Value) -> Result<String, JcsError> {
    let mut out = String::new();
    serialize(value, &mut out)?;
    Ok(out)
}

/// SHA3-256 (FIPS 202) over the JCS encoding of `value` — lowercase hex.
pub fn jcs_sha3_hex(value: &Value) -> Result<String, JcsError> {
    let canonical = canonicalize(value)?;
    let mut hasher = Sha3_256::new();
    Digest::update(&mut hasher, canonical.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const GOLDEN: &str = include_str!("../tests/fixtures/jcs_golden_v1.json");

    #[test]
    fn fixtures_canonical_bytes_match_reference() {
        let doc: Value = serde_json::from_str(GOLDEN).unwrap();
        for fx in doc["fixtures"].as_array().unwrap() {
            let got = canonicalize(&fx["value"]).unwrap();
            let want = fx["canonical"].as_str().unwrap();
            assert_eq!(got, want, "fixture #{}", fx["id"]);
        }
    }

    #[test]
    fn fixtures_sha3_256_matches_reference() {
        let doc: Value = serde_json::from_str(GOLDEN).unwrap();
        for fx in doc["fixtures"].as_array().unwrap() {
            let got = jcs_sha3_hex(&fx["value"]).unwrap();
            let want = fx["sha3_256"].as_str().unwrap();
            assert_eq!(got, want, "fixture #{}", fx["id"]);
        }
    }

    #[test]
    fn float_edge_sweep_matches_reference() {
        let doc: Value = serde_json::from_str(GOLDEN).unwrap();
        for case in doc["float_edge_sweep"].as_array().unwrap() {
            let got = canonicalize(&case["value"]).unwrap();
            let want = case["canonical"].as_str().unwrap();
            assert_eq!(got, want, "value {}", case["value"]);
        }
    }

    #[test]
    fn utf16_key_order_not_codepoint_order() {
        // U+10000 (surrogate pair, first unit 0xD800) sorts BEFORE U+FFFD
        // under UTF-16 code-unit order — the opposite of code-point order.
        let value = json!({ "\u{FFFD}": 1, "\u{10000}": 2 });
        let canonical = canonicalize(&value).unwrap();
        assert!(canonical.find('\u{10000}').unwrap() < canonical.find('\u{FFFD}').unwrap());
    }

    #[test]
    fn unsafe_integer_rejected() {
        assert!(matches!(
            canonicalize(&json!({ "too_big": 9_007_199_254_740_992_i64 })),
            Err(JcsError::UnsafeInteger(_))
        ));
        assert!(matches!(
            canonicalize(&json!({ "too_small": -9_007_199_254_740_992_i64 })),
            Err(JcsError::UnsafeInteger(_))
        ));
    }

    #[test]
    fn rust_lowerexp_is_shortest_round_trip() {
        // The ecmascript_f64 implementation trusts {:e} for shortest
        // round-trip digits — pin the property against known values.
        assert_eq!(format!("{:e}", 123456.789f64), "1.23456789e5");
        assert_eq!(format!("{:e}", 0.5f64), "5e-1");
        assert_eq!(format!("{:e}", 1e-7f64), "1e-7");
        assert_eq!(format!("{:e}", 1e21f64), "1e21");
        // round-trip property over a deterministic pseudo-random sweep
        let mut x: u64 = 0x9E3779B97F4A7C15;
        for _ in 0..10_000 {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let f = (x as f64) / (u64::MAX as f64) * 1e12;
            let repr = format!("{:e}", f);
            let back: f64 = repr.parse().unwrap();
            assert_eq!(format!("{:e}", back), repr, "not round-trip stable: {f}");
        }
    }
}
