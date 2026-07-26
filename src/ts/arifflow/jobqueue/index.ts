/**
 * arifFLOW Job Queue — Canonical Public API
 * ════════════════════════════════════════════
 *
 * P4 EXTRACTION: Queue ownership moves from A-FORGE → arifFLOW.
 * This is the ONE queue. ONE facade. ONE authority for job orchestration.
 *
 * Every verb emits or prepares a receipt (P1 binding).
 * A-FORGE executes only after arifFLOW dispatches.
 *
 * Usage:
 *   import { getJobQueue } from 'arifFLOW/jobqueue';
 *   const queue = getJobQueue();
 *   const { jobId, isHold } = queue.enqueue({ task: '...', ... });
 *
 * Migration path (old → new):
 *   A-FORGE getAgentManager()  →  arifFLOW getJobQueue()
 *   AgentManager.enqueue()     →  JobQueue.enqueue()
 *   AgentManager.dequeue()     →  JobQueue.claimNextJob()
 *   AgentManager.complete()    →  JobQueue.complete()
 *   AgentManager.fail()        →  JobQueue.fail()
 *   AgentManager.listJobs()    →  JobQueue.listJobs()
 *
 * DITEMPA BUKAN DIBERI — Forged, Not Given.
 * Extracted: 2026-07-26 P4
 * Authority: F13 SOVEREIGN
 */

import { JobQueueScheduler } from './scheduler.js';
import type { QueueConfig } from './types.js';

// Re-export types
export type {
  JobStatus,
  JobPriority,
  JobDefinition,
  JobRunState,
  QueueConfig,
  SchedulerState,
  DispatchPhase,
  QueueMetrics,
  QueueQuery,
  QueueReceiptRef,
  QueueReceiptClass,
  HoldTicket,
  JobToolPolicy,
} from './types.js';

export { JobQueueScheduler } from './scheduler.js';

// ── Singleton ─────────────────────────────────────────────────────────

let _instance: JobQueueScheduler | null = null;

/** Get the singleton JobQueue instance. Uses default config. */
export function getJobQueue(config?: Partial<QueueConfig>): JobQueueScheduler {
  if (!_instance) {
    _instance = new JobQueueScheduler(config);
  }
  return _instance;
}

/** Reset singleton (for testing only). */
export function resetJobQueue(): void {
  _instance?.reset();
  _instance = null;
}

// ── Backward Compatibility Shims (agentManager → jobQueue) ──────────

/**
 * @deprecated Use getJobQueue() instead.
 * Shim for A-FORGE compatibility until full migration.
 */
export function getAgentManagerShim(config?: Partial<QueueConfig>): JobQueueScheduler {
  return getJobQueue(config);
}

/**
 * @deprecated Use JobQueueScheduler.enqueue() directly.
 * Shim for A-FORGE AgentManager.enqueue() compatibility.
 */
export interface AgentManagerShim {
  enqueue: JobQueueScheduler['enqueue'];
  dequeue: (workerId: string) => ReturnType<JobQueueScheduler['claimNextJob']>;
  heartbeat: JobQueueScheduler['heartbeat'];
  complete: JobQueueScheduler['complete'];
  fail: JobQueueScheduler['fail'];
  hold: JobQueueScheduler['hold'];
  listJobs: JobQueueScheduler['listJobs'];
  getRun: JobQueueScheduler['getRun'];
}
