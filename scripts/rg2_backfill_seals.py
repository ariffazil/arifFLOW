#!/usr/bin/env python3
"""
RG-2 Backfill — Annotate existing seal entries with body_hash + parent_receipt_hashes.

Reads:
  - /root/arifOS/VAULT999/arifflow_sealed.jsonl  (chain entries, receipt_id only)
  - /var/lib/arifflow/receipts.jsonl             (full receipt bodies)

Writes:
  - /root/arifOS/VAULT999/arifflow_sealed_rg2.jsonl
    (lineage-aware seal entries: body_hash, parent_receipt_hashes, genesis_anchor, routed_organ)

Each output line preserves the original seal metadata and adds the lineage edges
needed by LineageResolver to reconstruct ancestry from sealed evidence alone.

Run:  python3 scripts/rg2_backfill_seals.py
Verify:  wc -l /root/arifOS/VAULT999/arifflow_sealed.jsonl /root/arifOS/VAULT999/arifflow_sealed_rg2.jsonl
"""
import json
import os
import sys

RECEIPTS_PATH = "/var/lib/arifflow/receipts.jsonl"
SEALS_PATH = "/root/arifOS/VAULT999/arifflow_sealed.jsonl"
OUT_PATH = "/root/arifOS/VAULT999/arifflow_sealed_rg2.jsonl"


def main():
    if not os.path.exists(RECEIPTS_PATH):
        print(f"FATAL: {RECEIPTS_PATH} missing", file=sys.stderr)
        sys.exit(1)
    if not os.path.exists(SEALS_PATH):
        print(f"FATAL: {SEALS_PATH} missing", file=sys.stderr)
        sys.exit(1)

    # Pass 1: Load all receipt bodies, keyed by receipt_id (UUID string).
    print(f"[rg2-backfill] Loading receipts from {RECEIPTS_PATH}…", file=sys.stderr)
    bodies = {}
    body_count = 0
    parse_errors = 0
    with open(RECEIPTS_PATH) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                r = json.loads(line)
                rid = r.get("receipt_id")
                if rid:
                    bodies[rid] = r
                    body_count += 1
            except Exception:
                parse_errors += 1
    print(
        f"[rg2-backfill]   loaded {body_count} receipts ({parse_errors} parse errors)",
        file=sys.stderr,
    )

    # Pass 2: Stream seal entries, annotate with body lineage data, write to RG-2 file.
    print(f"[rg2-backfill] Streaming seals from {SEALS_PATH}…", file=sys.stderr)
    seal_total = 0
    seal_annotated = 0
    seal_missing_body = 0
    seal_missing_parent_evidence = 0
    parent_edge_count = 0
    genesis_anchor_count = 0
    routed_organ_count = 0

    with open(SEALS_PATH) as fin, open(OUT_PATH, "w") as fout:
        for line in fin:
            line = line.strip()
            if not line:
                continue
            seal_total += 1
            try:
                seal = json.loads(line)
            except Exception:
                # Pass through unparseable lines verbatim.
                fout.write(line + "\n")
                continue

            rid = seal.get("receipt_id")
            body = bodies.get(rid) if rid else None
            if body is None:
                seal_missing_body += 1
                # Still emit the seal entry, marked PARTIAL.
                seal["rg2_status"] = "PARTIAL_NO_BODY"
                seal["body_hash"] = None
                seal["parent_receipt_hashes"] = []
                seal["genesis_anchor"] = None
                seal["routed_organ"] = None
                fout.write(json.dumps(seal) + "\n")
                continue

            # body_hash: carried through ONLY when the seal entry itself has
            # one (post-enrichment seal format). For legacy entries the true
            # body hash is the Rust-side SHA3-256 over the serialized receipt
            # — NOT computable in Python before JCS parity lands (three-names
            # rule, spec/RG2_JCS_HASH_CONTRACT_v1.md, forge P4-JCS). We write
            # null rather than a fake receipt_id fallback; the resolver joins
            # by receipt_id regardless.
            body_hash = seal.get("body_hash")  # None for legacy entries — honest
            parent_hashes = body.get("parent_receipt_ids", [])
            genesis_anchor = body.get("genesis_anchor")
            routed_organ = body.get("routed_organ")

            if parent_hashes:
                parent_edge_count += 1
                seal_missing_parent_evidence = max(0, seal_missing_parent_evidence)
            if genesis_anchor:
                genesis_anchor_count += 1
            if routed_organ:
                routed_organ_count += 1

            seal["rg2_status"] = "ANNOTATED"
            seal["body_hash"] = body_hash
            seal["parent_receipt_hashes"] = parent_hashes
            seal["genesis_anchor"] = genesis_anchor
            seal["routed_organ"] = routed_organ
            seal_annotated += 1
            fout.write(json.dumps(seal) + "\n")

    print(
        f"[rg2-backfill] Wrote {seal_total} seal entries → {OUT_PATH}",
        file=sys.stderr,
    )
    print(
        f"[rg2-backfill]   annotated: {seal_annotated}  PARTIAL_NO_BODY: {seal_missing_body}",
        file=sys.stderr,
    )
    print(
        f"[rg2-backfill]   parent edges: {parent_edge_count}  genesis_anchors: {genesis_anchor_count}  routed_organs: {routed_organ_count}",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
