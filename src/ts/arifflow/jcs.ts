/**
 * jcs.ts — RFC 8785 (JSON Canonicalization Scheme) for arifFlow receipts.
 * Contract: spec/RG2_JCS_HASH_CONTRACT_v1 (arifflow-jcs-v1).
 *
 * ECMAScript runtime semantics ARE the RFC 8785 target semantics, so this
 * implementation is the federation's reference: sort keys by UTF-16 code
 * units (default string comparison), serialize with JSON.stringify.
 * Hash: SHA3-256 (FIPS 202), hex-encoded.
 */

import { createHash } from "node:crypto";

/** Sort object keys per RFC 8785 §3.2.3 and serialize canonically. */
export function canonicalize(value: unknown): string {
  return serialize(canonicalValue(value));
}

function canonicalValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(canonicalValue);
  }
  if (value !== null && typeof value === "object") {
    const sorted: Record<string, unknown> = {};
    // Default Array<string>.sort compares UTF-16 code units — exactly JCS.
    for (const key of Object.keys(value).sort()) {
      sorted[key] = canonicalValue((value as Record<string, unknown>)[key]);
    }
    return sorted;
  }
  return value;
}

function serialize(value: unknown): string {
  // JSON.stringify applies ECMAScript escaping + number formatting natively.
  // Since ES2019 it is well-formed: lone surrogates come out ESCAPED as
  // \udXXX — valid JSON but NOT canonical (Rust/Python reject such inputs).
  // Detect both raw units (defensive) and escape sequences (actual path).
  const out = JSON.stringify(value);
  if (out === undefined) {
    throw new TypeError("jcs: value is not JSON-serializable");
  }
  if (/\\u[dD][89abAB][0-9a-fA-F]{2}/.test(out)) {
    throw new TypeError("jcs: lone surrogate (I-JSON violation)");
  }
  for (let i = 0; i < out.length; i++) {
    const c = out.charCodeAt(i);
    if (c >= 0xd800 && c <= 0xdbff) {
      const next = out.charCodeAt(i + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) {
        throw new TypeError("jcs: lone high surrogate (I-JSON violation)");
      }
      i++; // pair consumed
    } else if (c >= 0xdc00 && c <= 0xdfff) {
      throw new TypeError("jcs: lone low surrogate (I-JSON violation)");
    }
  }
  return out;
}

/** Canonical SHA3-256 over the JCS encoding of `value`. Hex, lowercase. */
export function jcsSha3Hex(value: unknown): string {
  return createHash("sha3-256").update(canonicalize(value), "utf8").digest("hex");
}
