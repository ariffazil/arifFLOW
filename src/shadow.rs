//! Shadow observation plane — **NON-AUTHORITATIVE**.
//!
//! Hard invariant (P1, 2026-11-22): this module NEVER writes, mutates, or
//! shares state with any enforcement path — hold, throttle, release, admission
//! or constitutional verdict. It receives receipts by immutable reference and
//! returns a value. The enforced metric is computed by the existing code path
//! and is never modified by anything here.
//!
//! Purpose: compute the NEXT candidate rule (session-bound witness eligibility)
//! alongside the CURRENT rule over the SAME population and window, so the
//! divergence on live traffic is observable before anything is enforced.
//!
//! Rule under observation (shadow.session-bound.v1):
//!   eligible Verify := not mechanical_self_verify
//!                    AND its session carries a counting execution
//!                       (Execute|Merge with risk_class != T0Observe)
//!                    AND its receipt_id was not already counted in this window
//!
//! Rule in force today (enforced): every Verify except mechanical_self_verify
//! counts; execute side excludes T0Observe. The two differ exactly by the
//! session-binding and replay guards.

use crate::receipt::{FlowReceipt, RiskClass, StepType};
use serde::Serialize;
use sha3::{Digest, Sha3_256};

/// Version of the OBSERVED rule set. Bump when the shadow rule changes.
pub const SHADOW_RULES_VERSION: &str = "shadow.session-bound.v1";
/// Version of the enforced rule set (for reference in the divergence report).
pub const ENFORCED_RULES_VERSION: &str = "enforced.mechanical-exclusion.v1";
/// Formula used by both sides of the comparison.
pub const FORMULA_VERSION: &str = "count-ratio-v3";
/// This plane has no authority. Always true; present so a reader never has to
/// infer it from the field names.
pub const NON_AUTHORITATIVE: bool = true;

/// Per-actor counts under one rule.
#[derive(Debug, Clone, Serialize)]
pub struct ActorShadow {
    pub actor_id: String,
    pub enforced_executes: u64,
    pub enforced_verifies: u64,
    pub shadow_executes: u64,
    pub shadow_verifies: u64,
    pub excluded_session_orphan: u64,
    pub excluded_duplicate: u64,
}

impl ActorShadow {
    pub fn enforced_quotient(&self) -> Option<f64> {
        if self.enforced_verifies == 0 || self.enforced_executes == 0 {
            None
        } else {
            Some(self.enforced_verifies as f64 / self.enforced_executes as f64)
        }
    }
    pub fn shadow_quotient(&self) -> Option<f64> {
        if self.shadow_verifies == 0 || self.shadow_executes == 0 {
            None
        } else {
            Some(self.shadow_verifies as f64 / self.shadow_executes as f64)
        }
    }
}

/// The shadow frame: everything an auditor needs to read the comparison.
#[derive(Debug, Clone, Serialize)]
pub struct ShadowFrame {
    /// Always true. This plane observes; it does not decide or enforce.
    pub non_authoritative: bool,
    /// Version of the observed (shadow) rule set.
    pub rules_version: String,
    /// Version of the enforced rule set for comparison.
    pub enforced_rules_version: String,
    /// Formula both sides use.
    pub formula_version: String,
    /// Population both sides are computed over.
    pub population: String,
    pub window_kind: String,
    pub window_size: usize,
    pub window_start_utc: Option<String>,
    pub window_end_utc: Option<String>,
    pub first_receipt_id: Option<String>,
    pub last_receipt_id: Option<String>,
    /// Deterministic fingerprint of this frame's inputs.
    pub frame_hash: String,
    pub calculated_at_utc: String,
    pub snapshot_age_ms: Option<u64>,
    pub freshness_class: &'static str,
    /// Enforced vs shadow, same population, same window.
    pub enforced_executes: u64,
    pub enforced_verifies: u64,
    pub enforced_quotient: Option<f64>,
    pub shadow_executes: u64,
    pub shadow_verifies: u64,
    pub shadow_quotient: Option<f64>,
    pub quotient_delta: Option<f64>,
    pub excluded_session_orphan: u64,
    pub excluded_duplicate: u64,
    pub per_actor: Vec<ActorShadow>,
    /// Why the state may differ — observation only.
    pub transition_cause: String,
    /// What would release the observed state — observation only.
    pub release_condition: String,
    /// Explicit statement of what this plane does NOT do.
    pub enforcement_effect: String,
}

const FRESHNESS_LIVE_MAX_MS: u64 = 5_000;
const FRESHNESS_RECENT_MAX_MS: u64 = 13_000;

fn freshness_class(age_ms: u64) -> &'static str {
    if age_ms < FRESHNESS_LIVE_MAX_MS {
        "LIVE"
    } else if age_ms < FRESHNESS_RECENT_MAX_MS {
        "RECENT"
    } else {
        "STALE"
    }
}

fn is_counting_execution(r: &FlowReceipt) -> bool {
    r.step_type.is_work_execution() && r.risk_class != RiskClass::T0Observe
}

/// Compute the shadow frame. Pure: takes receipts by reference, returns a value.
pub fn observe(window: &[FlowReceipt], window_size: usize) -> ShadowFrame {
    // Pass 1 — sessions carrying a counting execution (physical order).
    let mut work_sessions: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for r in window {
        if is_counting_execution(r) {
            work_sessions.insert(r.session_id.as_str());
        }
    }

    // Pass 2 — count under both rules over the same population.
    let mut actors: std::collections::BTreeMap<String, ActorShadow> =
        std::collections::BTreeMap::new();
    let mut counted_ids: std::collections::HashSet<uuid::Uuid> = std::collections::HashSet::new();
    let (mut e_exec, mut e_ver, mut s_exec, mut s_ver) = (0u64, 0u64, 0u64, 0u64);
    let (mut excl_orphan, mut excl_dup) = (0u64, 0u64);

    for r in window {
        let a = r.actor_id.clone();
        let entry = actors.entry(a.clone()).or_insert_with(|| ActorShadow {
            actor_id: a.clone(),
            enforced_executes: 0,
            enforced_verifies: 0,
            shadow_executes: 0,
            shadow_verifies: 0,
            excluded_session_orphan: 0,
            excluded_duplicate: 0,
        });
        if is_counting_execution(r) {
            e_exec += 1;
            s_exec += 1;
            entry.enforced_executes += 1;
            entry.shadow_executes += 1;
            continue;
        }
        if !r.step_type.is_verification() {
            continue;
        }
        // --- enforced rule (today) ---
        if !r.mechanical_self_verify {
            e_ver += 1;
            entry.enforced_verifies += 1;
        }
        // --- shadow rule (observed) ---
        if r.mechanical_self_verify {
            continue;
        }
        if !work_sessions.contains(r.session_id.as_str()) {
            excl_orphan += 1;
            entry.excluded_session_orphan += 1;
            continue;
        }
        if !counted_ids.insert(r.receipt_id) {
            excl_dup += 1;
            entry.excluded_duplicate += 1;
            continue;
        }
        s_ver += 1;
        entry.shadow_verifies += 1;
    }

    let enforced_q = if e_ver == 0 || e_exec == 0 {
        None
    } else {
        Some(e_ver as f64 / e_exec as f64)
    };
    let shadow_q = if s_ver == 0 || s_exec == 0 {
        None
    } else {
        Some(s_ver as f64 / s_exec as f64)
    };
    let delta = match (enforced_q, shadow_q) {
        (Some(a), Some(b)) => Some(b - a),
        _ => None,
    };

    let first_id = window.first().map(|r| r.receipt_id.to_string());
    let last_id = window.last().map(|r| r.receipt_id.to_string());
    let start = window.first().map(|r| r.created_at.to_rfc3339());
    let end = window.last().map(|r| r.created_at.to_rfc3339());
    let newest = window.last().map(|r| r.created_at);
    let age_ms = newest.map(|t| {
        let d = chrono::Utc::now().signed_duration_since(t).num_milliseconds();
        if d < 0 { 0 } else { d as u64 }
    });

    // Deterministic frame fingerprint over the inputs that define the frame.
    let mut h = Sha3_256::new();
    h.update(FORMULA_VERSION.as_bytes());
    h.update(b"|");
    h.update(SHADOW_RULES_VERSION.as_bytes());
    h.update(b"|");
    h.update(ENFORCED_RULES_VERSION.as_bytes());
    h.update(b"|");
    h.update(window_size.to_string().as_bytes());
    h.update(b"|");
    if let Some(id) = &first_id {
        h.update(id.as_bytes());
    }
    h.update(b"|");
    if let Some(id) = &last_id {
        h.update(id.as_bytes());
    }
    h.update(b"|");
    h.update(e_exec.to_string().as_bytes());
    h.update(b":");
    h.update(e_ver.to_string().as_bytes());
    h.update(b"|");
    h.update(s_exec.to_string().as_bytes());
    h.update(b":");
    h.update(s_ver.to_string().as_bytes());
    let frame_hash = hex::encode(h.finalize());

    ShadowFrame {
        non_authoritative: NON_AUTHORITATIVE,
        rules_version: SHADOW_RULES_VERSION.to_string(),
        enforced_rules_version: ENFORCED_RULES_VERSION.to_string(),
        formula_version: FORMULA_VERSION.to_string(),
        population: "receipt_window (all actors)".to_string(),
        window_kind: "sliding_receipts".to_string(),
        window_size,
        window_start_utc: start,
        window_end_utc: end,
        first_receipt_id: first_id,
        last_receipt_id: last_id,
        frame_hash,
        calculated_at_utc: chrono::Utc::now().to_rfc3339(),
        snapshot_age_ms: age_ms,
        freshness_class: freshness_class(age_ms.unwrap_or(0)),
        enforced_executes: e_exec,
        enforced_verifies: e_ver,
        enforced_quotient: enforced_q,
        shadow_executes: s_exec,
        shadow_verifies: s_ver,
        shadow_quotient: shadow_q,
        quotient_delta: delta,
        excluded_session_orphan: excl_orphan,
        excluded_duplicate: excl_dup,
        per_actor: actors.into_values().collect(),
        transition_cause: format!(
            "shadow_only_no_enforcement_effect; session-orphan exclusions={}, replay exclusions={}",
            excl_orphan, excl_dup
        ),
        release_condition:
            "NONE — the shadow plane never releases anything. Holds, throttles and releases remain \
             governed by the enforced rule set and by arifOS judgment."
                .to_string(),
        enforcement_effect: "NONE — this frame is a read-only projection; it cannot hold, throttle, \
             release, admit or judge."
            .to_string(),
    }
}
