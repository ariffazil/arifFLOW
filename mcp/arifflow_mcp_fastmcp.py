"""
arifFlow MCP Server — FastMCP (replaces stdio shim arifflow-mcp.py)

arifFlow is METABOLISM — it routes, checkpoints, and witnesses.
It never judges (arifOS) and never executes (A-FORGE).
FQ = verify/execute ratio — the metabolic signal that intelligence
is flowing, not just burning.

THREE GRAMMARS, THREE OWNERS (F13 GRAMMAR_DOCTRINE — 2026-11-22)
  arifFlow  metabolic_gate_state : ALLOW | FLOW_BLOCKED | THROTTLED | COOLDOWN
  arifOS    constitutional_verdict: SEAL | HOLD | SABAR | VOID   ← NEVER from here
  observed  observed_diagnosis   : FLOWING | STUCK | BURNING | COHERENCE_FRACTURE
A caller of this server speaks the FIRST vocabulary only. If you find yourself
writing "Hold", "SEAL" or "VOID" into a flow receipt, you are borrowing
arifOS's authority — the receipt will refuse to be an authority.

WHAT FQ MEANS (metric_semantics — read before quoting a number)
  FQ = verify_count / execute_count over the ACTOR'S OWN recent receipt window
  (window 100 samples; formula qg.v0.3.1-vector). HIGHER IS HEALTHIER:
  it means verification is keeping pace with execution.
  It is NOT a verdict, NOT a score of a human, and NOT a claim that anything
  is correct. It measures metabolism only.
  Machine and human actors are scored separately (per_actor); the federation
  rollup is a gated harmonic mean, so one starved actor can pull it down.
  excluded_verify_count = verifies that were NOT counted in the numerator
  (e.g. session-orphan self-verifies). Absence of a number is not a zero.
  The `shadow` block is a READ-ONLY divergence projection (shadow vs enforced
  rule set). It cannot hold, throttle, release, admit or judge. Nothing in it
  is enforcement.

  LIVE-SURFACE CAVEAT (measured 2026-11-22): the running daemon binary
  (5e18663f) predates the SOT manifest / metric_semantics / gate_state
  release (5b503b0) and carries the shadow frame only. Some fields documented
  here are in SOURCE but not yet in the running binary. Probe /health and read
  what is actually present; do not assume a field exists because it is
  documented here.

Tools:
  flow_health       — GET /health: FQ, verdict, receipt count, uptime, shadow frame
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

# ── Grammar boundary: arifFlow speaks its OWN vocabulary (F13 GRAMMAR_DOCTRINE) ──
# The daemon's receipt schema still carries a legacy `floor_verdict` field whose
# "Hold" variant is documented in Rust as "888_HOLD" and "Void" as a permanent
# critical block. Those are arifOS's constitutional words. arifFlow measures and
# reports; it does not adjudicate. So callers mint arifFlow's gate state, and a
# legacy verdict is TRANSLATED, never trusted as authority.
METABOLIC_GATE_STATES = ("ALLOW", "FLOW_BLOCKED", "THROTTLED", "COOLDOWN")
_LEGACY_FLOOR_TO_GATE = {
    "pass": "ALLOW",
    "caution": "THROTTLED",
    "hold": "FLOW_BLOCKED",
    "void": "FLOW_BLOCKED",
}
_GATE_TO_LEGACY_FLOOR = {
    "ALLOW": "Pass",
    "THROTTLED": "Caution",
    "FLOW_BLOCKED": "Hold",
    "COOLDOWN": "Hold",
}


def _resolve_gate_state(
    metabolic_gate_state: str | None, floor_verdict: str | None
) -> tuple[str, str]:
    """Return (metabolic_gate_state, legacy_floor_verdict) for a receipt.

    A valid metabolic gate state always wins. A legacy floor verdict is accepted
    only as a translation source so existing callers keep working — it is never
    stored verbatim, because storing it verbatim would let a caller write
    arifOS's judgement into the metabolism ledger.
    """
    gate = str(metabolic_gate_state or "").strip().upper()
    legacy = str(floor_verdict or "").strip().lower()
    if gate not in METABOLIC_GATE_STATES:
        gate = _LEGACY_FLOOR_TO_GATE.get(legacy, "ALLOW")
    return gate, _GATE_TO_LEGACY_FLOOR.get(gate, "Pass")
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
    """arifFlow daemon health + Flow Quotient, with the metric semantics attached.

    FQ = verify_count / execute_count over the actor's own recent receipt window
    (100 samples, formula qg.v0.3.1-vector). HIGHER IS HEALTHIER — verification
    keeping pace with execution. It is a METABOLIC measurement, never a verdict,
    never a claim about a human, never proof that anything is correct.

    Three grammars, three owners — read the value in the right column:
      observed_diagnosis (FLOWING | STUCK | BURNING | COHERENCE_FRACTURE)
        — what the metabolism looks like. OBSERVED, not judged.
      metabolic_gate_state (ALLOW | FLOW_BLOCKED | THROTTLED | COOLDOWN)
        — arifFlow's own gate, on POST /check. arifFlow's vocabulary.
      constitutional_verdict (SEAL | HOLD | SABAR | VOID)
        — arifOS only. arifFlow never emits it.

    `per_actor` scores machine and human actors separately; `excluded_verify_count`
    counts verifies kept out of the numerator (absence of a value is not a zero).
    `shadow` is a READ-ONLY divergence projection (shadow rule set vs enforced
    rule set) — it cannot hold, throttle, release, admit or judge.

    Live-surface caveat (measured 2026-11-22): the running binary predates the
    SOT/metric_semantics/gate_state release, so `sot`, `metric_semantics` and
    `gate_state` may be absent from the payload. Probe it; do not assume.
    Read-only."""
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
            # Organ-scoped key fallback (333-AGI, 2026-10-28): arifOS records
            # receipts as `<organ>:<actor>` while the registry declares the bare
            # `<actor>`. Without this, a correct declaration is INERT and a
            # probe actor recorded under an organ prefix is reported as a real
            # consequence-bearing actor.
            #
            # SAFETY: exact match runs first, then case-insensitive, so no actor
            # can change from one declared class to another. The fallback only
            # makes an actor whose BARE form is already declared reachable; the
            # only reachable change is unknown -> declared. An undeclared bare
            # form stays "unknown". Mirrors the Rust aggregate's
            # `entity_class.rs::classify` (commit ad79fe4) so the report and the
            # aggregate can no longer disagree.
            _bare = actor_id.split(":", 1)[1] if ":" in actor_id else actor_id
            for _k, _v in _ENTITY_CLASSES.items():
                if _k.lower() == _bare.lower():
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
    # Void guard: "unknown" is not "all clear". An unclassified actor is excluded
    # from governance FQ by default, so an unreported residue is a silent hole in
    # who counts. Report the residue and its volume, do not bury it in a default.
    unknown = sorted(
        (
            (v["execute"] + v["verify"], k)
            for k, v in classified.items()
            if v["entity_class"] == "unknown"
        ),
        reverse=True,
    )
    counted = sum(
        v["execute"] + v["verify"]
        for v in classified.values()
        if v["consequence_bearing"]
    )
    seen_total = sum(v["execute"] + v["verify"] for v in classified.values())
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
        "governance_weighted_fq": round(gov_fq, 4) if gov_fq is not None else None,
        "governance_verdict": gov_verdict,
        "governance_execute": gov_exec,
        "governance_verify": gov_ver,
        "entity_classes": class_totals,
        "classification_coverage": {
            "actors_seen": len(classified),
            "actors_unclassified": len(unknown),
            "unclassified_volume": sum(n for n, _ in unknown),
            "volume_basis": seen_total,
            "consequence_bearing_volume": counted,
            "top_unclassified": [k for _n, k in unknown[:8]],
            "note": "unclassified == excluded from governance FQ by default, not by decision",
        },
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
    metabolic_gate_state: str = "ALLOW",
    floor_verdict: str | None = None,
    topology_id: str | None = None,
    lane_id: int | None = None,
    previous_receipt_hash: str | None = None,
    witness_organs: Any = None,
    routed_organ: str | None = None,
    session_token: str | None = None,
    harness_fingerprint: str | None = None,
    parent_receipt_ids: Any = None,
    parent_receipt_hashes: Any = None,
    jcs_body_hash: str | None = None,
    supersedes_receipt_ids: Any = None,
    supersedes_receipt_hashes: Any = None,
    payload: Any = None,
) -> dict[str, Any]:
    """Mint and ingest a FlowReceipt into the arifFlow metabolic ledger
    (POST /ingest). Records one governed step: identity, step_type, cost,
    epistemic label, and arifFlow's OWN gate state. Use to checkpoint work so FQ
    monitoring and cooling correlation see it. Returns FQ after ingest.

    GRAMMAR: `metabolic_gate_state` is arifFlow's vocabulary —
    ALLOW | FLOW_BLOCKED | THROTTLED | COOLDOWN. Do NOT pass "SEAL", "HOLD",
    "SABAR" or "VOID" here: those are arifOS's constitutional verdicts and
    arifFlow does not adjudicate. A legacy `floor_verdict` (Pass|Caution|Hold|Void)
    is still accepted for backwards compatibility and is TRANSLATED into the
    metabolic vocabulary before it is stored, so a caller cannot write
    arifOS's judgement into the metabolism ledger.

    LINEAGE IS MANDATORY (daemon rule, measured 2026-11-22): the daemon rejects
    a receipt that declares no ancestry with
    `PARENT_REQUIRED: pass parent_receipt_ids OR set payload.top_level_intent=true
    with reason`. Pass `parent_receipt_ids=[...]` when the step descends from an
    earlier receipt, otherwise declare `payload.top_level_intent=true` with a
    `reason`.

    LEDGER FIELD (measured 2026-11-22): the running daemon's receipt schema
    still stores only the legacy `floor_verdict` key - serde drops the extra
    `metabolic_gate_state` key. So today the GATE STATE YOU ASK FOR IS
    TRANSLATED INTO THE LEGACY FIELD (ALLOW->Pass, THROTTLED->Caution,
    FLOW_BLOCKED/COOLDOWN->Hold) and the semantic meaning is arifFlow's gate,
    not a constitutional verdict. Carrying the field natively needs a Rust
    change to the FlowReceipt struct, plus rebuild and deploy of the daemon.

    supersedes_* records a SEQ-N belief death: this receipt retires the cited
    receipt(s). The daemon already accepts these fields on /ingest; the bridge
    exposed parent_* but not supersedes_*, which left agents on the governed
    path structurally unable to retract a claim."""
    # Boundary adapter (2026-10-20): some MCP clients stringify structured
    # args, so {"a":1} arrives as the STRING '{"a":1}' and pydantic rejects it
    # before this body runs. Annotated as Any so FastMCP still registers the
    # tool; coerced here so the governed write path is actually reachable.
    def _as_list(v):
        if v is None:
            return None
        if isinstance(v, list):
            return [str(x) for x in v]
        if isinstance(v, str):
            t = v.strip()
            if t.startswith("["):
                try:
                    parsed = json.loads(t)
                    if isinstance(parsed, list):
                        return [str(x) for x in parsed]
                except Exception:
                    pass
            return [t] if t else []
        return None

    def _as_dict(v):
        if isinstance(v, dict):
            return v
        if isinstance(v, str):
            try:
                parsed = json.loads(v)
                if isinstance(parsed, dict):
                    return parsed
            except Exception:
                pass
        return {}

    witness_organs = _as_list(witness_organs)
    parent_receipt_ids = _as_list(parent_receipt_ids)
    parent_receipt_hashes = _as_list(parent_receipt_hashes)
    supersedes_receipt_ids = _as_list(supersedes_receipt_ids)
    supersedes_receipt_hashes = _as_list(supersedes_receipt_hashes)
    payload = _as_dict(payload)

    gate_state, legacy_floor = _resolve_gate_state(metabolic_gate_state, floor_verdict)
    receipt: dict[str, Any] = {
        "receipt_id": str(uuid.uuid4()),
        "created_at": datetime.now(timezone.utc).isoformat(),
        "actor_id": actor_id,
        "session_id": session_id,
        "step_type": step_type,
        "step_number": step_number,
        "cost_ns": cost_ns,
        "epistemic_label": epistemic_label,
        # arifFlow's own gate state — the authoritative field for this ledger.
        "metabolic_gate_state": gate_state,
        # Legacy schema field (daemon struct: FloorVerdict). Kept for
        # compatibility and DERIVED from the gate state, never caller-verbatim.
        "floor_verdict": legacy_floor,
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
    if supersedes_receipt_ids:
        if supersedes_receipt_hashes and len(supersedes_receipt_hashes) != len(supersedes_receipt_ids):
            return {
                "http_status": 400,
                "error": "SUPERSESSION_ARITY: supersedes_receipt_hashes must be 1:1 with supersedes_receipt_ids",
                "remediation": "Pass one parent hash per superseded receipt id, or omit hashes",
            }
        receipt["supersedes_receipt_ids"] = supersedes_receipt_ids
        if supersedes_receipt_hashes:
            receipt["supersedes_receipt_hashes"] = supersedes_receipt_hashes
    elif supersedes_receipt_hashes:
        return {
            "http_status": 400,
            "error": "SUPERSESSION_ARITY: supersedes_receipt_hashes given without supersedes_receipt_ids",
            "remediation": "Pass supersedes_receipt_ids=[...] alongside the hashes",
        }
    if jcs_body_hash:
        receipt["jcs_body_hash"] = jcs_body_hash
    if payload:
        receipt["payload"] = payload
    resp = _flow_post("/ingest", receipt)
    # Response-layer noise removal (2026-10-20). These keys are computed on the
    # daemon's /ingest RESPONSE, not stored on the receipt, so dropping empty
    # values here cannot touch compute_jcs_body_hash or any sealed receipt.
    # explanation_gate is CM-1 (2026-09-19) and stays in the engine — it stays
    # visible whenever it actually fires. We only stop printing its
    # NOT_APPLICABLE shell with four nulls on every single receipt.
    if isinstance(resp, dict):
        gate = resp.get("explanation_gate")
        if isinstance(gate, dict):
            # Drop only the N/A SHELL: outcome says not-applicable AND every
            # other field is empty. A gate that actually fired (class/code set,
            # outcome anything else) must stay visible — that one carries meaning.
            others = {k: v for k, v in gate.items() if k != "outcome"}
            dormant = (gate.get("outcome") == "NOT_APPLICABLE" and
                       not any(v not in (None, "", [], {}) for v in others.values()))
            if dormant:
                resp.pop("explanation_gate", None)
        prov = resp.get("provenance")
        if isinstance(prov, dict):
            for k in list(prov):
                if prov[k] in (None, "", [], {}):
                    prov.pop(k, None)
            if not prov:
                resp.pop("provenance", None)
    return resp


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


@mcp.custom_route("/health", methods=["GET"])
async def _health_route(request):
    """Plain-HTTP liveness / contract / capability surface for the arifFlow organ.

    The tool count belongs HERE and not on the Rust daemon's :7073/health: this
    process owns the @mcp.tool() registrations. A daemon publishing a count for a
    surface it does not serve would be exactly the fabricated-capability class the
    observatory stopped publishing on 2026-10-03 (SCAR-OBS-GREENWASH).

    The count is read from the live FastMCP registry on every request, so it
    cannot drift from what the MCP wire actually exposes. If the registry cannot
    be read, tools_loaded is null and the reason is published — never a literal
    fallback count.
    
    Boundary: these records are EVIDENCE of what happened, never authority to act.
    arifFlow observes and anchors; arifOS adjudicates (SEAL/HOLD/SABAR/VOID);
    A-FORGE executes. A receipt is evidence that a step was recorded, not that it was right.
"""
    from starlette.responses import JSONResponse

    tool_error = None
    try:
        # FastMCP 4.x exposes the registry as list_tools() — verified on the
        # installed 4.0.4. It is synchronous there but awaited in some releases,
        # so handle both rather than pinning one and silently reporting None.
        _tools = mcp.list_tools()
        if hasattr(_tools, "__await__"):
            _tools = await _tools
        tool_count = len(_tools)
    except Exception as exc:  # noqa: BLE001 — publish the reason, do not guess
        tool_count = None
        tool_error = f"{type(exc).__name__}: {exc}"

    payload: dict[str, Any] = {
        "status": "healthy" if tool_count else "degraded",
        "service": "ariflow-mcp",
        "version": getattr(mcp, "version", None),
        "tools_loaded": tool_count,
        "tool_count": tool_count,
    }
    if tool_error:
        payload["tool_count_error"] = tool_error
    return JSONResponse(payload)


if __name__ == "__main__":
    mcp.run(transport="http", host="127.0.0.1", port=7075)
