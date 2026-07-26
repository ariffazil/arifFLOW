/**
 * arifFLOW Receipt Layer — Canonical Types
 * ════════════════════════════════════════════
 *
 * ONE receipt schema to rule them all. Collapses 3 independent receipt sources:
 *   AAA/operation-bus.ts    → ReceiptEvent (lightweight bus receipt)
 *   A-FORGE/forge.ts        → ExecutorReceipt (heavy kernel receipt)
 *   arifOS/transition_receipt.py → TransitionReceipt (constitutional seal)
 *
 * Every organ calls arifFLOW for receipt generation. No independent emit paths.
 *
 * Storage: append-only JSONL at /root/arifFlow/data/receipts.jsonl
 *
 * DITEMPA BUKAN DIBERI — Forged, Not Given.
 */

// ── Receipt Envelope (unified) ──────────────────────────────────────────

export type ReceiptClass = 'FLOW' | 'EXECUTION' | 'CONSTITUTIONAL';

export type ReceiptVerdict = 'SEAL' | 'SABAR' | 'HOLD' | 'VOID';

export interface ReceiptEnvelope {
  /** Unique receipt ID — arifFLOW-minted */
  receipt_id: string;

  /** Receipt class: FLOW (bus), EXECUTION (forge), CONSTITUTIONAL (seal) */
  class: ReceiptClass;

  /** ISO-8601 timestamp of emission */
  timestamp: string;

  // ── Lineage (required for all classes) ──

  /** Operation ID this receipt links to */
  op_id: string;

  /** Governing session */
  session_id: string;

  /** Distributed trace ID */
  trace_id: string;

  /** Source organ that triggered the receipt */
  organ: string;

  /** Capability/tool that produced this result */
  capability?: string;

  // ── Payload (class-dependent) ──

  /** Human-readable 1-line summary */
  result_summary: string;

  /** URI to evidence (snapshot, artifact, log) */
  evidence_uri?: string;

  /** Verdict if constitutional/execution class */
  verdict?: ReceiptVerdict;

  /** Constitutional chain ID (execution/constitutional class) */
  cc_id?: string;

  /** Judgment reference that authorized this (execution/constitutional) */
  judgment_reference?: string;

  /** Authority context (execution class) */
  authority?: {
    actor_id: string;
    session_id: string;
    valid_until?: string;
    lease_id?: string;
    scope?: string;
  };

  /** Execution bounds (execution class) */
  bounds?: {
    reversible: boolean;
    blast_radius: 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL';
    max_tools: number;
    timeout_ms?: number;
  };

  /** Input hash for tamper detection (execution/constitutional) */
  input_hash?: string;

  /** Kernel signature binding */
  kernel_signature?: string;

  // ── Metabolic metadata ──

  /** Metabolism stage this receipt maps to */
  stage?: string;

  /** Whether this receipt is a VAULT999 seal candidate */
  vault_candidate: boolean;

  /** Cryptographic signature (set after emission) */
  signature?: string;

  /** Previous receipt hash (hash chain) */
  prev_hash?: string;

  /** SHA-256 of this receipt (computed on store) */
  hash?: string;
}

// ── Receipt Store Stats ─────────────────────────────────────────────────

export interface ReceiptStats {
  total_receipts: number;
  by_class: Record<ReceiptClass, number>;
  by_organ: Record<string, number>;
  by_verdict: Record<string, number>;
  vault_candidates: number;
  last_receipt_at?: string;
  last_hash?: string;
}

// ── Receipt Query ───────────────────────────────────────────────────────

export interface ReceiptQuery {
  session_id?: string;
  trace_id?: string;
  op_id?: string;
  organ?: string;
  class?: ReceiptClass;
  verdict?: ReceiptVerdict;
  vault_candidate?: boolean;
  since?: string;      // ISO timestamp
  until?: string;      // ISO timestamp
  limit?: number;
  offset?: number;
}

// ── Receipt Validation ──────────────────────────────────────────────────

export interface ReceiptValidation {
  valid: boolean;
  violations: string[];
}

// ── Cross-language schema (for Python ↔ TypeScript parity) ─────────────

/**
 * JSON Schema representation for the ReceiptEnvelope.
 * Used by the Python arifflow.receipt module for schema validation.
 * Both TS and Python implementations MUST produce output matching this contract.
 */
export const RECEIPT_ENVELOPE_SCHEMA_URI =
  'https://arif-fazil.com/canon/schemas/receipt-envelope.json';
