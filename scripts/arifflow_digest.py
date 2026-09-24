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
from datetime import datetime, timezone, timedelta

VAULT_PATH = "/root/arifOS/VAULT999/arifflow_sealed.jsonl"
OVERRIDE_LOG = "/var/lib/arifflow/override_log.jsonl"
FLOW_STATE = "/root/AAA/state/flow_state.json"
HEALTH_URL = "http://127.0.0.1:7073/health"

# Human return block — human-benefit-sleep-joy-v1 (witness-ratified 2026-09-16)
HUMAN_DIRECTIVE = "/root/WELL/state/human-benefit-sleep-joy-v1.json"
BAIK_LOG = "/root/WELL/state/3baik_log.jsonl"
WELL_SNAPSHOT = "/state/triadic_snapshot.json"
DELIVERY_LOG = "/root/WELL/state/digest_delivery_log.jsonl"


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


NATS_BIN = "/usr/local/bin/nats"
NATS_SERVER = "nats://127.0.0.1:4222"
ORGAN_STREAM = "arifos-organs"


def get_organ_heartbeats():
    """Read last heartbeat per organ from JetStream arifos-organs.

    2026-09-09 witness-membrane: first real reader for the heartbeat stream
    (4 consumers existed, 0 deliveries ever). Existing river only — the
    digest — no new surface (LAW 8). Honest UNKNOWN on any failure;
    never fabricate status (Void Guard).
    """
    import re
    import subprocess

    statuses = {}
    for organ in ("arifos", "aforge", "geox", "wealth", "well"):
        try:
            out = subprocess.run(
                [NATS_BIN, "--server", NATS_SERVER, "stream", "get",
                 ORGAN_STREAM, "-S", "arifos.organ." + organ],
                capture_output=True, text=True, timeout=10,
            )
            text = out.stdout or ""
            m = re.search(r'"status"\s*:\s*"([a-zA-Z_]+)"', text)
            statuses[organ] = m.group(1) if (out.returncode == 0 and m) else "UNKNOWN"
        except Exception:
            statuses[organ] = "UNKNOWN"
    return statuses


def _find_sleep_hours(obj):
    """Recursively hunt a sleep_hours value in the triadic snapshot. None = no data."""
    if isinstance(obj, dict):
        for k, v in obj.items():
            if k in ("sleep_hours", "sleep_last_night_hours") and isinstance(v, (int, float)):
                return v
            found = _find_sleep_hours(v)
            if found is not None:
                return found
    elif isinstance(obj, list):
        for item in obj:
            found = _find_sleep_hours(item)
            if found is not None:
                return found
    return None


def baik_streak():
    """Consecutive-day streak of 3baik replies ending today or yesterday (UTC)."""
    if not os.path.exists(BAIK_LOG):
        return None
    days = set()
    with open(BAIK_LOG) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                days.add((json.loads(line).get("timestamp_utc") or "")[:10])
            except json.JSONDecodeError:
                continue
    days.discard("")
    if not days:
        return None
    d = datetime.now(timezone.utc).date()
    if d.isoformat() not in days:
        d = d - timedelta(days=1)
        if d.isoformat() not in days:
            return None
    n = 0
    while d.isoformat() in days:
        n += 1
        d = d - timedelta(days=1)
    return n


def delivery_failure_notice():
    """P1 (witness 2026-09-16): a failed nightly delivery must be VISIBLE in the
    next human surface — no silent empty-log condition."""
    if not os.path.exists(DELIVERY_LOG):
        return None
    cutoff = (datetime.now(timezone.utc) - timedelta(hours=26)).isoformat()
    worst = None
    with open(DELIVERY_LOG) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                e = json.loads(line)
            except json.JSONDecodeError:
                continue
            if e.get("status") == "FAILED" and (e.get("attempted_at_utc") or "") >= cutoff:
                worst = e
    if not worst:
        return None
    return (
        f"⚠️ Delivery digest malam lepas GAGAL ({worst.get('error_class', '?')} @ "
        f"{worst.get('attempted_at_utc', '?')[11:16]}Z) — cron-deliver perlu check"
    )


def human_joy_block():
    """22:00 human block — one sleep signal + one gratitude prompt. Never more.

    Directive: human-benefit-sleep-joy-v1 (external witness PROCEED_S3,
    2026-09-16). 'tak ada'/'skip'/'bad day' are valid replies — no nagging,
    no diagnosis, no medical inference. Missing directive file defaults ON.
    """
    try:
        with open(HUMAN_DIRECTIVE) as f:
            directive = json.load(f)
        if directive.get("status") != "ACTIVE":
            return []
        if "three_good_things" not in directive.get("scope", []):
            return []
    except Exception:
        pass

    lines = ["🌙 **MALAM INI**", ""]

    notice = delivery_failure_notice()
    if notice:
        lines.append(notice)

    # Sleep-as-joy signal — honest omit when no data (Void Guard)
    sleep_hours = None
    try:
        with open(WELL_SNAPSHOT) as f:
            sleep_hours = _find_sleep_hours(json.load(f))
    except Exception:
        sleep_hours = None
    if sleep_hours is not None:
        lines.append(f"Tidur: {sleep_hours:.1f}j — otak dah tune untuk esok ✨")
    else:
        lines.append("Tidur: data belum ada (biometric consent OFF)")

    streak = baik_streak()
    if streak:
        lines.append(f"Streak 3 baik: {streak} hari ✨")

    lines.append("")
    lines.append("**3 BAIK HARI INI** — balas mesej ini:")
    lines.append("`3baik: satu; dua; tiga`")
    lines.append("_(tak ada / skip pun sah — semua jawapan valid)_")
    lines.append("")
    return lines


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
        events.append(
            {
                "type": "OVERRIDE",
                "summary": f"Emergency override used: {o.get('override_type', 'unknown')}",
                "details": {
                    "Actor": o.get("actor", "unknown"),
                    "Reason": o.get("reason", "unknown"),
                    "Expiry": o.get("expiry", "unknown"),
                    "Timestamp": o.get("timestamp", "unknown"),
                },
                "priority": 1,
            }
        )

    # Vault entries.
    # 2026-09-15 VAULT999 SOT reconciliation (Hermes/333 subagent):
    #   arifflow_sealed.jsonl records carry NO timestamp/created_at field (keys
    #   are chain_entry_hash, chain_position, prev_hash, receipt_id,
    #   vault_entry_id, body_hash, genesis_anchor, parent_receipt_hashes,
    #   routed_organ). read_jsonl()'s `since` window is therefore inert and this
    #   is a CUMULATIVE-TO-DATE count of the mirror file, never a 24h count.
    #   It was previously published as "(24h)" — a fabricated window. Labelled
    #   honestly now.
    #   chain_position is a PER-CHAIN index that restarts at 0 for each batch,
    #   so first-record/last-record positions do not describe one chain (the
    #   old "0 → 147" output was the position of the first and last line, not a
    #   chain length). Report the chain count and the max position instead.
    actor_counts = {}
    for v in vault_entries:
        actor = v.get("receipt_id", "unknown")[:8]
        actor_counts[actor] = actor_counts.get(actor, 0) + 1
    if vault_entries:
        positions = [
            v["chain_position"]
            for v in vault_entries
            if isinstance(v.get("chain_position"), int)
        ]
        n_chains = positions.count(0)
        events.append(
            {
                "type": "VAULT_ACTIVITY",
                "summary": (
                    f"{len(vault_entries)} receipts in VAULT999 arifFLOW mirror "
                    f"(cumulative — mirror records carry no timestamp)"
                ),
                "details": {
                    "Total entries (cumulative)": str(len(vault_entries)),
                    "Chains in file": str(n_chains),
                    "Max chain_position": str(max(positions) if positions else "?"),
                    "Window": "NONE — source records have no timestamp field",
                },
                "priority": 2,
            }
        )

    # FQ diagnosis (not scalar — scalar is gameable via verification dominance)
    if fq_state or health:
        h_fq = health.get("fq", {}) if health else {}
        execute = int(
            h_fq.get(
                "execute_count", fq_state.get("execute_count", 0) if fq_state else 0
            )
        )
        verify = int(
            h_fq.get("verify_count", fq_state.get("verify_count", 0) if fq_state else 0)
        )
        total = execute + verify
        verify_concentration = (verify / total * 100) if total > 0 else 0
        balance = (
            "BALANCED"
            if 20 <= verify_concentration <= 80
            else "VERIFICATION DOMINANCE"
            if verify_concentration > 80
            else "EXECUTION DOMINANCE"
        )
        diagnosis = (
            f"{balance} ({verify_concentration:.0f}% verify, {execute}E/{verify}V)"
        )

        events.append(
            {
                "type": "DIAGNOSIS",
                "summary": f"Flow: {diagnosis}",
                "details": {
                    "Execute count": str(execute),
                    "Verify count": str(verify),
                    "Verify concentration": f"{verify_concentration:.1f}%",
                    "Balance verdict": balance,
                    "Scalar FQ (deprecated as health indicator)": str(
                        h_fq.get(
                            "quotient", fq_state.get("fq", "?") if fq_state else "?"
                        )
                    ),
                },
                "priority": 3,
            }
        )

    # Health from live daemon + cache fallback for fields daemon omits
    if health:
        h_fq = health.get("fq", {})
        h_inv = health.get("invariants", {})
        barrier = h_fq.get("barrier_count")
        if barrier is None and fq_state:
            barrier = fq_state.get("barrier_count")
        events.append(
            {
                "type": "DAEMON_HEALTH",
                "summary": f"Daemon: cycles={h_inv.get('cycle_count', '?')} restricted={h_inv.get('restricted_actors', [])}",
                "details": {
                    "Execute": str(h_fq.get("execute_count", "?")),
                    "Verify": str(h_fq.get("verify_count", "?")),
                    "Barrier": str(barrier if barrier is not None else "?"),
                    "Enforcement cycles": str(h_inv.get("cycle_count", "?")),
                    "Restricted actors": str(h_inv.get("restricted_actors", [])),
                },
                "priority": 4,
            }
        )

    # Organ heartbeats — arifos-organs stream reader (first real consumer, 2026-09-09)
    hb = get_organ_heartbeats()
    distress = [o for o, s in hb.items() if s.upper() not in ("HEALTHY", "OK", "UNKNOWN", "ENABLED")]
    events.append(
        {
            "type": "ORGAN_HEARTBEATS",
            "summary": "Organs: " + " ".join(f"{o}={s}" for o, s in hb.items()),
            "details": {
                "Distress": ",".join(distress) if distress else "none",
                "Reader": "arifos-organs JetStream (2026-09-09 wiring)",
            },
            "priority": 5,
        }
    )

    # Alibaba Quota Sentinel telemetry (Cliff: 2026-09-29)
    quota_ledger = "/root/AAA/state/alibaba_quota_ledger.json"
    if os.path.exists(quota_ledger):
        try:
            with open(quota_ledger) as f:
                qdata = json.load(f)
            q_models = qdata.get("models", {})
            q_exhausted = [m for m, v in q_models.items() if v.get("status") == "EXHAUSTED"]
            n_audio = sum(1 for m, v in q_models.items() if v.get("group") == "Audio")
            n_vision = sum(1 for m, v in q_models.items() if v.get("group") == "Vision")
            n_multi = sum(1 for m, v in q_models.items() if v.get("group") == "Multimodal")
            events.append({
                "type": "QUOTA_SENTINEL",
                "summary": f"Alibaba Quota Sentinel: {len(q_models)} models tracked, 5d to cliff (2026-09-29), {len(q_exhausted)} exhausted",
                "details": {
                    "Cliff Date": "2026-09-29 (Free Singapore Quotas)",
                    "Audio Pool": f"{n_audio} models (~140h free ASR)",
                    "Vision Pool": f"{n_vision} models",
                    "Multimodal Pool": f"{n_multi} models",
                    "Exhausted Models": ", ".join(q_exhausted[:5]) if q_exhausted else "none",
                    "Auto-Stop Invariant": "ACTIVE (Free quota only, RM0 floor)",
                },
                "priority": 3,
            })
        except Exception:
            pass

    # Human return block (sleep-as-joy + 3 baik) fires nightly regardless
    joy = human_joy_block()

    # No events AND no joy block = nothing to say
    if not events and not joy:
        return None

    # Format digest — human block first, governance after
    lines = joy
    lines.append("🍓 **arifFLOW Governance Digest**")
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
        lines.append(
            f"⚠️ {n_overrides} governance override(s) in last 24h — review recommended"
        )
    else:
        lines.append("✅ No governance overrides — flow clean")

    return "\n".join(lines)


if __name__ == "__main__":
    digest = format_digest()
    if digest:
        print(digest)
    # If no digest, output nothing (cron stays silent)
