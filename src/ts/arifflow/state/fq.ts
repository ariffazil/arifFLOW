/**
 * arifFLOW Flow Quotient (FQ) — Real-time Pulse
 * ════════════════════════════════════════════
 *
 * Mirrors the Rust FlowQuotient::compute() in src/receipt.rs.
 * Cross-language contract: same formula, same thresholds, same verdicts.
 *
 * FQ = Σ(Execute.cost_ns) / Σ(Verify.cost_ns + preceding_verify_cost_ns)
 *
 * This is the primary metric for whether an agent is in flow
 * or trapped in self-monitoring. Feeds the observatory dashboard.
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
  let execute_cost = 0;
  let verify_cost = 0;
  let execute_count = 0;
  let verify_count = 0;

  for (const step of steps) {
    if (step.execute_cost_ns > 0) {
      execute_cost += step.execute_cost_ns;
      execute_count++;
    }
    if (step.verify_cost_ns > 0) {
      verify_cost += step.verify_cost_ns;
      verify_count++;
    }
    if (step.preceding_verify_cost_ns) {
      verify_cost += step.preceding_verify_cost_ns;
    }
  }

  const quotient = (() => {
    if (verify_cost === 0) {
      // No verification cost: pure execution (optimal but suspicious)
      // or no receipts yet
      return execute_cost > 0 ? Number.MAX_VALUE : 0;
    }
    return execute_cost / verify_cost;
  })();

  const clampedQuotient = quotient === Number.MAX_VALUE ? 999.0 : quotient;

  return {
    execute_count,
    verify_count,
    quotient: Math.round(clampedQuotient * 100) / 100, // 2 decimal places
    verdict: fqVerdict(clampedQuotient, execute_count, verify_count),
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
