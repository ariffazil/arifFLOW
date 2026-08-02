"""
arifFlow Python Client — Invariant Gate + Receipt Ingestion

The client-side interface to the arifFlow daemon (:7073).
Agents use this to check invariant gates before executing,
and to ingest flow receipts after execution.

Usage:
    from arifflow.client import ArifFlowClient, check, ingest, release

    # Before executing: check invariant gate
    result = check("333-AGI")
    if not result.allowed:
        raise SystemExit(f"HOLD: {result.reason}")

    # After executing: ingest receipt
    ingest("333-AGI", "session-123", "Execute", "Observation", 1_000_000)

DITEMPA BUKAN DIBERI
"""

from arifflow.client import ArifFlowClient, check, ingest, release, get_client

__version__ = "2026.8.2"
__all__ = ["ArifFlowClient", "check", "ingest", "release", "get_client"]
