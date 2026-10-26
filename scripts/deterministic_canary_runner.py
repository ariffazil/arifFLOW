#!/usr/bin/env python3
"""
deterministic_canary_runner.py — Governed Deterministic Job Runner
═══════════════════════════════════════════════════════════════════

Replaces conversational LLM cron execution with deterministic, zero-discovery execution.
Complies with:
- SCAR-2026-10-26-002 (Chat engine abuse prevention)
- Reality Delta Filter (ΔS = 0 -> clean exit 0, silent, no notification spam)
- Typed machine receipts on every execution
"""

import argparse
import hashlib
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path


DEFAULT_RECEIPT_DIR = Path("/root/forge_work/receipts/AGY/canary")
DEFAULT_STATE_DIR = Path("/root/.hermes/state/canary_runner")


def run_job(job_id: str, command: str, timeout: int, state_dir: Path, receipt_dir: Path) -> dict:
    state_dir.mkdir(parents=True, exist_ok=True)
    receipt_dir.mkdir(parents=True, exist_ok=True)
    
    baseline_file = state_dir / f"{job_id}.baseline"
    started_at = datetime.now(timezone.utc).isoformat()
    t0 = time.monotonic()
    
    timed_out = False
    returncode = -1
    stdout_text = ""
    stderr_text = ""
    
    try:
        proc = subprocess.run(
            command,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        returncode = proc.returncode
        stdout_text = proc.stdout.strip()
        stderr_text = proc.stderr.strip()
    except subprocess.TimeoutExpired as e:
        timed_out = True
        returncode = 124
        stderr_text = f"Execution timed out after {timeout} seconds"
        stdout_text = (e.stdout or "").strip() if isinstance(e.stdout, str) else ""
    except Exception as e:
        returncode = 1
        stderr_text = f"Execution exception: {str(e)}"
    
    duration_ms = round((time.monotonic() - t0) * 1000, 2)
    finished_at = datetime.now(timezone.utc).isoformat()
    
    # Calculate output fingerprint
    content_payload = f"rc:{returncode}\nout:{stdout_text}"
    current_sha256 = hashlib.sha256(content_payload.encode("utf-8")).hexdigest()
    
    prior_sha256 = None
    if baseline_file.exists():
        try:
            prior_sha256 = baseline_file.read_text().strip()
        except Exception:
            prior_sha256 = None
            
    delta_s = 0 if (prior_sha256 and prior_sha256 == current_sha256) else 1
    
    # Verdict determination
    if timed_out:
        verdict = "TIMED_OUT"
        notify_required = True
    elif returncode != 0:
        verdict = "FAILED"
        notify_required = True
    elif delta_s == 0:
        verdict = "UNCHANGED"
        notify_required = False  # ΔS = 0 SILENT EXIT RULE
    else:
        verdict = "STATE_CHANGED"
        # If output is completely empty and rc=0, consider it silent OK
        notify_required = bool(stdout_text)
        # Update baseline on verified state change
        baseline_file.write_text(current_sha256)
        
    receipt = {
        "job_id": job_id,
        "command": command,
        "started_at": started_at,
        "finished_at": finished_at,
        "duration_ms": duration_ms,
        "returncode": returncode,
        "timed_out": timed_out,
        "delta_s": delta_s,
        "verdict": verdict,
        "notify_required": notify_required,
        "prior_sha256": prior_sha256,
        "current_sha256": current_sha256,
        "stdout_snippet": stdout_text[:500] if stdout_text else "",
        "stderr_snippet": stderr_text[:500] if stderr_text else "",
    }
    
    # Write typed machine receipt
    receipt_path = receipt_dir / f"{job_id}-receipt.json"
    receipt_path.write_text(json.dumps(receipt, indent=2))
    
    return receipt


def main():
    parser = argparse.ArgumentParser(description="Deterministic Canary Runner")
    parser.add_argument("--job-id", required=True, help="Canonical Job ID")
    parser.add_argument("--command", required=True, help="Shell command to execute")
    parser.add_argument("--timeout", type=int, default=120, help="Timeout in seconds")
    parser.add_argument("--state-dir", default=str(DEFAULT_STATE_DIR), help="State directory")
    parser.add_argument("--receipt-dir", default=str(DEFAULT_RECEIPT_DIR), help="Receipts directory")
    
    args = parser.parse_args()
    receipt = run_job(
        job_id=args.job_id,
        command=args.command,
        timeout=args.timeout,
        state_dir=Path(args.state_dir),
        receipt_dir=Path(args.receipt_dir),
    )
    
    print(json.dumps(receipt))
    
    # Return exit code based on verdict
    if receipt["verdict"] in ("UNCHANGED", "STATE_CHANGED"):
        sys.exit(0)
    else:
        sys.exit(receipt["returncode"] or 1)


if __name__ == "__main__":
    main()
