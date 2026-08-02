#!/usr/bin/env python3
"""
arifFLOW MCP shim — bridges Kimi Code MCP (stdio JSON-RPC) to the arifFlow
Rust daemon observability surface on 127.0.0.1:7073.

Tools:
  flow_health  — GET /health: Flow Quotient (FQ), verdict, receipt count, uptime
  flow_ingest  — POST /ingest: mint + submit a FlowReceipt (19-field schema
                 from /root/arifFlow/src/receipt.rs) for FQ monitoring,
                 trend analysis, cooling correlation.

Doctrine: arifFlow is METABOLISM — it routes, checkpoints, and witnesses.
It never judges (arifOS) and never executes (A-FORGE). FQ = verify/execute
ratio — the metabolic signal that intelligence is flowing, not just burning.

Stdlib only. MCP stdio = newline-delimited JSON-RPC 2.0.
"""

import json
import socket
import sys
import uuid
from datetime import datetime, timezone

FLOW_HOST = "127.0.0.1"
FLOW_PORT = 7073
PROTOCOL_VERSION = "2024-11-05"

STEP_TYPES = ["Execute", "Verify", "Cool", "Seal", "Barrier", "Merge", "Route"]
EPISTEMIC = ["Observation", "Derivation", "Interpretation", "Specification", "Seal"]
VERDICTS = ["Pass", "Caution", "Hold", "Void"]

TOOLS = [
    {
        "name": "flow_health",
        "description": (
            "arifFlow daemon health + Flow Quotient (FQ = verify/execute ratio over "
            "recent receipts). Verdicts: FLOWING (healthy metabolism), STUCK (no "
            "verification), BURNING (execution outruns verification). Read-only."
        ),
        "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False},
    },
    {
        "name": "flow_ingest",
        "description": (
            "Mint and ingest a FlowReceipt into the arifFlow metabolic ledger "
            "(POST /ingest). Records one governed step: identity, step_type, cost, "
            "epistemic label, and floor verdict. Use to checkpoint work so FQ "
            "monitoring and cooling correlation see it. Returns FQ after ingest."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "actor_id": {"type": "string", "description": "Agent performing the step (e.g. kimi-code/FI-008)"},
                "session_id": {"type": "string", "description": "Governing session id (arif_init)"},
                "step_type": {"type": "string", "enum": STEP_TYPES, "default": "Execute"},
                "step_number": {"type": "integer", "default": 1},
                "cost_ns": {"type": "integer", "description": "Wall-clock step duration in ns", "default": 0},
                "epistemic_label": {"type": "string", "enum": EPISTEMIC, "default": "Derivation"},
                "floor_verdict": {"type": "string", "enum": VERDICTS, "default": "Pass"},
                "session_token": {"type": "string", "description": "SCT token if governed by arifOS"},
                "topology_id": {"type": "string"},
                "lane_id": {"type": "integer"},
                "previous_receipt_hash": {"type": "string"},
                "payload": {"type": "object", "description": "Step-specific data, errors, intermediates"},
            },
            "required": ["actor_id", "session_id"],
            "additionalProperties": False,
        },
    },
]


def flow_raw(request: bytes) -> tuple[int, dict]:
    """One socket, one write, read to EOF.

    The arifFlow daemon performs a single read() per connection, so headers
    and body must arrive in one segment — loopback single-send guarantees this.
    """
    with socket.create_connection((FLOW_HOST, FLOW_PORT), timeout=10) as s:
        s.sendall(request)
        s.shutdown(socket.SHUT_WR)
        chunks = []
        while True:
            data = s.recv(65536)
            if not data:
                break
            chunks.append(data)
    raw = b"".join(chunks).decode(errors="replace")
    status = int(raw.split(" ", 2)[1]) if raw.startswith("HTTP/") else 0
    body = raw.split("\r\n\r\n", 1)[1] if "\r\n\r\n" in raw else "{}"
    return status, json.loads(body or "{}")


def flow_get(path: str) -> dict:
    req = (
        f"GET {path} HTTP/1.1\r\nHost: {FLOW_HOST}:{FLOW_PORT}\r\n"
        "Connection: close\r\n\r\n"
    ).encode()
    _, body = flow_raw(req)
    return body


def flow_post(path: str, body: dict) -> tuple[int, dict]:
    payload = json.dumps(body).encode()
    req = (
        f"POST {path} HTTP/1.1\r\nHost: {FLOW_HOST}:{FLOW_PORT}\r\n"
        f"Content-Type: application/json\r\nContent-Length: {len(payload)}\r\n"
        "Connection: close\r\n\r\n"
    ).encode() + payload
    return flow_raw(req)


def call_tool(name: str, args: dict) -> dict:
    if name == "flow_health":
        return flow_get("/health")
    if name == "flow_ingest":
        receipt = {
            "receipt_id": str(uuid.uuid4()),
            "previous_receipt_hash": args.get("previous_receipt_hash"),
            "created_at": datetime.now(timezone.utc).isoformat(),
            "actor_id": args["actor_id"],
            "session_id": args["session_id"],
            "session_token": args.get("session_token"),
            "step_type": args.get("step_type", "Execute"),
            "topology_id": args.get("topology_id"),
            "lane_id": args.get("lane_id"),
            "step_number": int(args.get("step_number", 1)),
            "cost_ns": int(args.get("cost_ns", 0)),
            "preceding_verify_cost_ns": None,
            "epistemic_label": args.get("epistemic_label", "Derivation"),
            "floor_verdict": args.get("floor_verdict", "Pass"),
            "cooling_decision": "None",
            "tri_witness_votes": None,
            "merkle_root": None,
            "merkle_inclusion_proof": None,
            "payload": args.get("payload"),
        }
        status, body = flow_post("/ingest", receipt)
        return {"http_status": status, "receipt_id": receipt["receipt_id"], **body}
    raise ValueError(f"unknown tool: {name}")


def respond(msg_id, result=None, error=None):
    out = {"jsonrpc": "2.0", "id": msg_id}
    if error is not None:
        out["error"] = error
    else:
        out["result"] = result
    sys.stdout.write(json.dumps(out) + "\n")
    sys.stdout.flush()


def main() -> None:
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        method = msg.get("method", "")
        msg_id = msg.get("id")

        if method == "initialize":
            respond(msg_id, {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "arifflow", "version": "2026.7.26"},
            })
        elif method == "ping":
            respond(msg_id, {})
        elif method and method.startswith("notifications/"):
            continue  # initialized, cancelled, etc. — no response
        elif method == "tools/list":
            respond(msg_id, {"tools": TOOLS})
        elif method == "tools/call":
            params = msg.get("params", {})
            tname = params.get("name", "")
            targs = params.get("arguments", {}) or {}
            try:
                result = call_tool(tname, targs)
                respond(msg_id, {
                    "content": [{"type": "text", "text": json.dumps(result, indent=2)}],
                    "isError": False,
                })
            except Exception as e:  # daemon down, bad input, etc.
                respond(msg_id, {
                    "content": [{"type": "text", "text": f"arifFLOW error: {e}"}],
                    "isError": True,
                })
        else:
            if msg_id is not None:
                respond(msg_id, error={"code": -32601, "message": f"method not found: {method}"})


if __name__ == "__main__":
    main()
