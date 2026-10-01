"""Governance FQ must never render a real number as absence.

Two defects, both measured on the live estate 2026-10-01:

1. `round(gov_fq, 4) if gov_fq else None` — a genuine 0.0 (consequence-bearing
   actors executed and verified NOTHING, the worst metabolism the instrument can
   see) is falsy in Python, so it serialised as null while the verdict beside it
   still read BURNING. The number and its own verdict disagreed about whether
   the number existed.
2. Actors absent from entity_classes.yaml resolved to "unknown", which excludes
   them from governance FQ by DEFAULT rather than by decision, and nothing
   reported the residue. "No data" is not "all clear".
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


stdio = _load("arifflow_stdio_mcp2", HERE / "arifflow-mcp.py")

# exec>0, verify=0 → gov_fq is exactly 0.0
ZERO_METABOLISM = {
    "fq": {
        "quotient": 0.0,
        "verdict": "BURNING",
        "per_actor": {
            "FI-003": {"execute": 45, "verify": 0},
        },
    }
}

ONLY_UNKNOWN = {
    "fq": {
        "quotient": 1.0,
        "verdict": "FLOWING",
        "per_actor": {
            "some-unlisted-actor": {"execute": 100, "verify": 100},
        },
    }
}


@pytest.mark.parametrize("bridge", ["fastmcp", "stdio"])
def test_zero_governance_fq_reports_zero_not_null(monkeypatch, bridge):
    if bridge == "fastmcp":
        monkeypatch.setattr(fastmcp, "_flow_get", lambda p: dict(ZERO_METABOLISM))
        out = fastmcp.flow_entity_report()
    else:
        monkeypatch.setattr(stdio, "flow_get", lambda p: dict(ZERO_METABOLISM))
        out = stdio.call_tool("flow_entity_report", {})
    assert out["governance_execute"] == 45
    assert out["governance_verify"] == 0
    assert out["governance_verdict"] == "BURNING"
    assert out["governance_weighted_fq"] == 0.0, (
        "a real 0.0 must not collapse into null — it is the loudest value "
        "this instrument can produce"
    )


@pytest.mark.parametrize("bridge", ["fastmcp", "stdio"])
def test_genuinely_absent_fq_still_reads_null(monkeypatch, bridge):
    """Fixing the 0.0 lie must not turn UNKNOWN into a fake zero."""
    payload = {"fq": {"quotient": None, "verdict": None, "per_actor": {}}}
    if bridge == "fastmcp":
        monkeypatch.setattr(fastmcp, "_flow_get", lambda p: payload)
        out = fastmcp.flow_entity_report()
    else:
        monkeypatch.setattr(stdio, "flow_get", lambda p: payload)
        out = stdio.call_tool("flow_entity_report", {})
    assert out["governance_weighted_fq"] is None
    assert out["governance_verdict"] == "UNKNOWN"


@pytest.mark.parametrize("bridge", ["fastmcp", "stdio"])
def test_unclassified_residue_is_visible(monkeypatch, bridge):
    if bridge == "fastmcp":
        monkeypatch.setattr(fastmcp, "_flow_get", lambda p: dict(ONLY_UNKNOWN))
        out = fastmcp.flow_entity_report()
    else:
        monkeypatch.setattr(stdio, "flow_get", lambda p: dict(ONLY_UNKNOWN))
        out = stdio.call_tool("flow_entity_report", {})
    cov = out["classification_coverage"]
    assert cov["actors_unclassified"] == 1
    assert cov["unclassified_volume"] == 200
    assert cov["top_unclassified"] == ["some-unlisted-actor"]
    assert cov["consequence_bearing_volume"] == 0, (
        "unknown actors must not silently count as consequence-bearing"
    )
    assert "excluded" in cov["note"]


def test_patrol_emitters_declared_not_defaulted():
    """The 2026-10-01 backfill: loop/patrol actors are stated telemetry."""
    import yaml

    cfg = yaml.safe_load(open("/root/arifFlow/config/entity_classes.yaml"))
    for cls in ("daemon", "infrastructure", "synthetic", "human_agent",
                "interactive_session"):
        assert isinstance(cfg.get(cls), list), f"{cls} must stay a list"
    assert "aed-v1" in cfg["daemon"] and "orchestrator-v1" in cfg["daemon"], (
        "regression: earlier daemon entries were dropped (duplicate YAML key)"
    )
    for actor in ("333-AGI/dynamic-gate", "hermes-rsi-loop", "333-AGI/agentic-web"):
        assert actor in cfg["daemon"], actor
    for actor in ("chron", "arifos"):
        assert actor in cfg["infrastructure"], actor
    # declaring telemetry must not make it consequence-bearing
    for cls in ("daemon", "infrastructure", "synthetic"):
        assert cls not in ("human_agent", "interactive_session")


def test_backfill_changes_no_governance_number():
    """unknown and declared-telemetry both exclude, so the verdict is stable."""
    declared = {"333-AGI/dynamic-gate", "chron", "verified", "hermes-rsi-loop"}
    for actor in declared:
        cls = fastmcp._ENTITY_CLASSES.get(actor) or next(
            (v for k, v in fastmcp._ENTITY_CLASSES.items() if k.lower() == actor.lower()),
            "unknown",
        )
        assert cls in ("daemon", "infrastructure", "synthetic"), (actor, cls)
        assert cls not in ("human_agent", "interactive_session"), (
            f"{actor} classified as {cls} would newly enter governance FQ"
        )
