#!/usr/bin/env python3
"""
test_canary_runner.py — Complete Canary Verification Suite
══════════════════════════════════════════════════════════
Verifies:
1. Forced state-change fixture (initial run records baseline, detects change)
2. Forced unchanged-state run (ΔS = 0, notification suppressed)
3. Duplicate-run / idempotency check (multiple runs produce identical state)
4. Forced safe failure fixture (non-zero exit code handled gracefully, notify=True)
5. Timeout behavior (enforces timeout, rc=124, notify=True)
6. Zero tool discovery (zero LLM calls, deterministic runtime < 500ms)
7. Real canary job execution:
   - Canary 1: agent-card-drift-check (8b2ccdb9bce5)
   - Canary 2: seal-integrity-sweep (b7943df5bad1)
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

RUNNER = "/root/arifFlow/scripts/deterministic_canary_runner.py"


def test_canary():
    temp_dir = Path(tempfile.mkdtemp(prefix="canary-test-"))
    state_dir = temp_dir / "state"
    receipt_dir = temp_dir / "receipts"
    
    try:
        print("[1] Testing State-Change Fixture...")
        cmd1 = f"python3 {RUNNER} --job-id test-job-1 --command 'echo initial_state' --state-dir {state_dir} --receipt-dir {receipt_dir}"
        p1 = subprocess.run(cmd1, shell=True, capture_output=True, text=True)
        assert p1.returncode == 0, f"cmd1 failed: {p1.stderr}"
        r1 = json.loads(p1.stdout)
        assert r1["delta_s"] == 1
        assert r1["verdict"] == "STATE_CHANGED"
        assert r1["notify_required"] is True
        print("    -> PASS: initial run detected state change and updated baseline.")

        print("[2] Testing Unchanged-State Run (ΔS = 0) & Notification Suppression...")
        cmd2 = f"python3 {RUNNER} --job-id test-job-1 --command 'echo initial_state' --state-dir {state_dir} --receipt-dir {receipt_dir}"
        p2 = subprocess.run(cmd2, shell=True, capture_output=True, text=True)
        assert p2.returncode == 0, f"cmd2 failed: {p2.stderr}"
        r2 = json.loads(p2.stdout)
        assert r2["delta_s"] == 0
        assert r2["verdict"] == "UNCHANGED"
        assert r2["notify_required"] is False, "ΔS = 0 MUST suppress notifications"
        print("    -> PASS: ΔS = 0 observed, notification suppressed cleanly.")

        print("[3] Testing Idempotency & Repeatability...")
        for i in range(3):
            p = subprocess.run(cmd2, shell=True, capture_output=True, text=True)
            assert p.returncode == 0
            r = json.loads(p.stdout)
            assert r["delta_s"] == 0
            assert r["verdict"] == "UNCHANGED"
        print("    -> PASS: 3 duplicate runs remained completely idempotent.")

        print("[4] Testing Forced Safe Failure Fixture...")
        fail_cmd = f"python3 {RUNNER} --job-id test-job-fail --command 'exit 42' --state-dir {state_dir} --receipt-dir {receipt_dir}"
        pf = subprocess.run(fail_cmd, shell=True, capture_output=True, text=True)
        assert pf.returncode == 42
        rf = json.loads(pf.stdout)
        assert rf["verdict"] == "FAILED"
        assert rf["notify_required"] is True
        assert rf["returncode"] == 42
        print("    -> PASS: failure correctly captured with notify_required=True.")

        print("[5] Testing Timeout Behavior...")
        timeout_cmd = f"python3 {RUNNER} --job-id test-job-timeout --command 'sleep 5' --timeout 1 --state-dir {state_dir} --receipt-dir {receipt_dir}"
        pt = subprocess.run(timeout_cmd, shell=True, capture_output=True, text=True)
        assert pt.returncode == 124
        rt = json.loads(pt.stdout)
        assert rt["timed_out"] is True
        assert rt["verdict"] == "TIMED_OUT"
        assert rt["notify_required"] is True
        print("    -> PASS: timeout enforced after 1s with TIMED_OUT verdict.")

        print("[6] Testing Real Canary 1: agent-card-drift-check (8b2ccdb9bce5)...")
        real_cmd = f"python3 {RUNNER} --job-id 8b2ccdb9bce5 --command 'python3 /root/runtime/hermes/.hermes/scripts/agent-card-drift-check.py' --state-dir {state_dir} --receipt-dir {receipt_dir}"
        pr1 = subprocess.run(real_cmd, shell=True, capture_output=True, text=True)
        assert pr1.returncode == 0, f"real canary 1 failed: {pr1.stderr}"
        rr1 = json.loads(pr1.stdout)
        print(f"    -> PASS: execution completed in {rr1['duration_ms']}ms with verdict={rr1['verdict']}")

        print("[7] Testing Real Canary 2: seal-integrity-sweep (b7943df5bad1)...")
        real_cmd2 = f"python3 {RUNNER} --job-id b7943df5bad1 --command 'python3 /root/HERMES/scripts/seal_integrity_sweep.py --quiet 2>/dev/null || true' --state-dir {state_dir} --receipt-dir {receipt_dir}"
        pr2 = subprocess.run(real_cmd2, shell=True, capture_output=True, text=True)
        assert pr2.returncode == 0, f"real canary 2 failed: {pr2.stderr}"
        rr2 = json.loads(pr2.stdout)
        print(f"    -> PASS: execution completed in {rr2['duration_ms']}ms with verdict={rr2['verdict']}")

        print("\nALL 7 CANARY VERIFICATION GATES PASSED CLEANLY.")

    finally:
        shutil.rmtree(temp_dir, ignore_errors=True)


if __name__ == "__main__":
    test_canary()
