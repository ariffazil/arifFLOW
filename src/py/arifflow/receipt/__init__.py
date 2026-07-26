"""
arifFLOW Receipt Layer — Python Mirror
════════════════════════════════════════

Python implementation of the arifFLOW receipt engine.
Mirrors src/ts/arifflow/receipt/engine.ts contract-for-contract.

Cross-language parity:
  - Same JSONL format
  - Same hash chain (SHA-256)
  - Same ID generation
  - Same validation rules

This module is called by arifOS when generating TransitionReceipts.
For FLOW-class and EXECUTION-class receipts, the TypeScript engine
is canonical (arifFLOW runs on Node/Rust). This Python module exists
for interop and offline validation.

Storage: append-only JSONL at /root/arifFlow/data/receipts.jsonl

DITEMPA BUKAN DIBERI — Forged, Not Given.
"""

from __future__ import annotations

import hashlib
import json
import os
import secrets
from datetime import datetime, timezone
from enum import Enum
from typing import Any, Optional

from pydantic import BaseModel, Field


# ── Enums ────────────────────────────────────────────────────────────


class ReceiptClass(str, Enum):
    FLOW = "FLOW"
    EXECUTION = "EXECUTION"
    CONSTITUTIONAL = "CONSTITUTIONAL"


class ReceiptVerdict(str, Enum):
    SEAL = "SEAL"
    SABAR = "SABAR"
    HOLD = "HOLD"
    VOID = "VOID"


# ── Receipt Envelope ─────────────────────────────────────────────────


class ReceiptAuthority(BaseModel):
    actor_id: str
    session_id: str
    valid_until: Optional[str] = None
    lease_id: Optional[str] = None
    scope: Optional[str] = None


class ReceiptBounds(BaseModel):
    reversible: bool
    blast_radius: str  # LOW | MEDIUM | HIGH | CRITICAL
    max_tools: int
    timeout_ms: Optional[int] = None


class ReceiptEnvelope(BaseModel):
    """Canonical receipt — mirrors TypeScript ReceiptEnvelope field-for-field."""

    receipt_id: str
    class_: str = Field(alias="class")
    timestamp: str

    # Lineage
    op_id: str
    session_id: str
    trace_id: str
    organ: str
    capability: Optional[str] = None

    # Payload
    result_summary: str
    evidence_uri: Optional[str] = None
    verdict: Optional[str] = None
    cc_id: Optional[str] = None
    judgment_reference: Optional[str] = None
    authority: Optional[ReceiptAuthority] = None
    bounds: Optional[ReceiptBounds] = None
    input_hash: Optional[str] = None
    kernel_signature: Optional[str] = None

    # Metabolic
    stage: Optional[str] = None
    vault_candidate: bool = False
    signature: Optional[str] = None
    prev_hash: Optional[str] = None
    hash: Optional[str] = None

    class Config:
        populate_by_name = True


# ── Receipt Engine ───────────────────────────────────────────────────

DEFAULT_DATA_DIR = "/root/arifFlow/data"
RECEIPTS_LOG = "receipts.jsonl"


def _iso_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _generate_id(prefix: str) -> str:
    rand = secrets.token_hex(8)
    ts = _iso_now().replace("-", "").replace(":", "").replace(".", "")[:14]
    return f"{prefix}-{ts}-{rand}"


def _sha256(data: str) -> str:
    return hashlib.sha256(data.encode("utf-8")).hexdigest()


def _receipts_path(data_dir: str = DEFAULT_DATA_DIR) -> str:
    return os.path.join(data_dir, RECEIPTS_LOG)


def _ensure_dir(data_dir: str = DEFAULT_DATA_DIR) -> None:
    os.makedirs(data_dir, exist_ok=True)


def _get_last_hash(file_path: str) -> Optional[str]:
    if not os.path.exists(file_path):
        return None
    with open(file_path, "r") as f:
        lines = [line.strip() for line in f if line.strip()]
    if not lines:
        return None
    last = json.loads(lines[-1])
    return last.get("hash")


def _append_line(file_path: str, obj: dict) -> None:
    with open(file_path, "a") as f:
        f.write(json.dumps(obj, default=str) + "\n")


def _read_all(file_path: str) -> list[dict]:
    if not os.path.exists(file_path):
        return []
    with open(file_path, "r") as f:
        lines = [line.strip() for line in f if line.strip()]
    return [json.loads(line) for line in lines]


def emit_receipt(
    *,
    class_: ReceiptClass,
    op_id: str,
    session_id: str,
    trace_id: str,
    organ: str,
    result_summary: str,
    capability: Optional[str] = None,
    evidence_uri: Optional[str] = None,
    verdict: Optional[ReceiptVerdict] = None,
    cc_id: Optional[str] = None,
    judgment_reference: Optional[str] = None,
    authority: Optional[ReceiptAuthority] = None,
    bounds: Optional[ReceiptBounds] = None,
    input_hash: Optional[str] = None,
    kernel_signature: Optional[str] = None,
    stage: Optional[str] = None,
    vault_candidate: bool = False,
    data_dir: str = DEFAULT_DATA_DIR,
) -> dict:
    """
    Canonical receipt emission — Python mirror of ReceiptEngine.emit().

    Produces a hash-chained ReceiptEnvelope and appends to receipts.jsonl.
    All organs call this function. One emit path, one storage format.
    """
    _ensure_dir(data_dir)
    file_path = _receipts_path(data_dir)
    prev_hash = _get_last_hash(file_path)

    envelope = {
        "receipt_id": _generate_id("rcpt"),
        "class": class_.value,
        "timestamp": _iso_now(),
        "op_id": op_id,
        "session_id": session_id,
        "trace_id": trace_id,
        "organ": organ,
        "capability": capability,
        "result_summary": result_summary,
        "evidence_uri": evidence_uri,
        "verdict": verdict.value if verdict else None,
        "cc_id": cc_id,
        "judgment_reference": judgment_reference,
        "authority": authority.model_dump() if isinstance(authority, ReceiptAuthority) else authority,
        "bounds": bounds.model_dump() if isinstance(bounds, ReceiptBounds) else bounds,
        "input_hash": input_hash,
        "kernel_signature": kernel_signature,
        "stage": stage,
        "vault_candidate": vault_candidate,
        "prev_hash": prev_hash,
    }

    # Compute hash
    hashable = json.dumps({
        "receipt_id": envelope["receipt_id"],
        "class": envelope["class"],
        "timestamp": envelope["timestamp"],
        "op_id": envelope["op_id"],
        "session_id": envelope["session_id"],
        "trace_id": envelope["trace_id"],
        "organ": envelope["organ"],
        "result_summary": envelope["result_summary"],
        "verdict": envelope["verdict"],
        "cc_id": envelope["cc_id"],
        "prev_hash": envelope["prev_hash"],
    }, sort_keys=True)
    envelope["hash"] = _sha256(hashable)

    _append_line(file_path, envelope)
    return envelope


def verify_receipt(receipt: dict) -> tuple[bool, list[str]]:
    """
    Validate a receipt against constitutional requirements.
    Python mirror of ReceiptEngine.verify().
    """
    violations: list[str] = []

    # Universal
    if not receipt.get("receipt_id"):
        violations.append("Missing receipt_id")
    if not receipt.get("op_id"):
        violations.append("Missing op_id")
    if not receipt.get("session_id"):
        violations.append("Missing session_id")
    if not receipt.get("trace_id"):
        violations.append("Missing trace_id")
    if not receipt.get("organ"):
        violations.append("Missing organ")
    if not receipt.get("result_summary"):
        violations.append("Missing result_summary")
    if not receipt.get("timestamp"):
        violations.append("Missing timestamp")
    if not receipt.get("class"):
        violations.append("Missing class")
    if not receipt.get("hash"):
        violations.append("Missing hash — receipt not sealed")

    receipt_class = receipt.get("class", "")
    if receipt_class in ("EXECUTION", "CONSTITUTIONAL"):
        verdict = receipt.get("verdict")
        if not verdict:
            violations.append("Missing verdict")
        elif verdict not in ("SEAL", "SABAR", "HOLD", "VOID"):
            violations.append(f"Invalid verdict: {verdict}")

        if not receipt.get("cc_id"):
            violations.append("Missing cc_id")
        if not receipt.get("judgment_reference"):
            violations.append("Missing judgment_reference")
        if not receipt.get("input_hash"):
            violations.append("Missing input_hash")
        if not receipt.get("kernel_signature"):
            violations.append("Missing kernel_signature")

        authority = receipt.get("authority", {}) or {}
        if not authority.get("actor_id"):
            violations.append("Missing authority.actor_id")
        if not authority.get("session_id"):
            violations.append("Missing authority.session_id")

    if receipt_class == "EXECUTION":
        bounds = receipt.get("bounds", {}) or {}
        if not bounds:
            violations.append("Missing bounds")
        else:
            if "reversible" not in bounds:
                violations.append("Missing bounds.reversible")
            if not bounds.get("blast_radius"):
                violations.append("Missing bounds.blast_radius")
            if not bounds.get("max_tools") or bounds.get("max_tools", 0) < 1:
                violations.append("bounds.max_tools must be >= 1")
            if (
                bounds.get("blast_radius") == "CRITICAL"
                and bounds.get("reversible") is False
            ):
                violations.append(
                    "CRITICAL irreversible action requires F13 sovereign path"
                )

    if receipt_class == "CONSTITUTIONAL":
        if not receipt.get("prev_hash"):
            violations.append("Missing prev_hash — constitutional receipts must chain")

    return len(violations) == 0, violations


def verify_chain(data_dir: str = DEFAULT_DATA_DIR) -> tuple[bool, list[str]]:
    """Verify the entire hash chain integrity. Mirrors ReceiptEngine.verifyChain()."""
    receipts = _read_all(_receipts_path(data_dir))
    violations: list[str] = []

    for i, r in enumerate(receipts):
        hashable = json.dumps({
            "receipt_id": r.get("receipt_id"),
            "class": r.get("class"),
            "timestamp": r.get("timestamp"),
            "op_id": r.get("op_id"),
            "session_id": r.get("session_id"),
            "trace_id": r.get("trace_id"),
            "organ": r.get("organ"),
            "result_summary": r.get("result_summary"),
            "verdict": r.get("verdict"),
            "cc_id": r.get("cc_id"),
            "prev_hash": r.get("prev_hash"),
        }, sort_keys=True)
        computed = _sha256(hashable)

        if computed != r.get("hash"):
            violations.append(
                f"Hash mismatch at {r.get('receipt_id')}: "
                f"stored={str(r.get('hash'))[:12]}... computed={computed[:12]}..."
            )

        if i > 0:
            prev = receipts[i - 1]
            if r.get("prev_hash") != prev.get("hash"):
                violations.append(
                    f"Chain break at {r.get('receipt_id')}: "
                    f"prev_hash doesn't match {prev.get('receipt_id')}"
                )

    return len(violations) == 0, violations


def get_stats(data_dir: str = DEFAULT_DATA_DIR) -> dict:
    """Compute receipt store statistics."""
    receipts = _read_all(_receipts_path(data_dir))

    by_class: dict[str, int] = {}
    by_organ: dict[str, int] = {}
    by_verdict: dict[str, int] = {}
    vault_candidates = 0

    for r in receipts:
        cls = r.get("class", "UNKNOWN")
        by_class[cls] = by_class.get(cls, 0) + 1
        organ = r.get("organ", "UNKNOWN")
        by_organ[organ] = by_organ.get(organ, 0) + 1
        verdict = r.get("verdict")
        if verdict:
            by_verdict[verdict] = by_verdict.get(verdict, 0) + 1
        if r.get("vault_candidate"):
            vault_candidates += 1

    last = receipts[-1] if receipts else None

    return {
        "total_receipts": len(receipts),
        "by_class": by_class,
        "by_organ": by_organ,
        "by_verdict": by_verdict,
        "vault_candidates": vault_candidates,
        "last_receipt_at": last.get("timestamp") if last else None,
        "last_hash": last.get("hash") if last else None,
    }
