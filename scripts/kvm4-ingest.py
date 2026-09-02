#!/usr/bin/env python3
"""KVM4 CCC worker arifFlow client.

Tries KVM8 organ :7073. If the mesh is down, queues receipts locally.
Never runs the metabolism organ on this box.
"""
from __future__ import annotations

import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ORGAN_URL = os.environ.get("ARIFLOW_URL", "http://100.64.0.2:7073")
QUEUE = Path(os.environ.get("ARIFLOW_QUEUE", "/var/lib/arifflow-kvm4/receipts.jsonl"))
ACTOR = os.environ.get("ARIFLOW_ACTOR", "grok-build")


def _hash_line(obj: dict) -> str:
    payload = json.dumps(obj, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(payload).hexdigest()


def _last_hash() -> str | None:
    if not QUEUE.exists():
        return None
    last = None
    with QUEUE.open() as f:
        for line in f:
            line = line.strip()
            if line:
                last = json.loads(line)
    return None if last is None else last.get("receipt_hash")


def _queue(obj: dict) -> str:
    QUEUE.parent.mkdir(parents=True, exist_ok=True)
    obj = dict(obj)
    obj["queued_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    obj["host"] = "kvm4-forge"
    obj["organ_url"] = ORGAN_URL
    prev = _last_hash()
    if prev:
        obj["previous_receipt_hash"] = prev
    obj["receipt_hash"] = _hash_line({k: v for k, v in obj.items() if k != "receipt_hash"})
    with QUEUE.open("a") as f:
        f.write(json.dumps(obj, sort_keys=True) + "\n")
    return obj["receipt_hash"]


def _post(path: str, body: dict, timeout: float = 4.0) -> tuple[int, dict | str]:
    req = urllib.request.Request(
        ORGAN_URL.rstrip("/") + path,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read().decode()
            try:
                return resp.status, json.loads(raw)
            except json.JSONDecodeError:
                return resp.status, raw
    except urllib.error.HTTPError as e:
        raw = e.read().decode() if e.fp else ""
        try:
            return e.code, json.loads(raw)
        except json.JSONDecodeError:
            return e.code, raw
    except Exception as e:
        return 0, str(e)


def check(actor_id: str = ACTOR) -> dict:
    code, body = _post("/check", {"actor_id": actor_id})
    result = {"actor_id": actor_id, "http": code, "body": body, "organ_reachable": code != 0}
    if code == 0:
        result["allowed"] = True
        result["mode"] = "queue-degraded"
        result["reason"] = "KVM8 arifFlow unreachable; proceeding with local queue"
    else:
        result["allowed"] = bool(body.get("allowed", False)) if isinstance(body, dict) else False
        result["mode"] = "organ"
    return result


def ingest(step_type: str, observation: str, extra: dict | None = None) -> dict:
    import uuid
    from datetime import datetime, timezone

    allowed = {"Execute", "Verify", "Cool", "Seal", "Barrier", "Merge", "Route"}
    if step_type not in allowed:
        step_type = "Cool"
    payload = {
        "receipt_id": str(uuid.uuid4()),
        "created_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ"),
        "actor_id": ACTOR,
        "session_id": os.environ.get("ARIFLOW_SESSION", "forge-core-zen"),
        "step_type": step_type,
        "step_number": 1,
        "observation": observation,
        "host": os.environ.get("ARIFOS_NODE_NAME", "forge-core"),
        "harness": "grok-build",
        "cost_ns": 0,
        "verdict": "Pass",
        "epistemic_label": "Observation",
        "floor_verdict": "Pass",
        "cooling_decision": "Hold" if step_type == "Cool" else "None",
    }
    if extra:
        payload.update(extra)
    code, body = _post("/ingest", payload)
    queued = _queue({**payload, "organ_http": code, "organ_body": body})
    return {"http": code, "body": body, "queued_hash": queued, "organ_reachable": code != 0}


def release(actor_id: str = ACTOR) -> dict:
    code, body = _post("/release", {"actor_id": actor_id})
    return {"http": code, "body": body, "organ_reachable": code != 0}


def main(argv: list[str]) -> int:
    if len(argv) < 2 or argv[1] in {"-h", "--help"}:
        print("usage: kvm4-ingest.py check|ingest|release [step_type] [observation...]")
        return 2
    cmd = argv[1]
    if cmd == "check":
        print(json.dumps(check(argv[2] if len(argv) > 2 else ACTOR), indent=2))
        return 0
    if cmd == "release":
        print(json.dumps(release(argv[2] if len(argv) > 2 else ACTOR), indent=2))
        return 0
    if cmd == "ingest":
        step = argv[2] if len(argv) > 2 else "Observe"
        obs = " ".join(argv[3:]) if len(argv) > 3 else "kvm4-ccc"
        print(json.dumps(ingest(step, obs), indent=2))
        return 0
    print("unknown command", cmd, file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
