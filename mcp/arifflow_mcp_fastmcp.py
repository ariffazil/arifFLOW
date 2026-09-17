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
    result = _flow_get("/health")
    result.setdefault("provenance", {})
    result["provenance"].setdefault("formula_version", "qg.v0.2")
    result["provenance"].setdefault(
        "formula_hash", "sha256:arifflow-fq-v2.2-2026-08-14"
    )
    result["provenance"].setdefault(
        "missing_inputs",
        ["window_duration_s", "apex_block", "flow_block", "projection_block"],
    )
    return result


@mcp.tool()
def flow_entity_report() -> dict[str, Any]:
    """Entity-classified FQ report. Classifies actors by type (human_agent,
    interactive_session, daemon, infrastructure, synthetic) and computes
    governance-weighted FQ from consequence-bearing actors only.
    Doctrine: E4 (not all receipts are reality), E6 (actors are not equal)."""
    health = _flow_get("/health")
    if "error" in health:
        return health
    fq_data = health.get("fq", {})
    per_actor = fq_data.get("per_actor", {})
    classified: dict[str, dict] = {}
    class_totals: dict[str, dict] = {}
    for actor_id, data in per_actor.items():
        # RG-CC (2026-09-17 FI-008): case-insensitive lookup. Doctrine forbids
        # case-variant copies (anti-entropy rules); YAML keeps native case
        # for human readability; runtime lookup normalizes.
        entity_class = _ENTITY_CLASSES.get(actor_id)
        if entity_class is None:
            for _k, _v in _ENTITY_CLASSES.items():
                if _k.lower() == actor_id.lower():
                    entity_class = _v
                    break
        if entity_class is None:
            entity_class = "unknown"
        consequence_bearing = entity_class in ("human_agent", "interactive_session")
        entry = {
            "entity_class": entity_class,
            "execute": data.get("execute", 0),
            "verify": data.get("verify", 0),
            "quotient": data.get("quotient"),
            "held": data.get("held", False),
            "diagnosis": data.get("diagnosis", "?"),
            "verdict": data.get("verdict", "?"),
            "consequence_bearing": consequence_bearing,
        }
        classified[actor_id] = entry
        if entity_class not in class_totals:
            class_totals[entity_class] = {"execute": 0, "verify": 0, "actors": 0}
        class_totals[entity_class]["execute"] += entry["execute"]
        class_totals[entity_class]["verify"] += entry["verify"]
        class_totals[entity_class]["actors"] += 1
    gov_exec = sum(
        d["execute"] for d in classified.values() if d["consequence_bearing"]
    )
    gov_ver = sum(d["verify"] for d in classified.values() if d["consequence_bearing"])
    gov_fq = (gov_ver / gov_exec) if gov_exec > 0 else None
    if gov_fq is None:
        gov_verdict = "UNKNOWN"
    elif gov_fq < 0.1:
        gov_verdict = "BURNING"
    elif gov_fq < 0.5:
        gov_verdict = "STUCK"
    elif gov_fq < 2.0:
        gov_verdict = "FLOWING"
    else:
        gov_verdict = "FOSSILIZED"
    return {
        "raw_fq": fq_data.get("quotient"),
        "raw_verdict": fq_data.get("verdict"),
        "governance_weighted_fq": round(gov_fq, 4) if gov_fq else None,
        "governance_verdict": gov_verdict,
        "governance_execute": gov_exec,
        "governance_verify": gov_ver,
        "entity_classes": class_totals,
        "actors": classified,
        "classification_source": _ENTITY_CLASS_FILE,
        "note": "Only human_agent + interactive_session contribute to governance FQ",
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
    parent_receipt_ids: list[str] | None = None,
    parent_receipt_hashes: list[str] | None = None,
    jcs_body_hash: str | None = None,
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
    # RG-OM (2026-09-17 FI-008): parent_receipt_ids mandatory unless explicit
    # top_level_intent. F2: orphan ratio 71% (5/7 events) in arifFlow ledger.
    # Lineage CAN reconstruct chains when parents set (2207231e verified
    # 5-deep across arifOS→arifFlow→AAA); defect is upstream actors skipping
    # parent at receipt creation, not in lineage system.
    payload_dict = payload if isinstance(payload, dict) else {}
    top_level_claim = bool(payload_dict.get("top_level_intent"))
    if not parent_receipt_ids and not top_level_claim:
        return {
            "http_status": 400,
            "error": "PARENT_REQUIRED: pass parent_receipt_ids OR set payload.top_level_intent=true with reason",
            "remediation": "Pass parent_receipt_ids=[...] or declare top_level_intent=True",
        }
    if parent_receipt_ids:
        receipt["parent_receipt_ids"] = parent_receipt_ids
    if parent_receipt_hashes:
        receipt["parent_receipt_hashes"] = parent_receipt_hashes
    if jcs_body_hash:
        receipt["jcs_body_hash"] = jcs_body_hash
    if payload:
        receipt["payload"] = payload
    return _flow_post("/ingest", receipt)


@mcp.tool()
def flow_fq_g() -> dict[str, Any]:
    """FQ_G — institutional metabolism rate (read-only). Counts beliefs
    born/superseded, governance events, scar-bound policies, reality invoices;
    computes revision_rate, invoice_yield, and latencies scar→policy,
    policy→invoice, belief lifetime. v1 distributions only — thresholds not
    invented (measure-first doctrine)."""
    return _flow_post("/fq_g", {})


@mcp.tool()
def flow_consequences(
    before_receipt_id: str | None = None,
) -> dict[str, Any]:
    """RG-7 consequence records (read-only). Reality's invoices: observed
    outcomes ATTRIBUTED to the policy/receipts that produced them.
    outcome_class recovery|regression|neutral; evidence keeps attribution
    falsifiable. 'Did belief change reality?' — traversable."""
    body: dict[str, Any] = {}
    if before_receipt_id:
        body["before_receipt_id"] = before_receipt_id
    return _flow_post("/consequences", body)


@mcp.tool()
def flow_scar_policies(
    before_receipt_id: str | None = None,
) -> dict[str, Any]:
    """RG-5 scar-bound policy query (read-only). Policies compressed from
    scars: slug, scar id, enforcement surface+ref, causal parents, and
    supersession status. 'Did reality change future behaviour?' — traversable."""
    body: dict[str, Any] = {}
    if before_receipt_id:
        body["before_receipt_id"] = before_receipt_id
    return _flow_post("/scar_policies", body)


@mcp.tool()
def flow_gov_events(
    before_receipt_id: str | None = None,
) -> dict[str, Any]:
    """RG-4 governance-event query (read-only). Seal/seal_refused/bind_failed
    events with verdict, chain_id, judge_state_hash, and supersession status.
    Optional before_receipt_id = time travel."""
    body: dict[str, Any] = {}
    if before_receipt_id:
        body["before_receipt_id"] = before_receipt_id
    return _flow_post("/gov_events", body)


@mcp.tool()
def flow_lineage(
    receipt_id: str,
    before_receipt_id: str | None = None,
) -> dict[str, Any]:
    """SEQ-N belief-lineage query (read-only). Reconstruct causal ancestry of
    a receipt with per-edge hash verification, supersession status (belief
    death), and optional time travel: with before_receipt_id, only receipts at
    or before that ledger position exist — 'what did we believe then, and why?'"""
    body: dict[str, Any] = {"receipt_id": receipt_id}
    if before_receipt_id:
        body["before_receipt_id"] = before_receipt_id
    return _flow_post("/lineage", body)


if __name__ == "__main__":
    mcp.run(transport="http", host="127.0.0.1", port=7075)
