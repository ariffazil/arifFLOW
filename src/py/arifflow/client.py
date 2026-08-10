"""
arifFlow Python Client — Invariant Gate + Receipt Ingestion
============================================================

Wraps the arifFlow daemon (:7073) invariant enforcement endpoints
for use by AAA agents (333-AGI, 555-ASI, 888-APEX, A-FORGE, etc.).

T1-1 Override Closure (2026-08-10):
When ARIFLOW_FAIL_OPEN fires, an OVERRIDE_RECEIPT is emitted to
/var/lib/arifflow/override_log.jsonl. Override is an observability
event, not a configuration event.

DITEMPA BUKAN DIBERI
"""

from __future__ import annotations

import datetime
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

    def _emit_override_receipt(self, error: str) -> None:
        """T1-1: Emit governance receipt when fail-open override is used.

        Every bypass MUST leave an audit trail: actor, reason, expiry, timestamp.
        Written to override_log.jsonl — same persistence dir as daemon receipts.
        """
        try:
            now = datetime.datetime.now(datetime.timezone.utc)
            expiry = now + datetime.timedelta(minutes=30)
            receipt = {
                "event": "OVERRIDE_RECEIPT",
                "timestamp": now.isoformat(),
                "expiry": expiry.isoformat(),
                "actor": "arifflow-client",
                "reason": f"ARIFLOW_FAIL_OPEN bypass: {error}",
                "override_type": "ARIFLOW_FAIL_OPEN",
                "status": "ACTIVE",
                "expires_at": expiry.isoformat(),
            }
            log_path = "/var/lib/arifflow/override_log.jsonl"
            os.makedirs(os.path.dirname(log_path), exist_ok=True)
            with open(log_path, "a") as f:
                f.write(json.dumps(receipt) + "\n")
        except Exception:
            pass  # logging failure must not block the override

    def _post(self, path: str, data: dict) -> dict:
        """Send a POST request to the arifFlow daemon.

        On daemon unreachable:
        - FAIL CLOSED by default (allowed=False, action=Hold)
        - ARIFLOW_FAIL_OPEN=true overrides to allowed=True AND emits
          an override governance receipt (T1-1: observability event).
        """
        url = f"{self.base_url}{path}"
        body = json.dumps(data).encode("utf-8")
        req = Request(url, data=body, headers={"Content-Type": "application/json"})
        try:
            with urlopen(req, timeout=5) as resp:
                return json.loads(resp.read())
        except Exception as e:
            # arifFlow unreachable — FAIL CLOSED (governance unavailable, do not proceed)
            # Emergency override: ARIFLOW_FAIL_OPEN=true bypasses AND emits governance receipt
            fail_open = os.environ.get("ARIFLOW_FAIL_OPEN", "").lower() in (
                "1", "true", "yes",
            )
            if fail_open:
                # T1-1: Override is an OBSERVABILITY EVENT, not a config event.
                self._emit_override_receipt(str(e))
                return {
                    "status": "override",
                    "error": str(e),
                    "allowed": True,
                    "reason": "arifFlow unreachable (fail-open override active)",
                    "action": "Allow",
                }
            return {
                "status": "error",
                "error": str(e),
                "allowed": False,
                "reason": "arifFlow unreachable — governance unavailable (fail-closed)",
                "action": "Hold",
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
            "step_number": 1,
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
