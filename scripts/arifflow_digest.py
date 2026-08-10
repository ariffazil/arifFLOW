#!/usr/bin/env python3
"""T2-2: Human Return Path — arifFLOW governance digest.

Reads VAULT999 + override_log + FQ state.
Produces human-readable digest for Arif.
Deployed as cron job → Telegram delivery.

Output: human-readable text (cron delivers to origin).
Silent when nothing happened (no noise).
"""
import json
import os
import sys
from datetime import datetime, timezone, timedelta

VAULT_PATH = "/root/arifOS/VAULT999/arifflow_sealed.jsonl"
OVERRIDE_LOG = "/var/lib/arifflow/override_log.jsonl"
FLOW_STATE = "/root/AAA/state/flow_state.json"
HEALTH_URL = "http://127.0.0.1:7073/health"

def read_jsonl(path, since=None):
    """Read JSONL file, return list of dicts. Optional: filter by timestamp."""
    entries = []
    if not os.path.exists(path):
        return entries
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
                if since:
                    ts = entry.get("timestamp") or entry.get("created_at", "")
                    if ts and ts < since:
                        continue
                entries.append(entry)
            except json.JSONDecodeError:
                continue
    return entries

def read_flow_state():
    """Read FQ from flow_state.json (mirror cache)."""
    if os.path.exists(FLOW_STATE):
        with open(FLOW_STATE) as f:
            return json.load(f)
    return None

def get_health():
    """Probe daemon /health endpoint."""
    try:
        import urllib.request
        with urllib.request.urlopen(HEALTH_URL, timeout=5) as resp:
            return json.loads(resp.read())
    except Exception:
        return None

def format_digest():
    """Generate the governance digest for Arif."""
    now = datetime.now(timezone.utc)
    since = (now - timedelta(hours=24)).isoformat()

    # Gather events
    vault_entries = read_jsonl(VAULT_PATH, since)
    overrides = read_jsonl(OVERRIDE_LOG, since)
    fq_state = read_flow_state()
    health = get_health()

    # Build event list
    events = []

    # Overrides are governance events — highest priority
    for o in overrides:
        events.append({
            "type": "OVERRIDE",
            "summary": f"Emergency override used: {o.get('override_type', 'unknown')}",
            "details": {
                "Actor": o.get("actor", "unknown"),
                "Reason": o.get("reason", "unknown"),
                "Expiry": o.get("expiry", "unknown"),
                "Timestamp": o.get("timestamp", "unknown"),
            },
            "priority": 1,
        })

    # Vault entries (receipts sealed in last 24h)
    actor_counts = {}
    for v in vault_entries:
        actor = v.get("receipt_id", "unknown")[:8]
        actor_counts[actor] = actor_counts.get(actor, 0) + 1
    if vault_entries:
        events.append({
            "type": "VAULT_ACTIVITY",
            "summary": f"{len(vault_entries)} receipts sealed to VAULT999 (24h)",
            "details": {
                "Total sealed": str(len(vault_entries)),
                "Chain positions": f"{vault_entries[0].get('chain_position', '?')} → {vault_entries[-1].get('chain_position', '?')}",
            },
            "priority": 2,
        })

    # FQ state
    if fq_state:
        fq = fq_state.get("fq", fq_state.get("quotient", "?"))
        verdict = fq_state.get("verdict", fq_state.get("status", "?"))
        events.append({
            "type": "FQ_STATE",
            "summary": f"FQ: {fq} ({verdict})",
            "details": {
                "Quotient": str(fq),
                "Verdict": verdict,
                "Receipts": str(fq_state.get("receipt_count", fq_state.get("receipts", "?"))),
            },
            "priority": 3,
        })

    # Health from live daemon
    if health:
        h_fq = health.get("fq", {})
        h_inv = health.get("invariants", {})
        events.append({
            "type": "DAEMON_HEALTH",
            "summary": f"Daemon: {h_fq.get('quotient', '?'):.2f} ({h_fq.get('verdict', '?')}) cycles={h_inv.get('cycle_count', '?')}",
            "details": {
                "Execute": str(h_fq.get("execute_count", "?")),
                "Verify": str(h_fq.get("verify_count", "?")),
                "Enforcement cycles": str(h_inv.get("cycle_count", "?")),
                "Restricted actors": str(h_inv.get("restricted_actors", [])),
            },
            "priority": 4,
        })

    # No events = nothing happened = stay silent
    if not events:
        return None

    # Format digest
    lines = []
    lines.append(f"🍓 **arifFLOW Governance Digest**")
    lines.append(f"_{now.strftime('%a %d %b %H:%M UTC')}_\n")

    # Sort by priority
    events.sort(key=lambda e: e["priority"])

    for i, event in enumerate(events, 1):
        lines.append(f"**Event {i}: {event['type']}**")
        lines.append(f"{event['summary']}")
        for k, v in event["details"].items():
            lines.append(f"  {k}: `{v}`")
        lines.append("")

    # Summary line
    n_overrides = sum(1 for e in events if e["type"] == "OVERRIDE")
    if n_overrides > 0:
        lines.append(f"⚠️ {n_overrides} governance override(s) in last 24h — review recommended")
    else:
        lines.append("✅ No governance overrides — flow clean")

    return "\n".join(lines)

if __name__ == "__main__":
    digest = format_digest()
    if digest:
        print(digest)
    # If no digest, output nothing (cron stays silent)
