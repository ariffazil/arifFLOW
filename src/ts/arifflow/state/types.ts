/**
 * arifFLOW State Spine — Canonical Types
 * ════════════════════════════════════════
 *
 * Session continuity and Flow Quotient (FQ) types.
 * arifFLOW owns session state and FQ pulse. All organs query here.
 *
 * Organism grammar (§7):
 *   arifFLOW mengalirkan — connects all organs through state continuity.
 *
 * DITEMPA BUKAN DIBERI — Forged, Not Given.
 */

// ── Session State ──────────────────────────────────────────────────────

export type SessionStatus = 'ACTIVE' | 'IDLE' | 'DEGRADED' | 'CLOSED' | 'ORPHANED';

export interface SessionOrganLink {
  organ: string;
  session_id: string;
  status: 'CONNECTED' | 'DISCONNECTED' | 'DEGRADED';
  last_seen: string;
  receipt_count: number;
}

export interface SessionState {
  /** Primary session ID — arifOS-minted */
  session_id: string;

  /** Actor identity */
  actor_id: string;

  /** Session status */
  status: SessionStatus;

  /** ISO-8601 created timestamp */
  created_at: string;

  /** ISO-8601 last activity timestamp */
  last_active_at: string;

  /** Primary intent declared at session init */
  intent?: string;

  /** Organ links — which organs are connected to this session */
  organs: SessionOrganLink[];

  /** Current Flow Quotient for this session */
  fq: FQPulse;

  /** Total receipts across all linked organs */
  total_receipts: number;

  /** Active lease IDs */
  active_leases: string[];

  /** Current constitutional chain ID */
  cc_id?: string;

  /** Last verdict issued */
  last_verdict?: string;

  /** Session metadata (extensible) */
  metadata?: Record<string, unknown>;
}

// ── Flow Quotient (FQ) ─────────────────────────────────────────────────

export type FQVerdict = 'OPTIMAL' | 'BALANCED' | 'WATCHING' | 'STUCK' | 'UNMEASURED';

export interface FQPulse {
  /** Number of execution steps in window */
  execute_count: number;

  /** Number of verification steps in window */
  verify_count: number;

  /** Flow Quotient = execute_cost / verify_cost */
  quotient: number;

  /** Health verdict from FQ range */
  verdict: FQVerdict;

  /** Window size used for computation */
  window_size: number;

  /** ISO-8601 timestamp of last computation */
  computed_at: string;
}

/**
 * FQ Range thresholds (mirrors Rust FlowVerdict):
 *   > 3.0    → OPTIMAL   — Agent in flow. Governance in the architecture.
 *   1.0–3.0  → BALANCED  — Healthy verification.
 *   0.5–1.0  → WATCHING  — Self-monitoring competes with execution.
 *   < 0.5    → STUCK     — Self-monitoring has become the task. mPFC takeover.
 *   0/0      → UNMEASURED — No data yet.
 */
export function fqVerdict(quotient: number, executeCount: number, verifyCount: number): FQVerdict {
  if (executeCount === 0 && verifyCount === 0) return 'UNMEASURED';
  if (quotient > 3.0) return 'OPTIMAL';
  if (quotient >= 1.0) return 'BALANCED';
  if (quotient >= 0.5) return 'WATCHING';
  return 'STUCK';
}

// ── State Store Stats ──────────────────────────────────────────────────

export interface StateStats {
  total_sessions: number;
  active_sessions: number;
  degraded_sessions: number;
  orphaned_sessions: number;
  average_fq: number;
  fq_distribution: Record<FQVerdict, number>;
  last_session_at?: string;
}
