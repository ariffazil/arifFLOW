/**
 * arifFLOW Client — TypeScript bridge for AAA / A-FORGE federation organs.
 *
 * Matches the Rust FlowReceipt struct EXACTLY for POST /ingest compatibility.
 *
 * Usage:
 *   import { ArifFlowClient, emitReceipt } from './arifflow-client.js';
 *   const client = new ArifFlowClient('http://127.0.0.1:7073');
 *   await emitReceipt({ actor_id: 'forge', session_id: 'sess_abc', summary: '...' });
 *
 * DITEMPA BUKAN DIBERI — receipts are evidence, not decoration.
 */

// ── Enums (match Rust receipt.rs exactly) ─────────────────────────────────

export type StepType =
  | 'Execute' | 'Verify' | 'Cool' | 'Seal' | 'Barrier' | 'Merge' | 'Route';

export type EpistemicLabel = 'OBS' | 'DER' | 'INT' | 'SPEC' | 'SEAL';

export type FloorVerdict = 'PASS' | 'CAUTION' | 'HOLD' | 'VOID';

export type CoolingDecision = 'NONE' | 'HOLD' | 'CLAMP' | 'BYPASS';

// ── Types ────────────────────────────────────────────────────────────────

export interface TriWitnessVotes {
  human: number;
  ai: number;
  earth: number;
}

/**
 * Rust serde enum variant names (PascalCase — NOT display strings).
 * EpistemicLabel: "Observation"|"Derivation"|"Interpretation"|"Specification"|"Seal"
 * FloorVerdict:   "Pass"|"Caution"|"Hold"|"Void"
 * CoolingDecision:"None"|"Hold"|"Clamp"|"Bypass"
 */
export type RustStepType = 'Execute' | 'Verify' | 'Cool' | 'Seal' | 'Barrier' | 'Merge' | 'Route';
export type RustEpistemicLabel = 'Observation' | 'Derivation' | 'Interpretation' | 'Specification' | 'Seal';
export type RustFloorVerdict = 'Pass' | 'Caution' | 'Hold' | 'Void';
export type RustCoolingDecision = 'None' | 'Hold' | 'Clamp' | 'Bypass';

/** EXACT match for Rust FlowReceipt struct fields */
export interface FlowReceiptIngest {
  receipt_id: string;
  previous_receipt_hash: string | null;
  created_at: string;
  actor_id: string;
  session_id: string;
  session_token: string | null;
  step_type: RustStepType;
  topology_id: string | null;
  lane_id: number | null;
  step_number: number;
  cost_ns: number;
  preceding_verify_cost_ns: number | null;
  epistemic_label: RustEpistemicLabel;
  floor_verdict: RustFloorVerdict;
  cooling_decision: RustCoolingDecision;
  tri_witness_votes: TriWitnessVotes | null;
  merkle_root: string | null;
  merkle_inclusion_proof: string | null;
  payload: Record<string, unknown> | null;
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

export interface EmitReceiptParams {
  step_type?: StepType;
  organ?: string;
  actor_id: string;
  session_id: string;
  summary: string;
  epistemic_label?: EpistemicLabel;
  floor_verdict?: FloorVerdict;
  cooling_decision?: CoolingDecision;
  cost_ns?: number;
  preceding_verify_cost_ns?: number;
  parent_receipt_id?: string;
  chain_id?: string;
  lease_id?: string;
  details?: Record<string, unknown>;
  tri_witness_votes?: TriWitnessVotes;
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

  async health(): Promise<HealthResponse> {
    const res = await fetch(`${this.baseUrl}/health`, {
      signal: AbortSignal.timeout(this.timeout),
    });
    if (!res.ok) throw new Error(`arifFLOW health failed: ${res.status}`);
    return res.json();
  }

  /** POST /ingest — submit a Rust FlowReceipt-compatible JSON */
  async ingest(receipt: FlowReceiptIngest): Promise<IngestResponse> {
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

  async fq(): Promise<HealthResponse['fq']> {
    const h = await this.health();
    return h.fq;
  }

  async isAlive(): Promise<boolean> {
    try { await this.health(); return true; } catch { return false; }
  }
}

// Singleton
let _client: ArifFlowClient | null = null;
export function getClient(baseUrl?: string): ArifFlowClient {
  if (!_client) _client = new ArifFlowClient(baseUrl);
  return _client;
}

/**
 * Emit a receipt to arifFLOW. Matches Rust FlowReceipt struct exactly.
 * This is the ONE function every organ calls after P1.
 */
// ── Enum mapping (Python/TS → Rust serde variant names) ──────────────────

const EPISTEMIC_TO_RUST: Record<string, RustEpistemicLabel> = {
  OBS: 'Observation', DER: 'Derivation', INT: 'Interpretation',
  SPEC: 'Specification', SEAL: 'Seal',
};
const FLOOR_TO_RUST: Record<string, RustFloorVerdict> = {
  PASS: 'Pass', CAUTION: 'Caution', HOLD: 'Hold', VOID: 'Void',
};
const COOLING_TO_RUST: Record<string, RustCoolingDecision> = {
  NONE: 'None', HOLD: 'Hold', CLAMP: 'Clamp', BYPASS: 'Bypass',
};

export async function emitReceipt(
  params: EmitReceiptParams,
  client?: ArifFlowClient,
): Promise<IngestResponse> {
  const c = client || getClient();
  const now = new Date().toISOString();

  const payload: Record<string, unknown> = {
    organ: params.organ || 'A-FORGE', summary: params.summary,
  };
  if (params.details) payload.details = params.details;
  if (params.chain_id) payload.chain_id = params.chain_id;
  if (params.lease_id) payload.lease_id = params.lease_id;

  const ingest: FlowReceiptIngest = {
    receipt_id: crypto.randomUUID(),
    previous_receipt_hash: params.parent_receipt_id || null,
    created_at: now,
    actor_id: params.actor_id,
    session_id: params.session_id,
    session_token: null,
    step_type: (params.step_type || 'Execute') as RustStepType,
    topology_id: null, lane_id: null, step_number: 0,
    cost_ns: params.cost_ns || 0,
    preceding_verify_cost_ns: params.preceding_verify_cost_ns || null,
    epistemic_label: EPISTEMIC_TO_RUST[params.epistemic_label || 'OBS'] || 'Observation',
    floor_verdict: FLOOR_TO_RUST[params.floor_verdict || 'PASS'] || 'Pass',
    cooling_decision: COOLING_TO_RUST[params.cooling_decision || 'NONE'] || 'None',
    tri_witness_votes: params.tri_witness_votes || DEFAULT_TRI_WITNESS,
    merkle_root: null, merkle_inclusion_proof: null,
    payload,
  };
  return c.ingest(ingest);
}
