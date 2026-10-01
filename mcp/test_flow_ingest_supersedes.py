"""SEQ-N belief death must be writable through the governed MCP surface.

The Rust daemon accepts supersedes_receipt_ids/_hashes on POST /ingest and
enforces 1:1 arity (src/receipt.rs:1440). Until 2026-10-01 neither bridge
forwarded those fields, so an agent on the MCP path could record a parent edge
but could NOT retract a claim — the estate's differentiator was unreachable
from the only surface agents actually use.

These tests capture the outbound POST body; nothing is written to the live
ledger. Run: python3 -m pytest mcp/test_flow_ingest_supersedes.py -q
"""

import importlib.util
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import arifflow_mcp_fastmcp as fastmcp  # noqa: E402


def _load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


stdio = _load("arifflow_stdio_mcp", HERE / "arifflow-mcp.py")

PARENT_ID = "e89b06c6-cfcd-41cc-9cb9-d82931be078b"
PARENT_HASH = "a4486406dfeabec7447d0981a7e6cc2cad85019858b4433a2b8a5f21101ba99e"
TARGET_ID = "e3867dd8-5556-4a3b-9158-2d66cae6235b"
TARGET_HASH = "c3ebc899de645b39c1fe310855bf06f815833f80d69846a69e77327b74214c75"

BASE_ARGS = {
    "actor_id": "test-supersedes",
    "session_id": "test-supersedes-session",
    "step_type": "Verify",
    "parent_receipt_ids": [PARENT_ID],
    "parent_receipt_hashes": [PARENT_HASH],
}


@pytest.fixture
def captured(monkeypatch):
    """Intercept the daemon POST for both bridges; return the receipt bodies."""
    seen = []

    def fake_fast(path, body):
        seen.append(("fastmcp", path, body))
        return {"status": "ingested", "receipt_id": body["receipt_id"]}

    def fake_stdio(path, body):
        seen.append(("stdio", path, body))
        return 200, {"status": "ingested", "receipt_id": body["receipt_id"]}

    monkeypatch.setattr(fastmcp, "_flow_post", fake_fast)
    monkeypatch.setattr(stdio, "flow_post", fake_stdio)
    return seen


def _calls_for(seen, bridge):
    return [b for (name, _path, b) in seen if name == bridge]


# ── fastmcp surface (what Qwen Code / most harnesses load) ──────────────────

def test_fastmcp_forwards_death_edge(captured):
    out = fastmcp.flow_ingest(
        **BASE_ARGS,
        supersedes_receipt_ids=[TARGET_ID],
        supersedes_receipt_hashes=[TARGET_HASH],
    )
    assert out.get("status") == "ingested", out
    body = _calls_for(captured, "fastmcp")[0]
    assert body["supersedes_receipt_ids"] == [TARGET_ID]
    assert body["supersedes_receipt_hashes"] == [TARGET_HASH]


def test_fastmcp_accepts_ids_only_death_edge(captured):
    """Daemon zips ids with hashes; ids-only is accepted unverified, not rejected."""
    out = fastmcp.flow_ingest(**BASE_ARGS, supersedes_receipt_ids=[TARGET_ID])
    assert out.get("status") == "ingested", out
    body = _calls_for(captured, "fastmcp")[0]
    assert body["supersedes_receipt_ids"] == [TARGET_ID]
    assert "supersedes_receipt_hashes" not in body


@pytest.mark.parametrize("bridge,call", [
    ("fastmcp", lambda **kw: fastmcp.flow_ingest(**kw)),
    ("stdio", lambda **kw: stdio.call_tool("flow_ingest", kw)),
])
def test_arity_mismatch_is_refused_before_the_daemon_sees_it(captured, bridge, call):
    out = call(
        **BASE_ARGS,
        supersedes_receipt_ids=[TARGET_ID, "another-id"],
        supersedes_receipt_hashes=[TARGET_HASH],
    )
    assert out.get("http_status") == 400, out
    assert "SUPERSESSION_ARITY" in out.get("error", ""), out
    assert _calls_for(captured, bridge) == [], "malformed death edge reached the daemon"


@pytest.mark.parametrize("bridge,call", [
    ("fastmcp", lambda **kw: fastmcp.flow_ingest(**kw)),
    ("stdio", lambda **kw: stdio.call_tool("flow_ingest", kw)),
])
def test_hashes_without_ids_are_refused(captured, bridge, call):
    out = call(**BASE_ARGS, supersedes_receipt_hashes=[TARGET_HASH])
    assert out.get("http_status") == 400, out
    assert "SUPERSESSION_ARITY" in out.get("error", ""), out
    assert _calls_for(captured, bridge) == []


def test_stdio_forwards_death_edge(captured):
    out = stdio.call_tool(
        "flow_ingest",
        {**BASE_ARGS,
         "supersedes_receipt_ids": [TARGET_ID],
         "supersedes_receipt_hashes": [TARGET_HASH]},
    )
    assert out.get("status") == "ingested", out
    body = _calls_for(captured, "stdio")[0]
    assert body["supersedes_receipt_ids"] == [TARGET_ID]
    assert body["supersedes_receipt_hashes"] == [TARGET_HASH]


# ── regression: the RG-OM parent gate must not have been loosened ───────────

@pytest.mark.parametrize("bridge,call", [
    ("fastmcp", lambda **kw: fastmcp.flow_ingest(**kw)),
    ("stdio", lambda **kw: stdio.call_tool("flow_ingest", kw)),
])
def test_parent_gate_still_enforced(captured, bridge, call):
    out = call(actor_id="x", session_id="y", supersedes_receipt_ids=[TARGET_ID])
    assert out.get("http_status") == 400, out
    assert "PARENT_REQUIRED" in out.get("error", ""), out
    assert _calls_for(captured, bridge) == []


def test_both_bridges_advertise_the_fields():
    """A field the daemon accepts but no schema exposes is an invisible capability."""
    fast_schema = fastmcp.flow_ingest.__annotations__
    for field in ("supersedes_receipt_ids", "supersedes_receipt_hashes"):
        assert field in fast_schema, f"fastmcp flow_ingest missing {field}"

    stdio_tool = next(
        t for t in stdio.TOOLS if t["name"] == "flow_ingest"
    ) if hasattr(stdio, "TOOLS") else None
    if stdio_tool is not None:
        props = stdio_tool["inputSchema"]["properties"]
        for field in ("supersedes_receipt_ids", "supersedes_receipt_hashes"):
            assert field in props, f"stdio flow_ingest schema missing {field}"
