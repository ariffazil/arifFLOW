"""
arifFlow Python Client — Invariant Gate + Receipt Ingestion
============================================================

Wraps the arifFlow daemon (:7073) invariant enforcement endpoints
for use by AAA agents (333-AGI, 555-ASI, 888-APEX, A-FORGE, etc.).

Usage:
    from arifflow.client import ArifFlowClient

    client = ArifFlowClient()

    # Before executing: check invariant gate
    allowed, reason, action = client.check("333-AGI")
    if not allowed:
        print(f"HOLD: {reason}")
        return

    # After executing: ingest receipt
    client.ingest("333-AGI", "session-123", "Execute", "Observation", 1_000_000)

    # After verifying: ingest verify receipt + release
    client.ingest("333-AGI", "session-123", "Verify", "Derivation", 500_000)
    client.release("333-AGI")

    # Trigger enforcement cycle
    client.enforce()

DITEMPA BUKAN DIBERI
"""

from __future__ import annotations

import json
import os
from dataclasses import dataclass
from typing import Optional
from urllib.request import Request, urlopen


@dataclass
class CheckResult:
    actor: str
    allowed: bool
    reason: str
    action: str  # "Allow" | "Throttle" | "Hold" | "Void"


@dataclass
class IngestResult:
    actor: str
    status: str
    fq: float
    fq_verdict: str


class ArifFlowClient:
    """Client for arifFlow daemon invariant enforcement + receipt ingestion."""

    def __init__(self, base_url: str = "http://127.0.0.1:7073"):
        self.base_url = base_url.rstrip("/")

    def _post(self, path: str, data: dict) -> dict:
        """Send a POST request to the arifFlow daemon."""
        url = f"{self.base_url}{path}"
        body = json.dumps(data).encode("utf-8")
        req = Request(url, data=body, headers={"Content-Type": "application/json"})
        try:
            with urlopen(req, timeout=5) as resp:
                return json.loads(resp.read())
        except Exception as e:
            # arifFlow unreachable — FAIL CLOSED (constitutional: governance unavailable, do not proceed)
            # Emergency override: set ARIFLOW_FAIL_OPEN=true to bypass (e.g., arifFlow daemon down for maintenance)
            fail_open = os.environ.get("ARIFLOW_FAIL_OPEN", "").lower() in (
                "1",
                "true",
                "yes",
            )
            if fail_open:
                return {
                    "status": "error",
                    "error": str(e),
                    "allowed": True,
                    "reason": "arifFlow unreachable (fail-open override active)",
                    "action": "Allow",
                }
            return {
                "status": "error",
                "error": str(e),
                "allowed": False,
                "reason": "arifFlow unreachable",
                "action": "Block",
            }

    def check(self, actor_id: str) -> CheckResult:
        """Check if an actor is allowed to execute.

        Agents MUST call this before any mutation action.
        Returns CheckResult with allowed=False if blocked by invariants.
        """
        data = self._post("/check", {"actor_id": actor_id})
        return CheckResult(
            actor=data.get("actor", actor_id),
            allowed=data.get("allowed", False),
            reason=data.get("reason", "unknown"),
            action=data.get("action", "Allow"),
        )

    def ingest(
        self,
        actor_id: str,
        session_id: str,
        step_type: str,
        epistemic_label: str,
        cost_ns: int,
        floor_verdict: str = "Pass",
        cooling_decision: str = "None",
        payload: Optional[dict] = None,
    ) -> IngestResult:
        """Ingest a flow receipt into arifFlow.

        Called after every Execute or Verify step.
        """
        import uuid
        from datetime import datetime, timezone

        receipt = {
            "receipt_id": str(uuid.uuid4()),
            "actor_id": actor_id,
            "session_id": session_id,
            "step_type": step_type,
            "epistemic_label": epistemic_label,
            "cost_ns": cost_ns,
            "step_number": 1,  # will be tracked by daemon
            "created_at": datetime.now(timezone.utc).isoformat(),
            "floor_verdict": floor_verdict,
            "cooling_decision": cooling_decision,
        }
        if payload:
            receipt["payload"] = payload

        data = self._post("/ingest", receipt)
        fq = data.get("fq", {})
        return IngestResult(
            actor=data.get("actor", actor_id),
            status=data.get("status", "unknown"),
            fq=fq.get("quotient", 0.0),
            fq_verdict=fq.get("verdict", "UNKNOWN"),
        )

    def release(self, actor_id: str) -> dict:
        """Release a hold on an actor after verification."""
        return self._post("/release", {"actor_id": actor_id})

    def enforce(self) -> dict:
        """Manually trigger the invariant enforcement cycle."""
        return self._post("/enforce", {})

    def health(self) -> dict:
        """Get arifFlow health status including invariants."""
        url = f"{self.base_url}/health"
        req = Request(url)
        try:
            with urlopen(req, timeout=5) as resp:
                return json.loads(resp.read())
        except Exception as e:
            return {"status": "error", "error": str(e)}


# ── Convenience functions ────────────────────────────────────────────────

_default_client: Optional[ArifFlowClient] = None


def get_client() -> ArifFlowClient:
    """Get or create a default client instance."""
    global _default_client
    if _default_client is None:
        _default_client = ArifFlowClient()
    return _default_client


def check(actor_id: str) -> CheckResult:
    """Convenience: check if actor is allowed to execute."""
    return get_client().check(actor_id)


def ingest(
    actor_id: str,
    session_id: str,
    step_type: str,
    epistemic_label: str,
    cost_ns: int,
    **kwargs,
) -> IngestResult:
    """Convenience: ingest a flow receipt."""
    return get_client().ingest(
        actor_id, session_id, step_type, epistemic_label, cost_ns, **kwargs
    )


def release(actor_id: str) -> dict:
    """Convenience: release hold on actor."""
    return get_client().release(actor_id)
