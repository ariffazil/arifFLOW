/**
 * arifFLOW Flow Quotient (FQ) — Real-time Pulse
 * ════════════════════════════════════════════
 *
 * Mirrors the Rust FlowQuotient::compute() in src/receipt.rs.
 * Cross-language contract: same formula, same thresholds, same verdicts.
 *
 * v2.1 (2026-08-05): FQ = verify_count / execute_count (count-based, inverted)
 * Prior v2.0: FQ = execute_cost / verify_cost (cost-based)
 *
 * formula_hash: sha256:arifflow-fq-v2.1-2026-08-05
 * formula_version: qg.v0.2
 *
 * DITEMPA BUKAN DIBERI — Forged, Not Given.
 */

import { type FQPulse, fqVerdict } from './types.js';

// ── FQ Computation ─────────────────────────────────────────────────────

export interface FQInput {
  execute_cost_ns: number;
  verify_cost_ns: number;
  preceding_verify_cost_ns?: number;
}

/**
 * Compute Flow Quotient from a window of step costs.
 *
 * Mirrors `FlowQuotient::compute()` in src/receipt.rs exactly:
 * - execute_cost = sum of all execution step costs
 * - verify_cost = sum of all verification step costs + preceding_verify
 * - quotient = execute_cost / verify_cost
 *
 * @param steps — Array of step cost records in the window
 * @returns FQPulse with verdict
 */
export function computeFQ(steps: FQInput[]): FQPulse {
  let execute_count = 0;
  let verify_count = 0;

  for (const step of steps) {
    if (step.execute_cost_ns > 0) {
      execute_count++;
    }
    if (step.verify_cost_ns > 0) {
      verify_count++;
    }
  }

  // v2.1: count-based quotient = verify / execute
  // null when verify_count == 0 (undefined — not 0, not ∞)
  const quotient: number | null = verify_count === 0 || execute_count === 0
    ? null
    : Math.round((verify_count / execute_count) * 100) / 100;

  const verdict = (() => {
    if (execute_count === 0 && verify_count === 0) return 'UNMEASURED' as const;
    if (verify_count === 0) return 'UNKNOWN' as const;
    if (verify_count < 2) return 'CAUTION' as const;
    const q = quotient ?? 0;
    if (q >= 1.0) return 'OPTIMAL' as const;
    if (q >= 0.5) return 'FLOWING' as const;
    if (q >= 0.1) return 'STUCK' as const;
    return 'BURNING' as const;
  })();

  return {
    execute_count,
    verify_count,
    quotient,
    verdict,
    window_size: steps.length,
    computed_at: new Date().toISOString(),
  };
}

/**
 * Compute FQ from the arifFLOW receipt store.
 * Queries recent receipts and computes the FQ pulse.
 *
 * @param getSteps — Function that returns recent step cost records
 * @param windowSize — Max number of steps to include (default 100)
 */
export function computeFQFromStore(
  getSteps: (limit: number) => FQInput[],
  windowSize: number = 100,
): FQPulse {
  const steps = getSteps(windowSize);
  return computeFQ(steps);
}

/**
 * Default FQ pulse for fresh sessions (no data yet).
 */
export const EMPTY_FQ: FQPulse = {
  execute_count: 0,
  verify_count: 0,
  quotient: 0,
  verdict: 'UNMEASURED',
  window_size: 0,
  computed_at: new Date().toISOString(),
};
