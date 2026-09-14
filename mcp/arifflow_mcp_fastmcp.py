"""
arifFlow MCP Server — FastMCP (replaces stdio shim arifflow-mcp.py)

arifFlow is METABOLISM — it routes, checkpoints, and witnesses.
It never judges (arifOS) and never executes (A-FORGE).
FQ = verify/execute ratio — the metabolic signal that intelligence
is flowing, not just burning.

Tools:
  flow_health       — GET /health: FQ, verdict, receipt count, uptime
  flow_entity_report — Entity-classified FQ report
  flow_ingest       — POST /ingest: mint + submit a FlowReceipt

DITEMPA BUKAN DIBERI — Forged, Not Given.
"""

from __future__ import annotations

import json
import socket
import uuid
from datetime import datetime, timezone
from typing import Any

from fastmcp import FastMCP

FLOW_HOST = "127.0.0.1"
FLOW_PORT = 7073

STEP_TYPES = ["Execute", "Verify", "Cool", "Seal", "Barrier", "Merge", "Route"]
EPISTEMIC = ["Observation", "Derivation", "Interpretation", "Specification", "Seal"]
VERDICTS = ["Pass", "Caution", "Hold", "Void"]

# Entity classification — loaded once at startup
_ENTITY_CLASSES: dict[str, str] = {}
_ENTITY_CLASS_FILE = "/root/arifFlow/config/entity_classes.yaml"
try:
    import yaml as _yaml

    with open(_ENTITY_CLASS_FILE) as _f:
        _raw = _yaml.safe_load(_f) or {}
    for _cls, _actors in _raw.items():
        if isinstance(_actors, list):
            for _a in _actors:
                _ENTITY_CLASSES[str(_a)] = _cls
except Exception:
    pass

mcp = FastMCP(
    name="arifflow-mcp",
    version="2026.09.14",
    instructions=(
        "arifFlow — Metabolism organ for arifOS Federation. "
        "Authority: METABOLIZE_ONLY. FQ monitoring, receipt ingestion. "
        "Routes, checkpoints, and witnesses. Never judges, never executes."
    ),
)


def _flow_get(path: str) -> dict[str, Any]:
    """HTTP GET to arifFlow daemon."""
    import urllib.request
    import urllib.error

    url = f"http://{FLOW_HOST}:{FLOW_PORT}{path}"
    try:
        with urllib.request.urlopen(url, timeout=10) as resp:
            return json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        return {"error": str(e), "status_code": e.code}
    except urllib.error.URLError as e:
        return {"error": f"arifFlow daemon unreachable: {e.reason}"}


def _flow_post(path: str, body: dict) -> dict[str, Any]:
    """HTTP POST to arifFlow daemon via raw socket (matches existing shim pattern)."""
    payload = json.dumps(body)
    request = (
        f"POST {path} HTTP/1.1\r\n"
        f"Host: {FLOW_HOST}:{FLOW_PORT}\r\n"
        f"Content-Type: application/json\r\n"
        f"Content-Length: {len(payload)}\r\n"
        f"Connection: close\r\n"
        f"\r\n"
        f"{payload}"
    )
    try:
        with socket.create_connection((FLOW_HOST, FLOW_PORT), timeout=5) as sock:
            sock.sendall(request.encode())
            response = b""
            while True:
                chunk = sock.recv(4096)
                if not chunk:
                    break
                response += chunk
            # Parse HTTP response
            header_end = response.find(b"\r\n\r\n")
            if header_end == -1:
                return {"error": "malformed response"}
            body_bytes = response[header_end + 4 :]
            return json.loads(body_bytes.decode())
    except Exception as e:
        return {"error": f"arifFlow POST failed: {e}"}


@mcp.tool()
def flow_health() -> dict[str, Any]:
    """arifFlow daemon health + Flow Quotient (FQ = verify/execute ratio over
    recent receipts). Verdicts: FLOWING (healthy metabolism), STUCK (no
    verification), BURNING (execution outruns verification). Read-only."""
    return _flow_get("/health")


@mcp.tool()
def flow_entity_report() -> dict[str, Any]:
    """Entity-classified FQ report. Classifies actors by type (human_agent,
    interactive_session, daemon, infrastructure, synthetic) and computes
    governance-weighted FQ from consequence-bearing actors only.
    Doctrine: E4 (not all receipts are reality), E6 (actors are not equal)."""
    health = _flow_get("/health")
    if "error" in health:
        return health
    per_actor = health.get("fq", {}).get("per_actor", {})
    classified: dict[str, list[dict]] = {}
    for actor, data in per_actor.items():
        entity_class = _ENTITY_CLASSES.get(actor, "unknown")
        classified.setdefault(entity_class, []).append(
            {"actor": actor, **data}
        )
    return {
        "entity_classes": classified,
        "total_actors": len(per_actor),
        "class_distribution": {k: len(v) for k, v in classified.items()},
    }


@mcp.tool()
def flow_ingest(
    actor_id: str,
    session_id: str,
    step_type: str = "Execute",
    step_number: int = 1,
    cost_ns: int = 0,
    epistemic_label: str = "Derivation",
    floor_verdict: str = "Pass",
    topology_id: str | None = None,
    lane_id: int | None = None,
    previous_receipt_hash: str | None = None,
    witness_organs: list[str] | None = None,
    routed_organ: str | None = None,
    session_token: str | None = None,
    harness_fingerprint: str | None = None,
    payload: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Mint and ingest a FlowReceipt into the arifFlow metabolic ledger
    (POST /ingest). Records one governed step: identity, step_type, cost,
    epistemic label, and floor verdict. Use to checkpoint work so FQ
    monitoring and cooling correlation see it. Returns FQ after ingest."""
    receipt: dict[str, Any] = {
        "receipt_id": str(uuid.uuid4()),
        "created_at": datetime.now(timezone.utc).isoformat(),
        "actor_id": actor_id,
        "session_id": session_id,
        "step_type": step_type,
        "step_number": step_number,
        "cost_ns": cost_ns,
        "epistemic_label": epistemic_label,
        "floor_verdict": floor_verdict,
        "cooling_decision": "None",
    }
    if topology_id:
        receipt["topology_id"] = topology_id
    if lane_id is not None:
        receipt["lane_id"] = lane_id
    if previous_receipt_hash:
        receipt["previous_receipt_hash"] = previous_receipt_hash
    if witness_organs:
        receipt["witness_organs"] = witness_organs
    if routed_organ:
        receipt["routed_organ"] = routed_organ
    if session_token:
        receipt["session_token"] = session_token
    if harness_fingerprint:
        receipt["harness_fingerprint"] = harness_fingerprint
    if payload:
        receipt["payload"] = payload
    return _flow_post("/ingest", receipt)


if __name__ == "__main__":
    mcp.run(transport="http", host="127.0.0.1", port=7075)
