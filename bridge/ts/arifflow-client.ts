/**
 * arifFLOW Client — TypeScript bridge for AAA / A-FORGE federation organs.
 *
 * Usage:
 *   import { ArifFlowClient, emitReceipt } from './arifflow-client.js';
 *
 *   const client = new ArifFlowClient('http://127.0.0.1:7073');
 *   const result = await client.ingest({
 *     receipt_id: crypto.randomUUID(),
 *     step_type: 'Execute',
 *     organ: 'A-FORGE',
 *     actor_id: 'forge',
 *     session_id: 'sess_abc',
 *     summary: 'Deployed authentication fix',
 *     epistemic: 'OBS',
 *     cost_ns: 2_500_000,
 *   });
 *
 * DITEMPA BUKAN DIBERI — receipts are evidence, not decoration.
 */

// ── Enums ────────────────────────────────────────────────────────────────

export type StepType =
  | 'Execute'
  | 'Verify'
  | 'Cool'
  | 'Seal'
  | 'Barrier'
  | 'Merge'
  | 'Route';

export type EpistemicLabel = 'OBS' | 'DER' | 'INT' | 'SPEC' | 'SEAL';

export type FloorVerdict = 'PASS' | 'CAUTION' | 'HOLD' | 'VOID';

export type CoolingDecision = 'NONE' | 'HOLD' | 'CLAMP' | 'BYPASS';

// ── Types ────────────────────────────────────────────────────────────────

export interface TriWitnessVotes {
  human: number;
  ai: number;
  earth: number;
}

export interface FlowReceiptEnvelope {
  receipt_id: string;
  step_type: StepType;
  step_index: number;
  actor_id: string;
  session_id: string;
  organ: string;
  epistemic: EpistemicLabel;
  floor_verdict: FloorVerdict;
  cooling: CoolingDecision;
  tri_witness?: TriWitnessVotes;
  cost_ns: number;
  cost_type: string;
  summary: string;
  details: Record<string, unknown>;
  parent_receipt_id?: string | null;
  chain_id?: string | null;
  lease_id?: string | null;
  timestamp_iso: string;
  sha256: string;
}

export interface IngestResponse {
  status: string;
  fq: {
    quotient: number;
    verdict: string;
    execute_count: number;
    verify_count: number;
  };
  receipts: number;
}

export interface HealthResponse {
  status: string;
  fq: {
    quotient: number;
    verdict: string;
    execute_count: number;
    verify_count: number;
  };
  receipts: number;
  uptime_ms: number;
}

export interface EmitReceiptParams {
  step_type?: StepType;
  organ?: string;
  actor_id: string;
  session_id: string;
  summary: string;
  epistemic?: EpistemicLabel;
  floor_verdict?: FloorVerdict;
  cooling?: CoolingDecision;
  cost_ns?: number;
  cost_type?: string;
  parent_receipt_id?: string;
  chain_id?: string;
  lease_id?: string;
  details?: Record<string, unknown>;
  tri_witness?: TriWitnessVotes;
}

// ── Defaults ─────────────────────────────────────────────────────────────

const DEFAULT_TRI_WITNESS: TriWitnessVotes = {
  human: 0.42,
  ai: 0.32,
  earth: 0.26,
};

// ── Client ───────────────────────────────────────────────────────────────

export class ArifFlowClient {
  private baseUrl: string;
  private timeout: number;

  constructor(baseUrl = 'http://127.0.0.1:7073', timeout = 5_000) {
    this.baseUrl = baseUrl.replace(/\/$/, '');
    this.timeout = timeout;
  }

  /** GET /health — returns FQ, receipt count, uptime */
  async health(): Promise<HealthResponse> {
    const res = await fetch(`${this.baseUrl}/health`, {
      signal: AbortSignal.timeout(this.timeout),
    });
    if (!res.ok) throw new Error(`arifFLOW health failed: ${res.status}`);
    return res.json();
  }

  /** POST /ingest — submit a receipt to arifFLOW */
  async ingest(
    receipt: FlowReceiptEnvelope | Partial<FlowReceiptEnvelope>,
  ): Promise<IngestResponse> {
    const res = await fetch(`${this.baseUrl}/ingest`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(receipt),
      signal: AbortSignal.timeout(this.timeout),
    });
    if (!res.ok) {
      const errBody = await res.text();
      throw new Error(`arifFLOW ingest failed (${res.status}): ${errBody}`);
    }
    return res.json();
  }

  /** Get current Flow Quotient */
  async fq(): Promise<HealthResponse['fq']> {
    const h = await this.health();
    return h.fq;
  }

  /** Check if arifFLOW is reachable */
  async isAlive(): Promise<boolean> {
    try {
      await this.health();
      return true;
    } catch {
      return false;
    }
  }
}

// ── Convenience Functions ────────────────────────────────────────────────

// Module-level singleton
let _client: ArifFlowClient | null = null;

export function getClient(baseUrl?: string): ArifFlowClient {
  if (!_client) {
    _client = new ArifFlowClient(baseUrl);
  }
  return _client;
}

/**
 * Emit a single receipt to arifFLOW.
 *
 * This is the ONE function every organ should call instead of generating
 * receipts independently. After P1, all 32 receipt sources converge here.
 */
export async function emitReceipt(
  params: EmitReceiptParams,
  client?: ArifFlowClient,
): Promise<IngestResponse> {
  const c = client || getClient();
  const now = new Date().toISOString();
  const receiptId = crypto.randomUUID();

  const envelope: FlowReceiptEnvelope = {
    receipt_id: receiptId,
    step_type: params.step_type || 'Execute',
    step_index: 0,
    actor_id: params.actor_id,
    session_id: params.session_id,
    organ: params.organ || 'A-FORGE',
    epistemic: params.epistemic || 'OBS',
    floor_verdict: params.floor_verdict || 'PASS',
    cooling: params.cooling || 'NONE',
    tri_witness: params.tri_witness || DEFAULT_TRI_WITNESS,
    cost_ns: params.cost_ns || 0,
    cost_type: params.cost_type || 'compute',
    summary: params.summary,
    details: params.details || {},
    parent_receipt_id: params.parent_receipt_id || null,
    chain_id: params.chain_id || null,
    lease_id: params.lease_id || null,
    timestamp_iso: now,
    sha256: '', // computed server-side
  };

  // Quick client-side SHA-256 for traceability
  const hashContent = JSON.stringify({
    receipt_id: envelope.receipt_id,
    step_type: envelope.step_type,
    actor_id: envelope.actor_id,
    session_id: envelope.session_id,
    organ: envelope.organ,
    summary: envelope.summary,
    cost_ns: envelope.cost_ns,
    parent_receipt_id: envelope.parent_receipt_id || '',
    timestamp_iso: envelope.timestamp_iso,
  });
  const hashBuffer = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(hashContent),
  );
  envelope.sha256 = Array.from(new Uint8Array(hashBuffer))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');

  return c.ingest(envelope);
}

// Legacy-compatible factory from AAA's emitReceipt() signature
export function createReceipt(params: EmitReceiptParams): FlowReceiptEnvelope {
  const receiptId = crypto.randomUUID();
  const now = new Date().toISOString();
  return {
    receipt_id: receiptId,
    step_type: params.step_type || 'Execute',
    step_index: 0,
    actor_id: params.actor_id,
    session_id: params.session_id,
    organ: params.organ || 'AAA',
    epistemic: params.epistemic || 'OBS',
    floor_verdict: params.floor_verdict || 'PASS',
    cooling: params.cooling || 'NONE',
    tri_witness: params.tri_witness || DEFAULT_TRI_WITNESS,
    cost_ns: params.cost_ns || 0,
    cost_type: params.cost_type || 'compute',
    summary: params.summary,
    details: params.details || {},
    parent_receipt_id: params.parent_receipt_id || null,
    chain_id: params.chain_id || null,
    lease_id: params.lease_id || null,
    timestamp_iso: now,
    sha256: '',
  };
}
