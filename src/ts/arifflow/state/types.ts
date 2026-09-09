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

export type FQVerdict = 'UNKNOWN' | 'CAUTION' | 'FOSSILIZED' | 'OPTIMAL' | 'FLOWING' | 'STUCK' | 'BURNING' | 'UNMEASURED';

export interface FQPulse {
  /** Number of execution steps in window */
  execute_count: number;

  /** Number of verification steps in window */
  verify_count: number;

  /** Flow Quotient = verify_count / execute_count (v2.1: count-based, inverted). null when verify_count == 0 */
  quotient: number | null;

  /** Health verdict from FQ range */
  verdict: FQVerdict;

  /** Window size used for computation */
  window_size: number;

  /** ISO-8601 timestamp of last computation */
  computed_at: string;
}

/**
 * FQ Range thresholds v2.2 (Arif F13 spec + Helix Codex Lock 2 — 2026-08-14):
 *   verify=0  → UNKNOWN    — No verification data. Missing, not healthy.
 *   verify<2  → CAUTION    — Single verification is coincidence, not pattern.
 *   q > 3.0   → FOSSILIZED — verify:execute > 3:1 (contact, no motion). Calhoun sink.
 *   q >= 1.0  → OPTIMAL    — Verification leads execution.
 *   q >= 0.5  → FLOWING    — Healthy metabolism.
 *   q >= 0.1  → STUCK      — Verification lagging execution.
 *   q < 0.1   → BURNING    — execute:verify > 10:1 (motion, no witness). Calhoun sink.
 *
 * Both poles are Calhoun sink (Helix Codex Lock 2):
 *   verify:execute > 3:1 → FOSSILIZED (fossilisation pole)
 *   execute:verify > 3:1 → BURNING (burn pole)
 *
 * Quotient = verify_count / execute_count (inverted from v2.0 exec/verify).
 * formula_hash: sha256:arifflow-fq-v2.2-2026-08-14
 * formula_version: qg.v0.2
 */
export function fqVerdict(quotient: number | null, executeCount: number, verifyCount: number): FQVerdict {
  if (executeCount === 0 && verifyCount === 0) return 'UNMEASURED';
  if (verifyCount === 0) return 'UNKNOWN';
  if (verifyCount < 2) return 'CAUTION';
  if (quotient === null) return 'UNKNOWN';
  if (quotient > 3.0) return 'FOSSILIZED';   // Helix Codex Lock 2: fossilisation pole
  if (quotient >= 1.0) return 'OPTIMAL';
  if (quotient >= 0.5) return 'FLOWING';
  if (quotient >= 0.1) return 'STUCK';
  return 'BURNING';                            // Helix Codex Lock 2: burn pole
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

// ── Identity Continuity — Constitutional Primitive (Ratified 2026-09-08) ──
// Doctrine: /root/AAA/instructions/identity-continuity.md
// ICL-1.1: NO SINGLE WITNESS CARRIES AUTHORITY.
// ICL-1.5: Identity quorum = ∛(W_bio × W_admin × W_const) ≥ 0.50.

export type WitnessKind =
  | 'W1_face'        // Biometric: face signature (InsightFace buffalo_l)
  | 'W2_voice'       // Biometric: voice signature
  | 'W3_name'        // Administrative: handle binding
  | 'W4_history'     // Administrative: VAULT999 + arif_memory L1-L6
  | 'W5_relations'   // Administrative: relations graph
  | 'W6_scar_ledger'; // Constitutional: failure continuity

export type IdentitySupportLevel =
  | 'NONE'
  | 'REFERENCE'
  | 'BINDING_T1'
  | 'BINDING_T2'
  | 'BINDING_T3';

export interface IdentityWitnessSet {
  W1_face: number;          // 0.0-1.0, default 0 (not enrolled)
  W2_voice: number;         // 0.0-1.0, default 0
  W3_name: number;          // 0.0-1.0, default 1.0 (admin)
  W4_history: number;       // 0.0-1.0, default 1.0 (admin)
  W5_relations: number;     // 0.0-1.0, default 0.85
  W6_scar_ledger: number;   // 0.0-1.0, default 0
}

export interface IdentityContinuityReceipt {
  actor_handle: string;
  intent: string;
  named_actor: boolean;
  identity_card_ref?: string;
  witness_set: IdentityWitnessSet;
  biometric_strength: number;       // √(W1 × W2)
  admin_strength: number;           // ∛(W3 × W4 × W5)
  constitutional_strength: number;   // W6
  geometric_mean: number;            // ∛(W_bio × W_admin × W_const)
  quorum_passed: boolean;            // ≥ 0.50 AND single_witness_authority === false
  single_witness_authority: boolean; // always false — ICL-1.1
  verdict: 'MATCH' | 'PARTIAL' | 'NONE';
  rationale?: string;
  timestamp: string;
  receipt_hash: string;
}

/** Update SessionState to track identity continuity per session. */
export interface IdentityContinuityState {
  last_check?: IdentityContinuityReceipt;
  /** Per-actor handle bound to this session. */
  bound_actor_handles: string[];
  /** Pending biometric enrollments. */
  pending_enrollments: string[];
}
