/**
 * arifFLOW Session Store — Cross-Organ Continuity
 * ═══════════════════════════════════════════════
 *
 * Single session continuity across all organs.
 * Replaces:
 *   AAA/state/flow_state.json    → lightweight FQ snapshot
 *   arifOS/kernel_state.py       → heavy KSR state model
 *
 * arifFLOW owns session state. All organs query here.
 * Storage: JSON files at /root/arifFlow/data/sessions/
 *
 * DITEMPA BUKAN DIBERI — Forged, Not Given.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import * as crypto from 'node:crypto';

import {
  type SessionState,
  type SessionStatus,
  type SessionOrganLink,
  type StateStats,
  type FQPulse,
} from './types.js';
import { EMPTY_FQ } from './fq.js';

// ── Configuration ─────────────────────────────────────────────────────

const DEFAULT_DATA_DIR = '/root/arifFlow/data';
const SESSIONS_DIR = 'sessions';

// ── Helpers ────────────────────────────────────────────────────────────

function sessionsDir(dataDir: string): string {
  return path.join(dataDir, SESSIONS_DIR);
}

function sessionPath(dataDir: string, sessionId: string): string {
  return path.join(sessionsDir(dataDir), `${sessionId}.json`);
}

function ensureDir(dir: string): void {
  fs.mkdirSync(dir, { recursive: true });
}

function isoNow(): string {
  return new Date().toISOString();
}

function generateId(): string {
  return `sess-${crypto.randomBytes(12).toString('hex')}`;
}

function writeJSON(filePath: string, data: unknown): void {
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2), 'utf-8');
}

function readJSON<T>(filePath: string): T | null {
  if (!fs.existsSync(filePath)) return null;
  const raw = fs.readFileSync(filePath, 'utf-8');
  return JSON.parse(raw) as T;
}

// ── Session Store ─────────────────────────────────────────────────────

export class SessionStore {
  private dataDir: string;

  constructor(dataDir: string = DEFAULT_DATA_DIR) {
    this.dataDir = dataDir;
    ensureDir(sessionsDir(dataDir));
  }

  // ── Create ──────────────────────────────────────────────────────

  /** Create a new session. Returns the session state. */
  create(params: {
    actor_id: string;
    intent?: string;
    cc_id?: string;
  }): SessionState {
    const sessionId = generateId();
    const now = isoNow();

    const state: SessionState = {
      session_id: sessionId,
      actor_id: params.actor_id,
      status: 'ACTIVE',
      created_at: now,
      last_active_at: now,
      intent: params.intent,
      organs: [],
      fq: EMPTY_FQ,
      total_receipts: 0,
      active_leases: [],
      cc_id: params.cc_id,
    };

    writeJSON(sessionPath(this.dataDir, sessionId), state);
    return state;
  }

  // ── Read ────────────────────────────────────────────────────────

  /** Get session by ID */
  get(sessionId: string): SessionState | null {
    return readJSON<SessionState>(sessionPath(this.dataDir, sessionId));
  }

  // ── Update ──────────────────────────────────────────────────────

  /** Update session status */
  setStatus(sessionId: string, status: SessionStatus): SessionState | null {
    const state = this.get(sessionId);
    if (!state) return null;

    state.status = status;
    state.last_active_at = isoNow();
    writeJSON(sessionPath(this.dataDir, sessionId), state);
    return state;
  }

  /** Link an organ to this session */
  linkOrgan(
    sessionId: string,
    link: Omit<SessionOrganLink, 'last_seen' | 'receipt_count'>,
  ): SessionState | null {
    const state = this.get(sessionId);
    if (!state) return null;

    const existing = state.organs.findIndex(o => o.organ === link.organ);
    const fullLink: SessionOrganLink = {
      ...link,
      last_seen: isoNow(),
      receipt_count: 0,
    };

    if (existing >= 0) {
      state.organs[existing] = fullLink;
    } else {
      state.organs.push(fullLink);
    }

    state.last_active_at = isoNow();
    writeJSON(sessionPath(this.dataDir, sessionId), state);
    return state;
  }

  /** Update organ receipt count */
  bumpOrganReceipts(sessionId: string, organ: string): SessionState | null {
    const state = this.get(sessionId);
    if (!state) return null;

    const link = state.organs.find(o => o.organ === organ);
    if (link) {
      link.receipt_count++;
      link.last_seen = isoNow();
    }

    state.total_receipts++;
    state.last_active_at = isoNow();
    writeJSON(sessionPath(this.dataDir, sessionId), state);
    return state;
  }

  /** Update FQ pulse for this session */
  updateFQ(sessionId: string, fq: FQPulse): SessionState | null {
    const state = this.get(sessionId);
    if (!state) return null;

    state.fq = fq;
    state.last_active_at = isoNow();
    writeJSON(sessionPath(this.dataDir, sessionId), state);
    return state;
  }

  /** Add an active lease */
  addLease(sessionId: string, leaseId: string): SessionState | null {
    const state = this.get(sessionId);
    if (!state) return null;

    if (!state.active_leases.includes(leaseId)) {
      state.active_leases.push(leaseId);
    }
    state.last_active_at = isoNow();
    writeJSON(sessionPath(this.dataDir, sessionId), state);
    return state;
  }

  /** Remove a lease */
  removeLease(sessionId: string, leaseId: string): SessionState | null {
    const state = this.get(sessionId);
    if (!state) return null;

    state.active_leases = state.active_leases.filter(l => l !== leaseId);
    state.last_active_at = isoNow();
    writeJSON(sessionPath(this.dataDir, sessionId), state);
    return state;
  }

  // ── Stats ───────────────────────────────────────────────────────

  /** Compute session store statistics */
  stats(): StateStats {
    ensureDir(sessionsDir(this.dataDir));
    const files = fs.readdirSync(sessionsDir(this.dataDir))
      .filter(f => f.endsWith('.json'));

    const sessions: SessionState[] = [];
    for (const f of files) {
      const s = readJSON<SessionState>(
        path.join(sessionsDir(this.dataDir), f),
      );
      if (s) sessions.push(s);
    }

    const active = sessions.filter(s => s.status === 'ACTIVE' || s.status === 'IDLE');
    const degraded = sessions.filter(s => s.status === 'DEGRADED');
    const orphaned = sessions.filter(s => s.status === 'ORPHANED');

    const fqs = sessions
      .map(s => s.fq.quotient)
      .filter(q => q > 0);

    const fqDist: Record<string, number> = {
      OPTIMAL: 0,
      BALANCED: 0,
      WATCHING: 0,
      STUCK: 0,
      UNMEASURED: 0,
    };
    for (const s of sessions) {
      fqDist[s.fq.verdict] = (fqDist[s.fq.verdict] ?? 0) + 1;
    }

    return {
      total_sessions: sessions.length,
      active_sessions: active.length,
      degraded_sessions: degraded.length,
      orphaned_sessions: orphaned.length,
      average_fq: fqs.length > 0
        ? Math.round((fqs.reduce((a, b) => a + b, 0) / fqs.length) * 100) / 100
        : 0,
      fq_distribution: fqDist as StateStats['fq_distribution'],
      last_session_at: sessions
        .sort((a, b) => b.last_active_at.localeCompare(a.last_active_at))[0]
        ?.last_active_at,
    };
  }

  /** List all session IDs */
  list(): string[] {
    ensureDir(sessionsDir(this.dataDir));
    return fs.readdirSync(sessionsDir(this.dataDir))
      .filter(f => f.endsWith('.json'))
      .map(f => f.replace('.json', ''));
  }
}

// ── Singleton ─────────────────────────────────────────────────────────

let _instance: SessionStore | null = null;

export function getSessionStore(dataDir?: string): SessionStore {
  if (!_instance) {
    _instance = new SessionStore(dataDir);
  }
  return _instance;
}

export function resetSessionStore(): void {
  _instance = null;
}
