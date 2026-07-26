#!/usr/bin/env python3
"""
P1 Wiring Progress Monitor — polls arifFLOW health and reports delta.

Usage:
  python3 p1_monitor.py [--watch 30] [--once]

DITEMPA BUKAN DIBERI
"""

from __future__ import annotations

import json
import os
import sys
import time
from datetime import UTC, datetime
from pathlib import Path
from urllib.request import Request, urlopen

ARIFLOW_HEALTH = "http://127.0.0.1:7073/health"
STATE_FILE = Path("/root/arifFlow/data/p1_monitor_state.json")


def fetch_health() -> dict:
    req = Request(ARIFLOW_HEALTH)
    with urlopen(req, timeout=5) as resp:
        return json.loads(resp.read())


def load_state() -> dict:
    if STATE_FILE.exists():
        return json.loads(STATE_FILE.read_text())
    return {"last_receipt_count": 0, "last_check": None, "history": []}


def save_state(state: dict) -> None:
    STATE_FILE.parent.mkdir(parents=True, exist_ok=True)
    STATE_FILE.write_text(json.dumps(state, indent=2, default=str))


def check(prev_state: dict) -> dict:
    now = datetime.now(UTC).isoformat()
    health = fetch_health()
    chain = health.get("receipt_chain", {})
    current_count = chain.get("count", 0)
    prev_count = prev_state.get("last_receipt_count", 0)
    delta = current_count - prev_count

    status = "HEALTHY" if chain.get("verified") else "BROKEN"

    report = {
        "timestamp": now,
        "receipts": current_count,
        "delta": delta,
        "chain_status": chain.get("chain_status", "UNKNOWN"),
        "status": status,
        "cooling": health.get("cooling", {}).get("state", "?"),
    }

    prev_state["last_receipt_count"] = current_count
    prev_state["last_check"] = now
    prev_state.setdefault("history", []).append(report)
    # Keep last 1000 entries
    prev_state["history"] = prev_state["history"][-1000:]

    return report


def fmt(report: dict) -> str:
    delta = report["delta"]
    arrow = "↑" if delta > 0 else ("↓" if delta < 0 else "→")
    return (
        f"[{report['timestamp'][:19]}] "
        f"receipts={report['receipts']:>6d} {arrow}{abs(delta):<4d} "
        f"chain={report['chain_status']:<10} "
        f"cooling={report['cooling']:<8} "
        f"status={report['status']}"
    )


def main():
    watch = "--watch" in sys.argv
    interval = int(sys.argv[sys.argv.index("--watch") + 1]) if watch else 0

    state = load_state()
    print(f"P1 Monitor — baseline: {state['last_receipt_count']} receipts")
    print(f"{'─' * 70}")

    while True:
        state = load_state()  # Re-read in case other process updates
        report = check(state)
        save_state(state)
        print(fmt(report))

        if not watch:
            break
        time.sleep(interval)


if __name__ == "__main__":
    main()
