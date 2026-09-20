#!/usr/bin/env python3
"""emit_federation_telemetry.py — arifFlow -> federation invocation log (Tuas 2).

WHY THIS EXISTS
---------------
arifFlow keeps a real flow ledger (/var/lib/arifflow/receipts.jsonl) but that
ledger is arifFlow's own step record, not a tool-invocation log, so the shared
federation aggregator could not see arifFlow alongside the other organs. This
emitter bridges the two WITHOUT touching the flow ledger or the sealed chain.

WHAT IT DOES
------------
Reads ONLY the new lines appended to the receipt log since a stored byte offset,
and forwards each one into the shared federation telemetry log through the frozen
contract in /root/AAA/lib/invocation_log.py.

  receipts.jsonl (O_RDONLY, never written)  ->  tool_invocations.jsonl (O_APPEND)

THE PLANE LAW (arifFlow invariants F2 + F3) — BINDING ON THIS FILE
------------------------------------------------------------------
  F3 "Observe, Never Interpret" · F2 "Checkpoint, Never Judge"

This telemetry records THAT a step arrived and WHICH operation it was. It must
never classify, score, band, rank or label anything. Therefore the forwarder
deliberately DROPS every judgement-bearing field the receipt carries:

    floor_verdict      -> dropped  (that is a verdict)
    epistemic_label    -> dropped  (that is an interpretation)
    cooling_decision   -> dropped  (that is a judgement)
    risk_class         -> dropped  (that is a classification)
    payload            -> dropped  (that is content; also PRIVACY_BOUNDARY)
    intent_reason, expected_outcome, apex_block, flow_block -> dropped

`step_type` is forwarded as the tool name because it is arifFlow's own structural
name for the operation performed (Route/Verify/Execute/Seal/...), not a score.
This is observation of the plane, never interpretation of it.

`ok` is hard-coded True and means "the forward succeeded" — it is NEVER derived
from floor_verdict. Mapping a verdict into a boolean would be exactly the F2
violation this file exists to avoid.

PRIVACY BOUNDARY
----------------
Tool name + actor id + session id only. No request bodies, no payloads, no
human state. A tool name and an actor id are MAP, not STORY.

SAFETY
------
* The receipt log is opened with os.O_RDONLY and there is no code path in this
  file that opens it for writing. Nothing here ever writes to receipts.jsonl or
  to the VAULT999 sealed chain.
* invocation_log.log_invocation never raises, so a telemetry failure cannot take
  down the caller.
* `--sink` is a TEST-ONLY destination for the federation log, mirroring the
  `path` sink override the frozen contract itself reserves for tests (see
  invocation_log.timed docstring). It exists so the offset/incremental logic can
  be exercised against a fixture WITHOUT writing test events into the live
  federation log. Never point it at production.
* State offset advances only past COMPLETE (newline-terminated) lines, so a
  partially-flushed line is retried on the next run instead of being dropped.
* A single-writer flock prevents two concurrent runs from double-forwarding.

Usage:
  emit_federation_telemetry.py               # tail-forward new receipts
  emit_federation_telemetry.py --dry-run     # read + report, write nothing
  emit_federation_telemetry.py --json        # machine-readable run report
  emit_federation_telemetry.py --start-at-eof  # first run: skip history
"""

from __future__ import annotations

import argparse
import fcntl
import json
import os
import sys
import time
from pathlib import Path

AAA_LIB = "/root/AAA/lib"
if AAA_LIB not in sys.path:
    sys.path.insert(0, AAA_LIB)

try:
    from invocation_log import log_invocation
except Exception as exc:  # pragma: no cover
    print(json.dumps({"ok": False, "error": f"cannot import invocation_log: {exc}"}))
    raise SystemExit(2)

ORGAN = "arifFlow"
RECEIPTS = Path("/var/lib/arifflow/receipts.jsonl")
STATE = Path("/var/lib/arifflow/federation_telemetry_offset.json")

# Fields present in the receipt log that this forwarder intentionally discards.
DROPPED_VERDICT_FIELDS = (
    "floor_verdict",
    "epistemic_label",
    "cooling_decision",
    "risk_class",
    "payload",
    "intent_reason",
    "expected_outcome",
    "apex_block",
    "flow_block",
    "projection_block",
    "tri_witness_votes",
    "merkle_root",
)


def _open_readonly(path: Path):
    """The ONLY way this file opens the receipt log. Read-only, always."""
    return os.fdopen(os.open(path, os.O_RDONLY), "rb")


def _load_state(path: Path) -> dict:
    try:
        with path.open("r", encoding="utf-8") as fh:
            st = json.load(fh)
        if not isinstance(st, dict):
            return {}
        return st
    except Exception:
        return {}


def _save_state(path: Path, state: dict) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as fh:
        json.dump(state, fh, indent=2, sort_keys=True)
        fh.write("\n")
    os.replace(tmp, path)


def _duration_ms(rec: dict):
    """cost_ns -> ms. A unit conversion of an observed scalar, not a judgement."""
    ns = rec.get("cost_ns")
    if isinstance(ns, bool) or not isinstance(ns, (int, float)):
        return None
    if ns < 0 or ns > 1e18:  # implausible; do not invent a number
        return None
    return ns / 1e6


def forward(
    receipts: Path = RECEIPTS,
    state_path: Path = STATE,
    *,
    dry_run: bool = False,
    start_at_eof: bool = False,
    max_forward: int = 0,
    sink: Path | str | None = None,
) -> dict:
    report: dict = {
        "organ": ORGAN,
        "receipts": str(receipts),
        "state": str(state_path),
        "dry_run": dry_run,
        "lines_scanned": 0,
        "forwarded": 0,
        "forward_failed": 0,
        "skipped_unparsable": 0,
        "skipped_no_step_type": 0,
        "partial_line_deferred": False,
        "offset_before": None,
        "offset_after": None,
        "receipt_log_size_before": None,
        "receipt_log_size_after": None,
        "rotated_or_truncated": False,
    }

    if not receipts.exists():
        report["error"] = "receipt log absent"
        return report

    st_before = os.stat(receipts)
    report["receipt_log_size_before"] = st_before.st_size

    state = _load_state(state_path)
    offset = state.get("offset")
    prev_inode = state.get("inode")

    if not isinstance(offset, int) or offset < 0:
        # First run. Default = full backfill; --start-at-eof = only new activity.
        offset = st_before.st_size if start_at_eof else 0
        report["first_run"] = True
    elif prev_inode is not None and prev_inode != st_before.st_ino:
        # Log rotated/replaced under us: restart from the top of the new file.
        offset = 0
        report["rotated_or_truncated"] = True
    elif st_before.st_size < offset:
        # Truncated in place.
        offset = 0
        report["rotated_or_truncated"] = True

    report["offset_before"] = offset

    lines = []
    consumed = 0
    with _open_readonly(receipts) as fh:
        fh.seek(offset)
        while True:
            line = fh.readline()
            if not line:
                break
            if not line.endswith(b"\n"):
                # Partially flushed by the live daemon; leave it for next run.
                report["partial_line_deferred"] = True
                break
            consumed += len(line)
            lines.append(line)
            if max_forward and len(lines) >= max_forward:
                break

    report["lines_scanned"] = len(lines)
    report["offset_after"] = offset + consumed

    for raw in lines:
        try:
            rec = json.loads(raw.decode("utf-8", errors="replace"))
        except Exception:
            report["skipped_unparsable"] += 1
            continue
        if not isinstance(rec, dict):
            report["skipped_unparsable"] += 1
            continue

        step_type = rec.get("step_type")
        if not step_type or not isinstance(step_type, str):
            report["skipped_no_step_type"] += 1
            continue

        extra = {}
        sn = rec.get("step_number")
        if isinstance(sn, int) and not isinstance(sn, bool):
            extra["step_number"] = sn
        rr = rec.get("receipt_id")
        if isinstance(rr, str) and rr:
            # Opaque identifier for correlation only. No content.
            extra["receipt_id"] = rr

        # ---- PLANE LAW CALL SITE (arifFlow F2/F3) -------------------------
        # Observational only: organ + operation name + actor + session.
        # No verdict, no band, no stage, no score, no payload is emitted here.
        # `ok` = "this forward succeeded"; it is never derived from the
        # receipt's floor_verdict (dropped above) -- that would be a judgement.
        # -------------------------------------------------------------------
        if dry_run:
            report["forwarded"] += 1
            continue
        wrote = log_invocation(
            ORGAN,
            step_type,
            actor_id=rec.get("actor_id") or None,
            ok=True,
            duration_ms=_duration_ms(rec),
            session_id=rec.get("session_id") or None,
            extra=extra or None,
            path=sink,
        )
        if wrote:
            report["forwarded"] += 1
        else:
            report["forward_failed"] += 1

    st_after = os.stat(receipts)
    report["receipt_log_size_after"] = st_after.st_size
    report["receipt_log_grew_by"] = st_after.st_size - st_before.st_size

    if not dry_run:
        new_state = {
            "organ": ORGAN,
            "receipts": str(receipts),
            "inode": st_after.st_ino,
            "offset": offset + consumed,
            "forwarded_total": int(state.get("forwarded_total") or 0) + report["forwarded"],
            "last_run_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "note": (
                "Byte offset into the arifFlow receipt log. This file is the "
                "forwarder's own cursor; it is NOT a receipt and NOT a seal."
            ),
        }
        _save_state(state_path, new_state)
        report["forwarded_total"] = new_state["forwarded_total"]

    return report


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(prog="emit_federation_telemetry", description=__doc__.splitlines()[0])
    ap.add_argument("--receipts", default=str(RECEIPTS))
    ap.add_argument("--state", default=str(STATE))
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--start-at-eof", action="store_true", help="first run: forward no history")
    ap.add_argument("--max-forward", type=int, default=0, help="cap lines per run (0 = all)")
    ap.add_argument(
        "--sink",
        default=None,
        help="TEST-ONLY federation log destination. Mirrors the contract's test "
        "sink override so the offset logic can be tested against a fixture "
        "without writing test events into production telemetry. Never point "
        "this at /var/lib/arifos/metrics/tool_invocations.jsonl.",
    )
    ap.add_argument("--json", action="store_true", help="print only the run report JSON")
    a = ap.parse_args(argv)

    state_path = Path(a.state)
    lock_path = state_path.with_suffix(state_path.suffix + ".lock")
    try:
        lock_path.parent.mkdir(parents=True, exist_ok=True)
        lf = os.open(lock_path, os.O_CREAT | os.O_RDWR, 0o644)
    except Exception as exc:
        print(json.dumps({"ok": False, "error": f"lock unavailable: {exc}"}))
        return 2

    try:
        try:
            fcntl.flock(lf, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            print(json.dumps({"ok": True, "skipped": "another run holds the lock"}))
            return 0
        rep = forward(
            Path(a.receipts),
            state_path,
            dry_run=a.dry_run,
            start_at_eof=a.start_at_eof,
            max_forward=a.max_forward,
            sink=a.sink,
        )
    finally:
        try:
            fcntl.flock(lf, fcntl.LOCK_UN)
            os.close(lf)
        except Exception:
            pass

    rep["ok"] = not rep.get("error")
    if a.json:
        print(json.dumps(rep, indent=2, sort_keys=True))
    else:
        print(
            "arifFlow->federation telemetry: scanned={lines_scanned} forwarded={forwarded} "
            "unparsable={skipped_unparsable} no_step_type={skipped_no_step_type} "
            "failed={forward_failed} offset {offset_before}->{offset_after} "
            "receipts {receipt_log_size_before}->{receipt_log_size_after}".format(**rep)
        )
    return 0 if rep["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
