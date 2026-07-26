/**
 * arifFLOW Job Queue Layer — Canonical Types
 * ════════════════════════════════════════════
 *
 * P4 EXTRACTION: Extracted from A-FORGE/src/application/jobs/AgentManager.ts
 * and A-FORGE/src/domain/reality-loop/engine.ts. Queue ownership moves from
 * A-FORGE (actuator) → arifFLOW (flow orchestration).
 *
 * ONE queue. ONE scheduler. ONE receipt path per transition.
 * A-FORGE executes approved jobs only — arifFLOW owns when/how.
 *
 * Storage: receipt-anchored via arifFLOW ReceiptEngine (P1 live).
 * Hash chain: every job transition emits a FlowReceipt.
 *
 * DITEMPA BUKAN DIBERI — Forged, Not Given.
 * Extracted: 2026-07-26 P4
 * Authority: F13 SOVEREIGN — Arif GO signal given
 */

// ── Job Identity ────────────────────────────────────────────────────────

/** Job statuses extracted from AgentManager JobStatus */
export type JobStatus = 'PENDING' | 'RUNNING' | 'COMPLETED' | 'FAILED' | 'CANCELLED';

/** Job priority extracted from AgentManager JobPriority */
export type JobPriority = 'low' | 'medium' | 'high' | 'critical';

/** A job definition — extracted from AgentManager JobDefinition */
export interface JobDefinition {
  id: string;
  task: string;
  profile: string;
  priority: JobPriority;
  toolAllowList?: string[];
  blockedToolPatterns?: string[];
  sessionId?: string;
  createdAt: string;
  enqueuedBy: string;
  maxTurns?: number;
  metadata?: Record<string, unknown>;
}

/** Job runtime state — extracted from AgentManager JobRunState */
export interface JobRunState {
  job: JobDefinition;
  status: JobStatus;
  startedAt?: string;
  completedAt?: string;
  workerId?: string;
  turnsUsed?: number;
  errorMessage?: string;
  holdTicketId?: string;
}

// ── Queue Events (Receipt-Anchored) ─────────────────────────────────────

/** Receipt classes for queue transitions. P1 receipt binding. */
export type QueueReceiptClass =
  | 'QUEUE_JOB_ENQUEUED'
  | 'QUEUE_JOB_CLAIMED'
  | 'QUEUE_JOB_DISPATCHED'
  | 'QUEUE_JOB_COMPLETED'
  | 'QUEUE_JOB_FAILED'
  | 'QUEUE_JOB_COOLED'
  | 'QUEUE_JOB_RETRIED';

export interface QueueReceiptRef {
  receipt_id: string;
  receipt_class: QueueReceiptClass;
  job_id: string;
  timestamp: string;
  transition: JobStatus;
  previous_status?: JobStatus;
}

// ── Tool Policy (extracted from AgentManager config) ────────────────────

export interface JobToolPolicy {
  defaultAllowList: string[];
  blockedPatterns: string[];
}

/** Default tool policy — mirrors AgentManager.getToolAllowList() */
export const DEFAULT_TOOL_ALLOW_LIST: string[] = [
  'read_file', 'write_file', 'list_files', 'grep_text',
  'run_tests', 'run_command',
];

/** Default blocked patterns — mirrors AgentManager.getBlockedPatterns() */
export const DEFAULT_BLOCKED_PATTERNS: string[] = [
  'rm -rf', 'shutdown', 'reboot', 'mkfs', 'dd ',
  'git reset --hard', 'curl ', 'wget ', '>:',
];

// ── Hold / Escalation ───────────────────────────────────────────────────

export interface HoldTicket {
  ticketId: string;
  jobId: string;
  task: string;
  priority: JobPriority;
  profile: string;
  sessionId?: string;
  createdAt: string;
  resolvedAt?: string;
  resolution?: 'APPROVED' | 'DENIED';
}

// ── Scheduler State (extracted from Reality Loop engine patterns) ───────

/** Scheduler dispatch state — who owns what */
export type DispatchPhase =
  | 'IDLE'         // No active jobs
  | 'INTAKE'       // Accepting new jobs
  | 'DISPATCHING'  // Claiming + sending to A-FORGE
  | 'WAITING'      // Job dispatched, awaiting execution result
  | 'COOLING'      // Job completed/failed, cooling before next dispatch
  | 'DRAINED';     // All jobs processed, queue empty

export interface SchedulerState {
  /** Current dispatch phase */
  phase: DispatchPhase;

  /** Active job being dispatched (null if IDLE/DRAINED) */
  active_job_id: string | null;

  /** Queue depth */
  pending_count: number;
  running_count: number;

  /** Cooling window — ms to wait between dispatches */
  cooldown_ms: number;

  /** Last dispatch timestamp */
  last_dispatch_at: string | null;

  /** Total jobs processed since scheduler start */
  total_processed: number;

  /** Total receipts emitted by this scheduler */
  total_receipts: number;

  /** Scheduler start time */
  started_at: string;

  /** Max concurrent running jobs */
  max_concurrency: number;
}

// ── Queue Metrics (extracted from jobsRoutes /metrics) ──────────────────

export interface QueueMetrics {
  total: number;
  counts: Record<JobStatus, number>;
  openHolds: number;
  timestamp: string;
}

// ── Queue Query ─────────────────────────────────────────────────────────

export interface QueueQuery {
  status?: JobStatus;
  priority?: JobPriority;
  profile?: string;
  workerId?: string;
  since?: string;
  limit?: number;
  offset?: number;
}

// ── Queue Config ────────────────────────────────────────────────────────

export interface QueueConfig {
  /** Max jobs allowed in queue before rejecting enqueue */
  max_queue_depth: number;

  /** Max concurrent running jobs */
  max_concurrency: number;

  /** Cooldown between dispatches (ms) */
  cooldown_ms: number;

  /** Jobs older than this (ms) auto-expire */
  job_ttl_ms: number;

  /** Auto-hold threshold: jobs with this priority or higher require 888_HOLD */
  hold_threshold: JobPriority;

  /** Tool policy */
  toolPolicy: JobToolPolicy;

  /** Receipt emission toggle */
  emit_receipts: boolean;
}

export const DEFAULT_QUEUE_CONFIG: QueueConfig = {
  max_queue_depth: 200,
  max_concurrency: 5,
  cooldown_ms: 500,
  job_ttl_ms: 24 * 60 * 60 * 1000, // 24h
  hold_threshold: 'high',
  toolPolicy: {
    defaultAllowList: DEFAULT_TOOL_ALLOW_LIST,
    blockedPatterns: DEFAULT_BLOCKED_PATTERNS,
  },
  emit_receipts: true,
};
