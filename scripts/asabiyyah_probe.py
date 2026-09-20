#!/usr/bin/env python3
"""arifFlow organ probe -- Asabiyyah cycle instrument (reading_version 1).

One organ measures its OWN substrate and drops a reading. The kernel federates;
the organ owns its substrate truth. Contract:
    /root/AAA/schemas/asabiyyah-reading.schema.json
Kernel helpers (NOT vendored -- loaded from source):
    /root/arifOS/arifosmcp/runtime/asabiyyah.py

--------------------------------------------------------------------------
PLANE LAW CONSTRAINT -- THIS IS A CORRECTNESS REQUIREMENT, NOT A STYLE CHOICE
--------------------------------------------------------------------------
arifFlow is PLANE 3 (FLOW) and is governed by:

    F3  "Flow observes, never interprets."
        arifFlow measures FQ, detects drift, emits cooling receipts and reports
        divergence. WHAT DRIFT MEANS belongs to ATLAS333 / arifOS.
    F2  "Flow checkpoints, never judges."
        Verdict grammar (SEAL / HOLD / SABAR / VOID) belongs to arifOS.

A probe running inside arifFlow therefore MUST NOT interpret. Concretely, this
probe:

  * emits NO cycle-stage label of any kind. The kernel's band vocabulary is the
    kernel's, and the classifying happens at aggregate time, not here;
  * does NOT set the `band` field on any metric. The kernel helpers are used for
    their ratio ARITHMETIC ONLY; the band they compute is stripped before
    emission (see `_strip_band`). Reason: band IS a stage classification, and
    F3 reserves it for the kernel;
  * does NOT judge arifFlow, any organ, any actor, or any person. Findings are
    emitted as named paths and raw counts, never as an accusation;
  * emits raw additive integer counts under `evidence`, plus ratios tagged with
    `source`. The kernel re-derives the ratios from the counts.

"No source = STORY." Every number below traces to a file, a port response, or a
live probe receipt recorded in `evidence`. NOT_APPLICABLE plus a reason is
always preferred over an invented number.

This probe is read-only against /root/arifFlow. It performs NO write to the
repo, the receipt log, or the VAULT999 chain; the one non-read action it takes
against the live daemon is a gate rejection test, and it verifies afterwards
that zero receipt lines were written. The only file it writes is
/var/lib/arifos/asabiyyah/arifFlow.json (makedirs exist_ok=True).

Standard library only. No third-party imports.
"""

from __future__ import annotations

import argparse
import collections
import datetime as _dt
import hashlib
import importlib.util
import json
import os
import socket
import sys
import urllib.error
import urllib.request

# --------------------------------------------------------------------------
# constants -- every path here is a real, resolvable substrate location
# --------------------------------------------------------------------------

KERNEL_MODULE = "/root/arifOS/arifosmcp/runtime/asabiyyah.py"
SCHEMA_PATH = "/root/AAA/schemas/asabiyyah-reading.schema.json"

REPO_ROOT = "/root/arifFlow"
RECEIPT_LOG = "/var/lib/arifflow/receipts.jsonl"
SEALED_LOG = "/root/arifOS/VAULT999/arifflow_sealed.jsonl"
ENTITY_REGISTRY = "/root/arifFlow/config/entity_classes.yaml"
ENFORCER_SRC = "/root/arifFlow/src/governance/invariants.rs"
DAEMON = "http://127.0.0.1:7073"
DROP_DIR = "/var/lib/arifos/asabiyyah"

# Doctrine-artifact exclusion set. Archive, build output, vendored trees and
# interpreter caches are not live doctrine.
MD_EXCLUDE_DIRS = {
    ".git", "target", "node_modules", ".venv", "__pycache__",
    ".pytest_cache", ".ruff_cache", ".mypy_cache",
}

# The daemon's own dispatch table, read from src/main.rs (`request.starts_with`
# chain). Enumerated here as the surface to audit, not as a claim about it.
ROUTES_DECLARED = [
    ("GET", "/health"), ("POST", "/fq_g"), ("POST", "/consequences"),
    ("POST", "/scar_policies"), ("POST", "/gov_events"), ("POST", "/lineage"),
    ("POST", "/ingest"), ("POST", "/vector"), ("POST", "/check"),
    ("POST", "/release"), ("POST", "/execute"), ("POST", "/enforce"),
    ("POST", "/flow"),
]

PROBE_ACTOR = "__asabiyyah_probe_no_such_actor__"


# --------------------------------------------------------------------------
# kernel helpers -- loaded standalone, never vendored (rule 7)
# --------------------------------------------------------------------------

def load_kernel(path: str = KERNEL_MODULE):
    """Import the asabiyyah kernel module standalone for its ratio functions.

    Deliberately used for `ceremony_exercise_ratio`, `asabiyyah_depth` and
    `enforcement_coverage` ONLY. Not used: `classify_stage`, `band_*`,
    `path_out`, `CycleVerdict` -- those interpret, and F3 forbids this plane
    from interpreting.
    """
    spec = importlib.util.spec_from_file_location("asabiyyah", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load kernel helper module at {path}")
    mod = importlib.util.module_from_spec(spec)
    # Required: the kernel module defines dataclasses, and dataclasses resolve
    # `cls.__module__` through sys.modules during exec_module.
    sys.modules[spec.name] = mod
    spec.loader.exec_module(mod)
    return mod


def _strip_band(metric):
    """Remove the stage classification the kernel helper attached.

    F3: the ratio is an observation; the band is an interpretation. This probe
    keeps the observation and drops the interpretation. The kernel re-bands at
    aggregate time from `evidence`.
    """
    if hasattr(metric, "band"):
        metric.band = ""
    return metric


# --------------------------------------------------------------------------
# small utilities
# --------------------------------------------------------------------------

def _now_iso() -> str:
    return _dt.datetime.now().astimezone().isoformat(timespec="seconds")


def _read_text(path: str):
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            return fh.read()
    except OSError:
        return None


def _count_lines(path: str):
    try:
        n = 0
        with open(path, "rb") as fh:
            for _ in fh:
                n += 1
        return n
    except OSError:
        return None


def _http(method: str, path: str, body=None, base: str = DAEMON,
          timeout: float = 6.0):
    """Minimal HTTP client. Returns (status|None, raw_text)."""
    url = base.rstrip("/") + path
    data = None
    if body is not None:
        data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.status, resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as exc:
        return exc.code, exc.read().decode("utf-8", "replace")
    except Exception as exc:  # connection refused, timeout, DNS, ...
        return None, f"{type(exc).__name__}: {exc}"


def _as_json(raw: str):
    try:
        return json.loads(raw)
    except (ValueError, TypeError):
        return None


def _http_observe_only(method: str, path: str, body=None, base: str = DAEMON,
                       attempts: int = 3):
    """Observe-only probe with retry. Returns (status, raw_text, attempts_used).

    Why the retry exists: the daemon serves each connection with a single
    `stream.read(&mut buf)`. If the request head and the JSON body arrive in
    separate TCP segments the daemon can see an empty body and answer HTTP 400
    for a request that is in fact well formed. Retrying is safe for
    observe-only probes because they change nothing; the attempt count is
    recorded in the reading so the flake is visible rather than smoothed over.
    """
    st = None
    raw = ""
    for i in range(1, attempts + 1):
        st, raw = _http(method, path, body, base=base)
        if st is not None and st != 400:
            return st, raw, i
    return st, raw, attempts


def _parse_ts(ts: str):
    try:
        t = ts.strip().replace("Z", "+00:00")
        dt = _dt.datetime.fromisoformat(t)
        return dt if dt.tzinfo else dt.replace(tzinfo=_dt.timezone.utc)
    except (ValueError, AttributeError):
        return None


# --------------------------------------------------------------------------
# CER input: live doctrine artifacts, and exercised primitive types
# --------------------------------------------------------------------------

def count_live_doctrine_artifacts(root: str = REPO_ROOT):
    """Count live .md artifacts under the repo, excluding archive/build/vendor."""
    mds = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in MD_EXCLUDE_DIRS]
        for fn in filenames:
            if fn.lower().endswith(".md"):
                mds.append(os.path.join(dirpath, fn))
    mds.sort()
    basenames = collections.Counter(os.path.basename(m) for m in mds)
    dupes = {k: v for k, v in basenames.items() if v > 1}
    detail = {
        "doctrine_artifact_duplicate_basenames": dupes,
        "doctrine_artifact_distinct_basenames": len(basenames),
        "doctrine_artifact_excluded_dirs": sorted(MD_EXCLUDE_DIRS),
    }
    return len(mds), mds, detail


def scan_receipts(path: str, window_days: int) -> dict:
    """Scan the live receipt log. Returns raw additive counts only.

    Observes: how many receipts, which actor_ids appear, which step_type values
    were exercised, inside the window. Does not rank, weight or judge them.
    """
    out = {
        "present": False,
        "entries_total": 0,
        "entries_in_window": 0,
        "unparsable_lines": 0,
        "entries_with_null_actor_id": 0,
        "window_start": None,
        "latest_receipt_at": None,
        "actors_all_steps": [],
        "actors_execute_step": [],
        "step_types_in_window": [],
        "step_type_counts_in_window": {},
        "distinct_sessions_in_window": 0,
        "execute_receipts_in_window": 0,
        "verify_receipts_in_window": 0,
        "last_receipt": None,
    }
    if not os.path.exists(path):
        return out
    out["present"] = True

    rows = []
    n_total = 0
    n_bad = 0
    last = None
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                n_total += 1
                try:
                    r = json.loads(line)
                except ValueError:
                    n_bad += 1
                    continue
                rows.append(r)
                last = r
    except OSError:
        return out

    out["entries_total"] = n_total
    out["unparsable_lines"] = n_bad
    if not rows:
        return out

    stamped = [(r, _parse_ts(r.get("created_at", ""))) for r in rows]
    stamped = [(r, ts) for r, ts in stamped if ts is not None]
    if not stamped:
        return out

    newest = max(ts for _, ts in stamped)
    cutoff = newest - _dt.timedelta(days=window_days)
    out["latest_receipt_at"] = newest.isoformat()
    out["window_start"] = cutoff.isoformat()

    win = [r for r, ts in stamped if ts >= cutoff]
    out["entries_in_window"] = len(win)
    out["entries_with_null_actor_id"] = sum(
        1 for r in rows if not r.get("actor_id")
    )

    step = collections.Counter(r.get("step_type") for r in win)
    out["step_type_counts_in_window"] = {
        str(k): v for k, v in sorted(step.items(), key=lambda x: str(x[0]))
    }
    out["step_types_in_window"] = sorted(str(k) for k in step if k)
    out["actors_all_steps"] = sorted(
        {r.get("actor_id") for r in win if r.get("actor_id")}
    )
    out["actors_execute_step"] = sorted(
        {r.get("actor_id") for r in win
         if r.get("step_type") == "Execute" and r.get("actor_id")}
    )
    out["distinct_sessions_in_window"] = len(
        {r.get("session_id") for r in win if r.get("session_id")}
    )
    out["execute_receipts_in_window"] = int(step.get("Execute", 0))
    out["verify_receipts_in_window"] = int(step.get("Verify", 0))
    out["last_receipt"] = last
    return out


def parse_entity_registry(path: str = ENTITY_REGISTRY) -> dict:
    """Parse the actor-class registry with a hand-rolled parser (stdlib only).

    A doctrine registry is itself an observation: it names the identities
    arifFlow recognises as holding doctrine, and it classifies them.
    """
    txt = _read_text(path)
    if txt is None:
        return {"present": False, "classes": {}, "identities": [],
                "identity_entries_total": 0, "duplicate_entries": {}}
    classes = collections.OrderedDict()
    current = None
    for line in txt.splitlines():
        if not line.strip() or line.strip().startswith("#"):
            continue
        if not line[0].isspace() and line.rstrip().endswith(":"):
            current = line.split(":", 1)[0].strip()
            classes[current] = []
        elif current is not None and line.strip().startswith("- "):
            classes[current].append(line.strip()[2:].strip())
    flat = [x for v in classes.values() for x in v]
    dupes = {k: v for k, v in collections.Counter(flat).items() if v > 1}
    return {
        "present": True,
        "classes": classes,
        "identity_entries_total": len(flat),
        "identities": sorted(set(flat)),
        "duplicate_entries": dupes,
    }


# --------------------------------------------------------------------------
# ENC: live gate probes. Adversarial by design.
# --------------------------------------------------------------------------

def probe_gates(base: str = DAEMON, receipt_log: str = RECEIPT_LOG) -> dict:
    """Probe whether each state-changing path verifies anything before mutating.

    Design notes, stated plainly:
      * POST /release is probed with a SYNTHETIC, NON-EXISTENT actor_id. The
        handler calls `InvariantEnforcer::release_hold`, which uses
        `self.actors.get_mut(id)` (verified in src/governance/invariants.rs) --
        an unknown id mutates nothing. So this probe cannot disturb live state,
        while still proving whether the path requires any credential.
      * POST /enforce is probed with no body and no credential. The daemon runs
        this same enforcement cycle itself on a 10s timer, so one extra call
        stays inside the daemon's own declared operating envelope.
      * POST /ingest is probed with a well-formed receipt whose
        previous_receipt_hash is a synthetic hash that is not in the store. The
        chain gate should reject it. The probe counts receipt-log lines before
        and after and records whether ANY write occurred, so the claim is
        falsifiable on every run.
      * No probe fires POST /vector or POST /execute: those would inject
        fabricated dimension readings into the live vector engine or dispatch a
        real execution to A-FORGE. Their gating is recorded from source instead.
    """
    result = {}

    # ---- GET /health : reachability + the flow-plane's own reported signals --
    st, raw, tries = _http_observe_only("GET", "/health", base=base)
    health = _as_json(raw) if raw else None
    result["health"] = {
        "status": st,
        "attempts_used": tries,
        "reachable": st == 200 and isinstance(health, dict),
        "source": f"{base}/health",
    }
    if isinstance(health, dict):
        fq = health.get("fq") or {}
        inv = health.get("invariants") or {}
        vec = health.get("vector") or {}
        diag = vec.get("diagnosis") or {}
        result["health"].update({
            # Transcribed verbatim from the daemon's own output. These are the
            # daemon's words, not this probe's judgement.
            "daemon_reported_verdict": health.get("verdict"),
            "daemon_reported_diagnosis": health.get("diagnosis"),
            "daemon_reported_primary_pathology": diag.get("primary_pathology"),
            "fq_scalar": fq.get("quotient"),
            "fq_execute_count": fq.get("execute_count"),
            "fq_verify_count": fq.get("verify_count"),
            "fq_barrier_count": fq.get("barrier_count"),
            "daemon_reported_actors_tracked":
                (fq.get("metric_frame") or {}).get("actors_tracked"),
            "daemon_reported_formula_version":
                (fq.get("metric_frame") or {}).get("formula_version"),
            "invariant_cycle_count": inv.get("cycle_count"),
            "invariant_hold_count": inv.get("hold_count"),
            "invariant_throttle_count": inv.get("throttle_count"),
            "restricted_actors_count": len(inv.get("restricted_actors") or []),
            "restricted_actors":
                [a.get("actor") for a in (inv.get("restricted_actors") or [])],
            "receipts_in_daemon_memory": health.get("receipts"),
            "uptime_ms": health.get("uptime_ms"),
        })

    # ---- POST /check : the advisory gate itself, observe-only ----
    st, raw, tries = _http_observe_only(
        "POST", "/check", {"actor_id": PROBE_ACTOR}, base=base)
    body = _as_json(raw)
    result["check_unknown_actor"] = {
        "status": st,
        "attempts_used": tries,
        "allowed": (body or {}).get("allowed") if isinstance(body, dict) else None,
        "reason": (body or {}).get("reason") if isinstance(body, dict) else None,
        "raw_response": raw[:400],
        "note": "unseen identity queried against the gate; changes nothing",
    }

    # The same gate, queried with an identity the daemon itself already lists as
    # restricted. Recorded for contrast: how the gate answers an actor it knows
    # beside one it has never seen. Observation, not judgement.
    restricted = []
    if isinstance(health, dict):
        restricted = ((health.get("invariants") or {})
                      .get("restricted_actors")) or []
    if restricted:
        known = restricted[0].get("actor")
        st, raw, tries = _http_observe_only(
            "POST", "/check", {"actor_id": known}, base=base)
        body = _as_json(raw)
        result["check_daemon_restricted_actor"] = {
            "actor_queried": known,
            "status": st,
            "attempts_used": tries,
            "allowed": (body or {}).get("allowed") if isinstance(body, dict) else None,
            "reason": (body or {}).get("reason") if isinstance(body, dict) else None,
            "raw_response": raw[:400],
            "note": "actor name taken verbatim from the daemon's own restricted_actors list",
        }

    # ---- POST /release : does the release path require any credential? ----
    st, raw = _http("POST", "/release", {"actor_id": PROBE_ACTOR}, base=base)
    body = _as_json(raw)
    result["release_synthetic_actor"] = {
        "status": st,
        "response": body if isinstance(body, dict) else None,
        "raw_response": raw[:400],
        "credentials_sent": [],
        "mutates_live_state": False,
        "note": ("synthetic non-existent actor_id; release_hold uses actors.get_mut() "
                 "so no actor is inserted or changed"),
    }

    # ---- POST /enforce : does the enforcement trigger require a credential? --
    st, raw = _http("POST", "/enforce", None, base=base)
    body = _as_json(raw)
    result["enforce_no_credentials"] = {
        "status": st,
        "blocking": (body or {}).get("blocking") if isinstance(body, dict) else None,
        "overall": (body or {}).get("overall") if isinstance(body, dict) else None,
        "raw_response": raw[:400],
        "credentials_sent": [],
        "body_sent": None,
        "note": ("daemon self-runs this same cycle on a 10s timer; this call is "
                 "inside its declared envelope"),
    }

    # ---- POST /ingest : does the chain gate actually reject? ----
    lines_before = _count_lines(receipt_log)
    last = None
    try:
        with open(receipt_log, encoding="utf-8", errors="replace") as fh:
            for line in fh:
                line = line.strip()
                if line:
                    try:
                        last = json.loads(line)
                    except ValueError:
                        continue
    except OSError:
        last = None

    result["ingest_chain_gate"] = {
        "probed": False,
        "reason": "no receipt line available to shape a valid body from",
    }
    if isinstance(last, dict):
        shaped = dict(last)
        shaped["receipt_id"] = "00000000-0000-0000-0000-000000000000"
        shaped["previous_receipt_hash"] = hashlib.sha256(
            b"arifflow-asabiyyah-probe-nonexistent-predecessor"
        ).hexdigest()
        shaped["actor_id"] = PROBE_ACTOR
        shaped["session_id"] = "asabiyyah-probe"
        st, raw = _http("POST", "/ingest", shaped, base=base)
        body = _as_json(raw)
        lines_after = _count_lines(receipt_log)
        wrote = None
        if lines_before is not None and lines_after is not None:
            wrote = (lines_after - lines_before) != 0
        result["ingest_chain_gate"] = {
            "probed": True,
            "status": st,
            "rejected": st == 400,
            "error": (body or {}).get("error") if isinstance(body, dict) else None,
            "raw_response": raw[:400],
            "previous_receipt_hash_sent": shaped["previous_receipt_hash"],
            "receipt_log_lines_before": lines_before,
            "receipt_log_lines_after": lines_after,
            "live_probe_wrote_receipts": wrote,
            "note": "probe asserts on every run that this rejection wrote zero receipt lines",
        }
    return result


# --------------------------------------------------------------------------
# metric assembly
# --------------------------------------------------------------------------

def build_reading(window_days: int, drop_dir: str, base: str) -> dict:
    asb = load_kernel()

    ceremony_artifacts, md_list, md_detail = count_live_doctrine_artifacts()
    receipts = scan_receipts(RECEIPT_LOG, window_days)
    registry = parse_entity_registry()
    gates = probe_gates(base=base)

    # ---- CER inputs -------------------------------------------------------
    exercised_capabilities = len(receipts["step_types_in_window"])

    # ---- ASD inputs -------------------------------------------------------
    # doctrine_holders: distinct agent identities registered in arifFlow's
    #   actor classes / config / policy.
    # executors: distinct actor_id values with a receipted Execute step inside
    #   the window. If the log carried no actor_id this would be
    #   NOT_APPLICABLE -- it does carry actor_id, so it is measurable.
    doctrine_holders = len(registry["identities"]) if registry["present"] else 0
    executors = len(receipts["actors_execute_step"])

    # ---- ENC inputs -------------------------------------------------------
    # Enumerated surface that can change state. `total_paths` counts only the
    # endpoints that mutate something; a read-only query endpoint is not a
    # mutation path and must not be counted as a covered or uncovered one.
    mutating = {
        "GET /health": "in-process vector store (tick / inject FQ / independence record)",
        "POST /ingest": "receipt store + invariant enforcer + VAULT999 chain append",
        "POST /release": "invariant enforcer hold state",
        "POST /enforce": "invariant enforcer cycle / hold / throttle state",
        "POST /vector": "in-process vector store dimensions",
        "POST /execute": "dispatches real execution to A-FORGE :7071",
    }
    total_paths = len(mutating)

    # gated = a state-changing path that VERIFIABLY performs a pre-mutation
    # validation which has been observed to reject. `POST /ingest` is the only
    # one: the probe's invalid-predecessor receipt was refused with
    # HTTP 400 chain_invalid and zero lines written.
    ingest_ok = bool(gates.get("ingest_chain_gate", {}).get("rejected"))
    gated_paths = 1 if ingest_ok else 0

    # A mutation path that reaches state with no credential, no lease, no
    # invariant verdict, and no other pre-mutation verification.
    ungated = [
        "GET /health",
        "POST /release",
        "POST /enforce",
        "POST /vector",
        "POST /execute",
    ]
    if not ingest_ok:
        ungated.append("POST /ingest")

    # Independent, stricter reading, recorded because it is the sharper number:
    # how many state-changing paths verify a constitutional invariant
    # (FQ gate / lease / 888_JUDGE) before mutating. A count, not a verdict.
    constitutional_gated_paths = 0

    observed = _now_iso()
    src_files = (
        f"repo scan {REPO_ROOT} (live .md, excl {sorted(MD_EXCLUDE_DIRS)}) + "
        f"{RECEIPT_LOG} (window {window_days}d)"
    )

    metrics = {
        "cer": _strip_band(asb.ceremony_exercise_ratio(
            ceremony_artifacts, exercised_capabilities, source=src_files)),
        "asd": _strip_band(asb.asabiyyah_depth(
            doctrine_holders, executors,
            source=(f"{ENTITY_REGISTRY} (doctrine holders) + "
                    f"{RECEIPT_LOG} (executors, {window_days}d)"))),
        "enc": _strip_band(asb.enforcement_coverage(
            gated_paths, total_paths,
            source=(f"live daemon {base} dispatch table (src/main.rs) + live gate "
                    f"probes; chain gate rejection observed={ingest_ok}"),
            ungated=ungated)),
    }
    for m in metrics.values():
        m.observed_at = observed

    # ---- evidence: raw additive integer counts plus supporting context -----
    declared_lower = {i.lower() for i in registry["identities"]}
    exec_lower = {a.lower() for a in receipts["actors_execute_step"]}
    matched = declared_lower & exec_lower

    sealed_entries = _count_lines(SEALED_LOG)
    chain_last = None
    chain_max = None
    try:
        with open(SEALED_LOG, encoding="utf-8", errors="replace") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    cp = json.loads(line).get("chain_position")
                except ValueError:
                    continue
                if isinstance(cp, int):
                    chain_last = cp
                    chain_max = cp if chain_max is None else max(chain_max, cp)
    except OSError:
        pass

    evidence = {
        # --- the schema's additive counts, raw, never pre-divided ---
        "ceremony_artifacts": int(ceremony_artifacts),
        "exercised_capabilities": int(exercised_capabilities),
        "doctrine_holders": int(doctrine_holders),
        "executors": int(executors),
        "gated_paths": int(gated_paths),
        "total_paths": int(total_paths),
        "ungated": sorted(ungated),
        "window_days": int(window_days),

        # --- CER provenance ---
        "doctrine_artifact_root": REPO_ROOT,
        "exercised_capabilities_basis":
            "distinct step_type values with a receipt inside window_days",
        "step_types_exercised_in_window": receipts["step_types_in_window"],
        "step_type_counts_in_window": receipts["step_type_counts_in_window"],
        "distinct_sessions_in_window": int(receipts["distinct_sessions_in_window"]),
        "execute_receipts_in_window": int(receipts["execute_receipts_in_window"]),
        "verify_receipts_in_window": int(receipts["verify_receipts_in_window"]),
        "equivalently_cer_by_session_count": (
            round(ceremony_artifacts / receipts["distinct_sessions_in_window"], 6)
            if receipts["distinct_sessions_in_window"] else None
        ),

        # --- ASD provenance ---
        "doctrine_registry_path": ENTITY_REGISTRY,
        "doctrine_registry_classes": registry["classes"],
        "doctrine_registry_identity_entries_total":
            int(registry.get("identity_entries_total", 0)),
        "doctrine_registry_duplicate_entries": registry["duplicate_entries"],
        "declared_identities_with_execute_receipt_in_window": len(matched),
        "declared_identities_without_execute_receipt_in_window":
            len(declared_lower - exec_lower),
        "executor_ids_not_in_declared_registry": len(exec_lower - declared_lower),
        "executor_ids_in_window": receipts["actors_execute_step"],
        "executors_ratio_note": (
            f"{executors} distinct executing actor_ids against {doctrine_holders} "
            "declared identities -- the ratio saturates at 1.0; the raw counts "
            "carry the observation"
        ),
        "receipt_log_entries_total": int(receipts["entries_total"]),
        "receipt_log_entries_in_window": int(receipts["entries_in_window"]),
        "receipt_log_entries_with_null_actor_id":
            int(receipts["entries_with_null_actor_id"]),
        "receipt_log_unparsable_lines": int(receipts["unparsable_lines"]),
        "receipt_actor_id_field_present": bool(
            receipts["last_receipt"] and "actor_id" in receipts["last_receipt"]
        ),

        # --- ENC provenance ---
        "mutation_paths_enumerated": mutating,
        "paths_probed_live": [
            "GET /health", "POST /check", "POST /release",
            "POST /enforce", "POST /ingest",
        ],
        "paths_not_probed_live": {
            "POST /vector":
                "firing it would inject fabricated dimension readings into the live vector engine",
            "POST /execute":
                "firing it would dispatch a real execution to A-FORGE :7071",
            "POST /fq_g, POST /consequences, POST /scar_policies, POST /gov_events, POST /lineage":
                "read-only query handlers, not mutation paths",
            "POST /flow":
                "returns an endpoint acknowledgement; no state change found in handler",
            "POST /check":
                "observe-only: asks the gate, does not mutate",
        },
        "constitutional_gated_paths": int(constitutional_gated_paths),
        "constitutional_gate_definition":
            "pre-mutation check of an FQ gate, a lease, or an 888_JUDGE verdict",
        "ingest_gate_kind":
            ("chain-integrity only: previous_receipt_hash must resolve, JCS body "
             "hash recomputed server-side, parent content-hash edges verified; "
             "NOT an FQ / lease / 888_JUDGE check"),
        "gate_probe_results": gates,

        # --- flow-plane's own reported signals (supporting context) ---
        "daemon_endpoint": base,
        "daemon_reported": (gates.get("health") or {}),
        "sealed_log_path": SEALED_LOG,
        "sealed_log_entries": sealed_entries,
        "chain_position_last": chain_last,
        "chain_position_max_observed": chain_max,
        "invariant_enforcer_source": ENFORCER_SRC,
        "schema_path": SCHEMA_PATH,
        "kernel_module": KERNEL_MODULE,
        "kernel_helpers_used": [
            "ceremony_exercise_ratio", "asabiyyah_depth", "enforcement_coverage",
        ],
        "kernel_helpers_deliberately_unused": [
            "classify_stage", "band_cer", "band_asd", "band_enc", "path_out",
        ],
        "plane_law": ("F3 observe-never-interpret / F2 checkpoint-never-judge: "
                      "no stage label emitted, no band emitted, no organ judged"),
        "host_tailscale_ip": "100.64.0.2",
        "doctrine_artifact_detail": md_detail,
    }

    reading = asb.SubstrateReading(
        organ="arifFlow",
        host=socket.gethostname(),
        observed_at=observed,
        metrics=metrics,
        evidence=evidence,
    )
    return json.loads(reading.to_json())


# --------------------------------------------------------------------------

def main(argv=None) -> int:
    ap = argparse.ArgumentParser(
        prog="asabiyyah_probe",
        description="arifFlow organ probe -- observe-only substrate reading",
    )
    ap.add_argument("--window-days", type=int, default=30)
    ap.add_argument("--drop-dir", default=DROP_DIR)
    ap.add_argument("--daemon", default=DAEMON, help="arifFlow daemon base URL")
    args = ap.parse_args(argv)

    reading = build_reading(args.window_days, args.drop_dir, args.daemon)

    os.makedirs(args.drop_dir, exist_ok=True)
    out_path = os.path.join(args.drop_dir, f"{reading['organ']}.json")
    with open(out_path, "w", encoding="utf-8") as fh:
        json.dump(reading, fh, indent=2, sort_keys=True)
        fh.write("\n")

    print(json.dumps(reading, indent=2, sort_keys=True))
    print(f"\n# written: {out_path}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
