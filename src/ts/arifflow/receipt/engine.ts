/**
 * arifFLOW Receipt Engine — Canonical Implementation
 * ═══════════════════════════════════════════════════
 *
 * ONE engine. ONE emit path. ONE storage format. ONE verify pipeline.
 *
 * Collapses:
 *   AAA/operation-bus.ts:123    emitReceipt()
 *   A-FORGE/forge.ts:141        validateReceipt()
 *   arifOS transition_receipt   mint path
 *
 * Storage: append-only JSONL at /root/arifFlow/data/receipts.jsonl
 * Hash chain: SHA-256 per receipt, linked via prev_hash
 *
 * Cross-language: TypeScript canonical implementation.
 * Python arifflow.receipt module mirrors this contract.
 *
 * DITEMPA BUKAN DIBERI — Forged, Not Given.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import * as crypto from 'node:crypto';

import {
  type ReceiptEnvelope,
  type ReceiptClass,
  type ReceiptVerdict,
  type ReceiptStats,
  type ReceiptQuery,
  type ReceiptValidation,
} from './types.js';

// ── Configuration ───────────────────────────────────────────────────────

const DEFAULT_DATA_DIR = '/root/arifFlow/data';
const RECEIPTS_LOG = 'receipts.jsonl';

// ── Helpers ─────────────────────────────────────────────────────────────

function isoNow(): string {
  return new Date().toISOString();
}

function generateId(prefix: string): string {
  const rand = crypto.randomBytes(8).toString('hex');
  return `${prefix}-${isoNow().replace(/[^0-9]/g, '').slice(0, 14)}-${rand}`;
}

function sha256(input: string): string {
  return crypto.createHash('sha256').update(input, 'utf-8').digest('hex');
}

function ensureDir(dir: string): void {
  fs.mkdirSync(dir, { recursive: true });
}

// ── Storage (append-only JSONL) ────────────────────────────────────────

function receiptsPath(dataDir: string): string {
  return path.join(dataDir, RECEIPTS_LOG);
}

function appendLine(filePath: string, obj: Record<string, unknown>): void {
  const line = JSON.stringify(obj) + '\n';
  fs.appendFileSync(filePath, line, 'utf-8');
}

function readAllLines<T>(filePath: string): T[] {
  if (!fs.existsSync(filePath)) return [];
  const content = fs.readFileSync(filePath, 'utf-8');
  if (!content.trim()) return [];
  return content.trim().split('\n').map(line => JSON.parse(line) as T);
}

function getLastHash(filePath: string): string | undefined {
  const receipts = readAllLines<ReceiptEnvelope>(filePath);
  if (receipts.length === 0) return undefined;
  return receipts[receipts.length - 1]!.hash;
}

// ── Receipt Engine ──────────────────────────────────────────────────────

export class ReceiptEngine {
  private dataDir: string;

  constructor(dataDir: string = DEFAULT_DATA_DIR) {
    this.dataDir = dataDir;
    ensureDir(dataDir);
  }

  // ── Emit ──────────────────────────────────────────────────────────

  /**
   * Canonical receipt emission. This is THE function.
   * All organs call this — AAA, A-FORGE, arifOS.
   *
   * @param params — Receipt parameters (class-dependent fields vary)
   * @returns Sealed ReceiptEnvelope with hash and prev_hash
   */
  emit(params: {
    class: ReceiptClass;
    op_id: string;
    session_id: string;
    trace_id: string;
    organ: string;
    result_summary: string;
    capability?: string;
    evidence_uri?: string;
    verdict?: ReceiptVerdict;
    cc_id?: string;
    judgment_reference?: string;
    authority?: ReceiptEnvelope['authority'];
    bounds?: ReceiptEnvelope['bounds'];
    input_hash?: string;
    kernel_signature?: string;
    stage?: string;
    vault_candidate?: boolean;
  }): ReceiptEnvelope {
    const filePath = receiptsPath(this.dataDir);
    const prev_hash = getLastHash(filePath);

    const envelope: ReceiptEnvelope = {
      receipt_id: generateId('rcpt'),
      class: params.class,
      timestamp: isoNow(),
      op_id: params.op_id,
      session_id: params.session_id,
      trace_id: params.trace_id,
      organ: params.organ,
      capability: params.capability,
      result_summary: params.result_summary,
      evidence_uri: params.evidence_uri,
      verdict: params.verdict,
      cc_id: params.cc_id,
      judgment_reference: params.judgment_reference,
      authority: params.authority,
      bounds: params.bounds,
      input_hash: params.input_hash,
      kernel_signature: params.kernel_signature,
      stage: params.stage,
      vault_candidate: params.vault_candidate ?? false,
      prev_hash,
    };

    // Compute hash (content → SHA-256)
    const hashable = JSON.stringify({
      receipt_id: envelope.receipt_id,
      class: envelope.class,
      timestamp: envelope.timestamp,
      op_id: envelope.op_id,
      session_id: envelope.session_id,
      trace_id: envelope.trace_id,
      organ: envelope.organ,
      result_summary: envelope.result_summary,
      verdict: envelope.verdict,
      cc_id: envelope.cc_id,
      prev_hash: envelope.prev_hash,
    });
    envelope.hash = sha256(hashable);

    // Append to storage
    appendLine(filePath, envelope as unknown as Record<string, unknown>);

    return Object.freeze(envelope) as ReceiptEnvelope;
  }

  // ── Verify ────────────────────────────────────────────────────────

  /**
   * Validate a receipt against constitutional requirements.
   * Replaces A-FORGE/forge.ts:validateReceipt().
   *
   * FLOW-class receipts: lightweight — just require lineage fields.
   * EXECUTION-class receipts: heavy — full kernel receipt validation.
   * CONSTITUTIONAL-class receipts: heaviest — full seal chain verification.
   */
  verify(receipt: ReceiptEnvelope): ReceiptValidation {
    const v: string[] = [];

    // ── Universal requirements (all classes) ──
    if (!receipt.receipt_id) v.push('Missing receipt_id');
    if (!receipt.op_id) v.push('Missing op_id');
    if (!receipt.session_id) v.push('Missing session_id');
    if (!receipt.trace_id) v.push('Missing trace_id');
    if (!receipt.organ) v.push('Missing organ');
    if (!receipt.result_summary) v.push('Missing result_summary');
    if (!receipt.timestamp) v.push('Missing timestamp');
    if (!receipt.class) v.push('Missing class');
    if (!receipt.hash) v.push('Missing hash — receipt not sealed');

    // ── Class-specific requirements ──
    if (receipt.class === 'EXECUTION' || receipt.class === 'CONSTITUTIONAL') {
      // Verdict
      if (!receipt.verdict) v.push('Missing verdict');
      else if (!['SEAL', 'SABAR', 'HOLD', 'VOID'].includes(receipt.verdict)) {
        v.push(`Invalid verdict: ${receipt.verdict}`);
      }

      // Constitutional chain
      if (!receipt.cc_id) v.push('Missing cc_id (constitutional chain ID)');
      if (!receipt.judgment_reference) {
        v.push('Missing judgment_reference — cannot prove authorization');
      }

      // Authority
      if (!receipt.authority?.actor_id) v.push('Missing authority.actor_id');
      if (!receipt.authority?.session_id) v.push('Missing authority.session_id');
      if (receipt.authority?.valid_until) {
        if (new Date(receipt.authority.valid_until) < new Date()) {
          v.push('Authority lease expired');
        }
      }

      // Input integrity
      if (!receipt.input_hash) v.push('Missing input_hash');

      // Kernel binding
      if (!receipt.kernel_signature) v.push('Missing kernel_signature');
    }

    if (receipt.class === 'EXECUTION') {
      // Bounds
      if (!receipt.bounds) v.push('Missing bounds');
      else {
        if (receipt.bounds.reversible === undefined) v.push('Missing bounds.reversible');
        if (!receipt.bounds.blast_radius) v.push('Missing bounds.blast_radius');
        if (!receipt.bounds.max_tools || receipt.bounds.max_tools < 1) {
          v.push('bounds.max_tools must be >= 1');
        }
        // CRITICAL irreversible → block
        if (
          receipt.bounds.blast_radius === 'CRITICAL' &&
          receipt.bounds.reversible === false
        ) {
          v.push('CRITICAL irreversible action requires F13 sovereign path');
        }
      }
    }

    if (receipt.class === 'CONSTITUTIONAL') {
      // Hash chain integrity
      if (!receipt.prev_hash) {
        v.push('Missing prev_hash — constitutional receipts must chain');
      }
    }

    return { valid: v.length === 0, violations: v };
  }

  // ── Query ─────────────────────────────────────────────────────────

  /** Query receipts by filter criteria */
  query(q: ReceiptQuery): ReceiptEnvelope[] {
    const all = readAllLines<ReceiptEnvelope>(receiptsPath(this.dataDir));

    let results = all;

    if (q.session_id) results = results.filter(r => r.session_id === q.session_id);
    if (q.trace_id) results = results.filter(r => r.trace_id === q.trace_id);
    if (q.op_id) results = results.filter(r => r.op_id === q.op_id);
    if (q.organ) results = results.filter(r => r.organ === q.organ);
    if (q.class) results = results.filter(r => r.class === q.class);
    if (q.verdict) results = results.filter(r => r.verdict === q.verdict);
    if (q.vault_candidate !== undefined) {
      results = results.filter(r => r.vault_candidate === q.vault_candidate);
    }
    if (q.since) results = results.filter(r => r.timestamp >= q.since);
    if (q.until) results = results.filter(r => r.timestamp <= q.until);

    // Most recent first
    results.sort((a, b) => b.timestamp.localeCompare(a.timestamp));

    const offset = q.offset ?? 0;
    const limit = q.limit ?? 50;
    return results.slice(offset, offset + limit);
  }

  /** Get a single receipt by ID */
  get(receiptId: string): ReceiptEnvelope | null {
    const all = readAllLines<ReceiptEnvelope>(receiptsPath(this.dataDir));
    return all.find(r => r.receipt_id === receiptId) ?? null;
  }

  // ── Stats ─────────────────────────────────────────────────────────

  /** Compute receipt store statistics */
  stats(): ReceiptStats {
    const receipts = readAllLines<ReceiptEnvelope>(receiptsPath(this.dataDir));

    const by_class: Record<string, number> = {};
    const by_organ: Record<string, number> = {};
    const by_verdict: Record<string, number> = {};
    let vault_candidates = 0;

    for (const r of receipts) {
      by_class[r.class] = (by_class[r.class] ?? 0) + 1;
      by_organ[r.organ] = (by_organ[r.organ] ?? 0) + 1;
      if (r.verdict) {
        by_verdict[r.verdict] = (by_verdict[r.verdict] ?? 0) + 1;
      }
      if (r.vault_candidate) vault_candidates++;
    }

    const last = receipts.length > 0 ? receipts[receipts.length - 1]! : undefined;

    return {
      total_receipts: receipts.length,
      by_class: by_class as ReceiptStats['by_class'],
      by_organ,
      by_verdict,
      vault_candidates,
      last_receipt_at: last?.timestamp,
      last_hash: last?.hash,
    };
  }

  // ── Replay ────────────────────────────────────────────────────────

  /** Replay all receipts (for observatory, audit, recovery) */
  replay(limit?: number): ReceiptEnvelope[] {
    const all = readAllLines<ReceiptEnvelope>(receiptsPath(this.dataDir));
    const sorted = [...all].sort((a, b) => b.timestamp.localeCompare(a.timestamp));
    return limit ? sorted.slice(0, limit) : sorted;
  }

  // ── Hash chain verification ──────────────────────────────────────

  /**
   * Verify the entire hash chain integrity.
   * Returns { valid: true } if every receipt's hash matches its content
   * and every prev_hash links to the previous receipt's hash.
   */
  verifyChain(): { valid: boolean; violations: string[] } {
    const receipts = readAllLines<ReceiptEnvelope>(receiptsPath(this.dataDir));
    const violations: string[] = [];

    for (let i = 0; i < receipts.length; i++) {
      const r = receipts[i]!;

      // Recompute hash
      const hashable = JSON.stringify({
        receipt_id: r.receipt_id,
        class: r.class,
        timestamp: r.timestamp,
        op_id: r.op_id,
        session_id: r.session_id,
        trace_id: r.trace_id,
        organ: r.organ,
        result_summary: r.result_summary,
        verdict: r.verdict,
        cc_id: r.cc_id,
        prev_hash: r.prev_hash,
      });
      const computedHash = sha256(hashable);

      if (computedHash !== r.hash) {
        violations.push(
          `Hash mismatch at ${r.receipt_id}: stored=${r.hash?.slice(0, 12)}... computed=${computedHash.slice(0, 12)}...`,
        );
      }

      // Check prev_hash link (skip first receipt)
      if (i > 0) {
        const prev = receipts[i - 1]!;
        if (r.prev_hash !== prev.hash) {
          violations.push(
            `Chain break at ${r.receipt_id}: prev_hash doesn't match ${prev.receipt_id}`,
          );
        }
      }
    }

    return { valid: violations.length === 0, violations };
  }
}

// ── Singleton ─────────────────────────────────────────────────────────

let _instance: ReceiptEngine | null = null;

export function getReceiptEngine(dataDir?: string): ReceiptEngine {
  if (!_instance) {
    _instance = new ReceiptEngine(dataDir);
  }
  return _instance;
}

/** Reset singleton (for testing) */
export function resetReceiptEngine(): void {
  _instance = null;
}
