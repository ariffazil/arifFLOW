/**
 * arifFLOW Job Queue Scheduler
 * ════════════════════════════════════════════
 *
 * P4 EXTRACTION: Extracted from A-FORGE/src/domain/reality-loop/engine.ts
 * and A-FORGE/src/application/jobs/AgentManager.ts.
 *
 * The scheduler OWNS:
 *   - Queue intake/enqueue with priority ordering
 *   - Job state transitions (PENDING→RUNNING→COMPLETED/FAILED)
 *   - Dispatch readiness check
 *   - Timeout + expiry detection
 *   - Cooling windows between dispatches
 *   - Receipt emission on every state transition (P1 binding)
 *
 * The scheduler MUST NOT:
 *   - Execute tools directly (that's A-FORGE)
 *   - Issue verdicts (that's arifOS)
 *   - Decide intent routing (that's P3 Router)
 *   - Mutate policy (that's governance)
 *
 * Storage: in-memory Map (ephemeral). All transitions emit receipts
 * to arifFLOW ReceiptEngine for durable proof.
 *
 * DITEMPA BUKAN DIBERI — Forged, Not Given.
 * Extracted: 2026-07-26 P4
 * Authority: F13 SOVEREIGN
 */

import * as crypto from 'node:crypto';

import {
  type JobDefinition,
  type JobRunState,
  type JobStatus,
  type JobPriority,
  type QueueConfig,
  type SchedulerState,
  type DispatchPhase,
  type QueueMetrics,
  type QueueQuery,
  type QueueReceiptRef,
  type QueueReceiptClass,
  type HoldTicket,
  DEFAULT_QUEUE_CONFIG,
} from './types.js';

import { getReceiptEngine } from '../receipt/engine.js';

// ── Helpers ─────────────────────────────────────────────────────────────

function isoNow(): string {
  return new Date().toISOString();
}

function generateId(prefix: string): string {
  const rand = crypto.randomBytes(6).toString('hex');
  return `${prefix}-${Date.now()}-${rand}`;
}

const PRIORITY_ORDER: Record<JobPriority, number> = {
  critical: 0,
  high: 1,
  medium: 2,
  low: 3,
};

// ── Scheduler ────────────────────────────────────────────────────────────

export class JobQueueScheduler {
  private readonly queue: Map<string, JobDefinition> = new Map();
  private readonly runs: Map<string, JobRunState> = new Map();
  private readonly holds: Map<string, HoldTicket> = new Map();
  private readonly receiptRefs: QueueReceiptRef[] = [];
  private readonly config: QueueConfig;
  private state: SchedulerState;

  constructor(config?: Partial<QueueConfig>) {
    this.config = { ...DEFAULT_QUEUE_CONFIG, ...config };
    this.state = this.freshState();
  }

  // ── Public API (matches AgentManager surface) ──────────────────────

  /** Enqueue a job into the queue. Returns jobId + isHold flag. */
  enqueue(
    job: Omit<JobDefinition, 'id' | 'createdAt' | 'enqueuedBy'>,
    enqueuedBy: string = 'arifFLOW',
  ): { jobId: string; isHold: boolean } {
    // Reject if queue at capacity
    if (this.queue.size >= this.config.max_queue_depth) {
      throw new Error(
        `Queue full (${this.queue.size}/${this.config.max_queue_depth}). Rejecting enqueue.`,
      );
    }

    const jobId = generateId('JOB');
    const fullJob: JobDefinition = {
      ...job,
      id: jobId,
      createdAt: isoNow(),
      enqueuedBy,
    };

    this.queue.set(jobId, fullJob);

    const isHold = this.requiresHold(job.priority);
    if (isHold) {
      const ticketId = generateId('HOLD');
      this.holds.set(jobId, {
        ticketId,
        jobId,
        task: job.task,
        priority: job.priority,
        profile: job.profile,
        sessionId: job.sessionId,
        createdAt: isoNow(),
      });
    }

    this.emitReceipt(jobId, 'QUEUE_JOB_ENQUEUED', 'PENDING');
    this.state.pending_count = this.queue.size;
    this.state.phase = this.queue.size > 0 ? 'INTAKE' : 'DRAINED';

    return { jobId, isHold };
  }

  /** Claim the next eligible job for a worker. Returns null if nothing ready. */
  claimNextJob(workerId: string): JobDefinition | null {
    // Sort by priority, filter out low-priority if higher exist
    const sorted = [...this.queue.values()]
      .filter((j) => !this.holds.has(j.id)) // skip held jobs
      .sort((a, b) => PRIORITY_ORDER[a.priority] - PRIORITY_ORDER[b.priority]);

    const job = sorted[0];
    if (!job) return null;

    // Check concurrency
    const runningCount = [...this.runs.values()].filter(
      (r) => r.status === 'RUNNING',
    ).length;
    if (runningCount >= this.config.max_concurrency) return null;

    // Remove from pending queue
    this.queue.delete(job.id);

    // Create session + run state
    const sessionId = job.sessionId ?? `SESSION-${job.id}`;
    const runState: JobRunState = {
      job: { ...job, sessionId },
      status: 'RUNNING',
      startedAt: isoNow(),
      workerId,
      turnsUsed: 0,
    };
    this.runs.set(job.id, runState);

    this.emitReceipt(job.id, 'QUEUE_JOB_CLAIMED', 'RUNNING');
    this.emitReceipt(job.id, 'QUEUE_JOB_DISPATCHED', 'RUNNING');

    this.state.pending_count = this.queue.size;
    this.state.running_count = [...this.runs.values()].filter(
      (r) => r.status === 'RUNNING',
    ).length;
    this.state.active_job_id = job.id;
    this.state.phase = 'DISPATCHING';
    this.state.last_dispatch_at = isoNow();

    return { ...job, sessionId };
  }

  /** Update heartbeat for a running job */
  heartbeat(jobId: string, turnsUsed: number): void {
    const run = this.runs.get(jobId);
    if (run && run.status === 'RUNNING') {
      run.turnsUsed = turnsUsed;
    }
  }

  /** Mark a job as completed */
  complete(jobId: string, summary: string): void {
    const run = this.runs.get(jobId);
    if (!run) return;

    run.status = 'COMPLETED';
    run.completedAt = isoNow();

    this.emitReceipt(jobId, 'QUEUE_JOB_COMPLETED', 'COMPLETED');

    this.runs.delete(jobId);
    this.state.total_processed++;
    this.state.active_job_id = null;
    this.enterCooldown();
  }

  /** Mark a job as failed */
  fail(jobId: string, error: string): void {
    const run = this.runs.get(jobId);
    if (!run) return;

    run.status = 'FAILED';
    run.errorMessage = error;
    run.completedAt = isoNow();

    this.emitReceipt(jobId, 'QUEUE_JOB_FAILED', 'FAILED');

    this.runs.delete(jobId);
    this.state.total_processed++;
    this.state.active_job_id = null;
    this.enterCooldown();
  }

  /** Cancel a job (PENDING or RUNNING) */
  cancel(jobId: string, reason: string): void {
    if (this.queue.has(jobId)) {
      this.queue.delete(jobId);
      this.state.pending_count = this.queue.size;
    }
    const run = this.runs.get(jobId);
    if (run) {
      run.status = 'CANCELLED';
      run.errorMessage = reason;
      run.completedAt = isoNow();
      this.runs.delete(jobId);
      this.state.total_processed++;
    }
    this.state.active_job_id = null;
  }

  /** Place a running job on hold with escalation ticket */
  hold(jobId: string, ticketId: string): void {
    const run = this.runs.get(jobId);
    if (run) {
      run.status = 'PENDING';
      run.holdTicketId = ticketId;
    }
    this.queue.delete(jobId);
    this.state.pending_count = this.queue.size;
  }

  /** Retry a previously failed job */
  retry(jobId: string): { jobId: string } | null {
    const run = this.runs.get(jobId);
    if (!run || (run.status !== 'FAILED' && run.status !== 'CANCELLED')) {
      return null;
    }

    // Re-enqueue with same parameters
    const newJobId = generateId('JOB-RETRY');
    const rehydrated: JobDefinition = {
      ...run.job,
      id: newJobId,
      createdAt: isoNow(),
      enqueuedBy: 'arifFLOW-retry',
      metadata: {
        ...run.job.metadata,
        retry_of: jobId,
        retry_count:
          (typeof run.job.metadata?.retry_count === 'number'
            ? run.job.metadata.retry_count
            : 0) + 1,
      },
    };

    this.queue.set(newJobId, rehydrated);
    this.runs.delete(jobId);

    this.emitReceipt(newJobId, 'QUEUE_JOB_RETRIED', 'PENDING', 'FAILED');
    this.state.pending_count = this.queue.size;

    return { jobId: newJobId };
  }

  // ── Query ─────────────────────────────────────────────────────────

  listJobs(status?: JobStatus): JobDefinition[] {
    if (status === undefined) {
      return [
        ...this.queue.values(),
        ...[...this.runs.values()].map((r) => r.job),
      ];
    }
    if (status === 'PENDING' || status === 'CANCELLED') {
      return [...this.queue.values()];
    }
    return [...this.runs.values()]
      .filter((r) => r.status === status)
      .map((r) => r.job);
  }

  getRun(jobId: string): JobRunState | undefined {
    return this.runs.get(jobId);
  }

  getHold(jobId: string): HoldTicket | undefined {
    return this.holds.get(jobId);
  }

  getHoldTickets(): HoldTicket[] {
    return [...this.holds.values()];
  }

  query(q: QueueQuery): JobDefinition[] {
    let results: JobDefinition[] = [];

    if (q.status) {
      results = this.listJobs(q.status);
    } else {
      results = this.listJobs();
    }

    if (q.priority) {
      results = results.filter((j) => j.priority === q.priority);
    }
    if (q.profile) {
      results = results.filter((j) => j.profile === q.profile);
    }
    if (q.workerId) {
      results = results.filter((j) => {
        const run = this.runs.get(j.id);
        return run?.workerId === q.workerId;
      });
    }
    if (q.since) {
      results = results.filter((j) => j.createdAt >= q.since!);
    }

    const offset = q.offset ?? 0;
    const limit = q.limit ?? 50;
    return results.slice(offset, offset + limit);
  }

  // ── Metrics ────────────────────────────────────────────────────────

  metrics(): QueueMetrics {
    const counts: Record<JobStatus, number> = {
      PENDING: this.queue.size,
      RUNNING: 0,
      COMPLETED: 0,
      FAILED: 0,
      CANCELLED: 0,
    };

    for (const run of this.runs.values()) {
      counts[run.status] = (counts[run.status] ?? 0) + 1;
    }

    return {
      total: this.queue.size + this.runs.size,
      counts,
      openHolds: this.holds.size,
      timestamp: isoNow(),
    };
  }

  // ── Scheduler State ────────────────────────────────────────────────

  getState(): SchedulerState {
    return { ...this.state };
  }

  getPhase(): DispatchPhase {
    return this.state.phase;
  }

  getReceiptRefs(): QueueReceiptRef[] {
    return [...this.receiptRefs];
  }

  isDispatchReady(): boolean {
    if (this.state.phase === 'COOLING') {
      const lastDispatch = this.state.last_dispatch_at
        ? new Date(this.state.last_dispatch_at).getTime()
        : 0;
      if (Date.now() - lastDispatch < this.config.cooldown_ms) {
        return false;
      }
    }
    if (this.queue.size === 0) return false;
    return true;
  }

  getConfig(): QueueConfig {
    return { ...this.config };
  }

  // ── Expiry / GC ────────────────────────────────────────────────────

  /** Expire stale jobs. Returns list of expired job IDs. */
  expireStale(): string[] {
    const now = Date.now();
    const expired: string[] = [];

    for (const [id, job] of this.queue) {
      const age = now - new Date(job.createdAt).getTime();
      if (age > this.config.job_ttl_ms) {
        this.queue.delete(id);
        expired.push(id);
        this.emitReceipt(id, 'QUEUE_JOB_COOLED', 'PENDING');
      }
    }

    this.state.pending_count = this.queue.size;
    return expired;
  }

  /** Reset scheduler to fresh state (testing only). */
  reset(): void {
    this.queue.clear();
    this.runs.clear();
    this.holds.clear();
    this.receiptRefs.length = 0;
    this.state = this.freshState();
  }

  // ── Private ────────────────────────────────────────────────────────

  private requiresHold(priority: JobPriority): boolean {
    const threshold = PRIORITY_ORDER[this.config.hold_threshold];
    return PRIORITY_ORDER[priority] <= threshold;
  }

  private enterCooldown(): void {
    this.state.phase = 'COOLING';
    this.state.running_count = [...this.runs.values()].filter(
      (r) => r.status === 'RUNNING',
    ).length;

    // Auto-transition to IDLE after cooldown
    setTimeout(() => {
      if (this.state.phase === 'COOLING') {
        this.state.phase =
          this.queue.size > 0 ? 'INTAKE' : 'DRAINED';
      }
    }, this.config.cooldown_ms).unref();
  }

  private emitReceipt(
    jobId: string,
    receiptClass: QueueReceiptClass,
    transition: JobStatus,
    previousStatus?: JobStatus,
  ): void {
    if (!this.config.emit_receipts) return;

    try {
      const engine = getReceiptEngine();
      const envelope = engine.emit({
        class: 'FLOW',
        op_id: jobId,
        session_id: `queue-${this.state.started_at.slice(0, 10)}`,
        trace_id: `${jobId}-${receiptClass}`,
        organ: 'arifFLOW',
        result_summary: `Job ${jobId} → ${receiptClass} (${transition})`,
        stage: transition,
      });

      this.receiptRefs.push({
        receipt_id: envelope.receipt_id,
        receipt_class: receiptClass,
        job_id: jobId,
        timestamp: envelope.timestamp,
        transition,
        previous_status: previousStatus,
      });

      this.state.total_receipts++;
    } catch {
      // Receipt emission is best-effort — queue must not fail on receipt errors
    }
  }

  private freshState(): SchedulerState {
    return {
      phase: 'IDLE',
      active_job_id: null,
      pending_count: 0,
      running_count: 0,
      cooldown_ms: this.config.cooldown_ms,
      last_dispatch_at: null,
      total_processed: 0,
      total_receipts: 0,
      started_at: isoNow(),
      max_concurrency: this.config.max_concurrency,
    };
  }
}
