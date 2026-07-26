"""
arifFLOW Client — Python bridge for arifOS federation organs.

Usage:
    from arifflow_client import ArifFlowClient, emit_receipt

    client = ArifFlowClient("http://127.0.0.1:7073")
    receipt_id = client.ingest(flow_receipt_dict)

    # Or use the convenience function:
    receipt_id = emit_receipt(
        step_type="Execute",
        organ="arifOS",
        actor_id="opencode",
        session_id="sess_abc123",
        summary="Ran petrophysics computation",
        epistemic="OBS",
        cost_ns=1_500_000,
    )

DITEMPA BUKAN DIBERI — receipts are evidence, not decoration.
"""

from __future__ import annotations

import hashlib
import json
import time
import uuid
from dataclasses import dataclass, field
from datetime import UTC, datetime
from enum import StrEnum
from typing import Any

import requests


# ── Enums (mirrors arifFLOW receipt.rs) ──────────────────────────────────


class StepType(StrEnum):
    EXECUTE = "Execute"
    VERIFY = "Verify"
    COOL = "Cool"
    SEAL = "Seal"
    BARRIER = "Barrier"
    MERGE = "Merge"
    ROUTE = "Route"


class EpistemicLabel(StrEnum):
    OBS = "OBS"
    DER = "DER"
    INT = "INT"
    SPEC = "SPEC"
    SEAL_LABEL = "SEAL"


class FloorVerdict(StrEnum):
    PASS = "PASS"
    CAUTION = "CAUTION"
    HOLD = "HOLD"
    VOID = "VOID"


class CoolingDecision(StrEnum):
    NONE = "NONE"
    HOLD = "HOLD"
    CLAMP = "CLAMP"
    BYPASS = "BYPASS"


# ── Data Classes ─────────────────────────────────────────────────────────


@dataclass
class TriWitnessVotes:
    human: float = 0.42
    ai: float = 0.32
    earth: float = 0.26

    def to_dict(self) -> dict:
        return {"human": self.human, "ai": self.ai, "earth": self.earth}


@dataclass
class FlowReceiptEnvelope:
    """The receipt that arifFLOW understands natively."""

    receipt_id: str = field(default_factory=lambda: str(uuid.uuid4()))
    step_type: StepType = StepType.EXECUTE
    step_index: int = 0
    actor_id: str = ""
    session_id: str = ""
    organ: str = ""
    epistemic: EpistemicLabel = EpistemicLabel.OBS
    floor_verdict: FloorVerdict = FloorVerdict.PASS
    cooling: CoolingDecision = CoolingDecision.NONE
    tri_witness: TriWitnessVotes | None = None
    cost_ns: int = 0
    cost_type: str = "compute"  # compute | token | io | human
    summary: str = ""
    details: dict[str, Any] = field(default_factory=dict)
    parent_receipt_id: str | None = None
    chain_id: str | None = None
    lease_id: str | None = None
    timestamp_iso: str = field(default_factory=lambda: datetime.now(UTC).isoformat())
    sha256: str = ""

    def compute_sha256(self) -> str:
        content = json.dumps(
            {
                "receipt_id": self.receipt_id,
                "step_type": str(self.step_type),
                "step_index": self.step_index,
                "actor_id": self.actor_id,
                "session_id": self.session_id,
                "organ": self.organ,
                "summary": self.summary,
                "cost_ns": self.cost_ns,
                "parent_receipt_id": self.parent_receipt_id or "",
                "timestamp_iso": self.timestamp_iso,
            },
            sort_keys=True,
        )
        self.sha256 = hashlib.sha256(content.encode()).hexdigest()
        return self.sha256

    def to_ingest_dict(self) -> dict[str, Any]:
        """Convert to the format arifFLOW POST /ingest expects."""
        self.compute_sha256()
        d: dict[str, Any] = {
            "receipt_id": self.receipt_id,
            "step_type": str(self.step_type),
            "step_index": self.step_index,
            "actor_id": self.actor_id,
            "session_id": self.session_id,
            "organ": self.organ,
            "epistemic": str(self.epistemic),
            "floor_verdict": str(self.floor_verdict),
            "cooling": str(self.cooling),
            "cost_ns": self.cost_ns,
            "cost_type": self.cost_type,
            "summary": self.summary,
            "details": self.details,
            "parent_receipt_id": self.parent_receipt_id,
            "chain_id": self.chain_id,
            "lease_id": self.lease_id,
            "timestamp_iso": self.timestamp_iso,
            "sha256": self.sha256,
        }
        if self.tri_witness:
            d["tri_witness"] = self.tri_witness.to_dict()
        return d


# ── Client ───────────────────────────────────────────────────────────────


class ArifFlowClient:
    """HTTP client for arifFLOW daemon (:7073)."""

    def __init__(self, base_url: str = "http://127.0.0.1:7073", timeout: int = 5):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout

    def health(self) -> dict[str, Any]:
        """GET /health — returns FQ, receipt count, uptime."""
        r = requests.get(f"{self.base_url}/health", timeout=self.timeout)
        r.raise_for_status()
        return r.json()

    def ingest(self, receipt: FlowReceiptEnvelope | dict[str, Any]) -> dict[str, Any]:
        """POST /ingest — submit a receipt to arifFLOW."""
        if isinstance(receipt, FlowReceiptEnvelope):
            body = receipt.to_ingest_dict()
        else:
            body = receipt

        r = requests.post(
            f"{self.base_url}/ingest",
            json=body,
            timeout=self.timeout,
        )
        r.raise_for_status()
        return r.json()

    @property
    def fq(self) -> dict[str, Any]:
        """Get current Flow Quotient."""
        h = self.health()
        return h.get("fq", {})


# ── Convenience Functions ────────────────────────────────────────────────

# Module-level singleton (lazy init)
_client: ArifFlowClient | None = None


def get_client(base_url: str = "http://127.0.0.1:7073") -> ArifFlowClient:
    """Get or create the arifFLOW client singleton."""
    global _client
    if _client is None:
        _client = ArifFlowClient(base_url)
    return _client


def emit_receipt(
    step_type: str = "Execute",
    organ: str = "arifOS",
    actor_id: str = "",
    session_id: str = "",
    summary: str = "",
    epistemic: str = "OBS",
    floor_verdict: str = "PASS",
    cost_ns: int = 0,
    cost_type: str = "compute",
    parent_receipt_id: str | None = None,
    chain_id: str | None = None,
    lease_id: str | None = None,
    details: dict[str, Any] | None = None,
    client: ArifFlowClient | None = None,
) -> dict[str, Any]:
    """
    Emit a single receipt to arifFLOW.

    This is the ONE function every organ should call instead of generating
    receipts independently.  After P1, all 32 receipt sources converge here.

    Returns: dict with keys "status", "fq", "receipts"
    """
    c = client or get_client()

    receipt = FlowReceiptEnvelope(
        step_type=StepType(step_type),
        organ=organ,
        actor_id=actor_id,
        session_id=session_id,
        summary=summary,
        epistemic=EpistemicLabel(epistemic),
        floor_verdict=FloorVerdict(floor_verdict),
        cost_ns=cost_ns,
        cost_type=cost_type,
        parent_receipt_id=parent_receipt_id,
        chain_id=chain_id,
        lease_id=lease_id,
        details=details or {},
    )

    return c.ingest(receipt)


# ── PAIReceipt Bridge ────────────────────────────────────────────────────


def pai_to_flow_receipt(
    pai: dict[str, Any],
    step_type: str = "Execute",
    cost_ns: int = 0,
) -> FlowReceiptEnvelope:
    """
    Convert a PAIReceipt (arifOS canonical) to FlowReceipt (arifFLOW).

    This bridge ensures arifOS's canonical receipt schema maps cleanly
    into arifFLOW's execution record store without duplicating logic.
    """
    return FlowReceiptEnvelope(
        receipt_id=pai.get("receipt_id", str(uuid.uuid4())),
        step_type=StepType(step_type),
        actor_id=pai.get("actor_id", pai.get("producer_id", "")),
        session_id=pai.get("session_id", ""),
        organ=pai.get("organ", "arifOS"),
        epistemic=_map_epistemic(pai.get("truth_class", "OBS")),
        floor_verdict=_map_floor_verdict(pai.get("constitutional_verdict", "PASS")),
        cost_ns=cost_ns,
        summary=pai.get("summary", pai.get("intent", "")),
        details=pai.get("evidence", {}),
        chain_id=pai.get("constitutional_chain_id"),
        lease_id=pai.get("lease_id"),
        timestamp_iso=pai.get("timestamp_iso", datetime.now(UTC).isoformat()),
    )


def _map_epistemic(truth_class: str) -> EpistemicLabel:
    mapping = {
        "OBSERVED": EpistemicLabel.OBS,
        "OBS": EpistemicLabel.OBS,
        "DERIVED": EpistemicLabel.DER,
        "DER": EpistemicLabel.DER,
        "INTERPRETATION": EpistemicLabel.INT,
        "INT": EpistemicLabel.INT,
        "HYPOTHESIS": EpistemicLabel.SPEC,
        "SPEC": EpistemicLabel.SPEC,
        "SEAL": EpistemicLabel.SEAL_LABEL,
    }
    return mapping.get(truth_class.upper(), EpistemicLabel.OBS)


def _map_floor_verdict(verdict: str) -> FloorVerdict:
    mapping = {
        "SEAL": FloorVerdict.PASS,
        "PASS": FloorVerdict.PASS,
        "SABAR": FloorVerdict.CAUTION,
        "CAUTION": FloorVerdict.CAUTION,
        "HOLD": FloorVerdict.HOLD,
        "VOID": FloorVerdict.VOID,
    }
    return mapping.get(verdict.upper(), FloorVerdict.PASS)
