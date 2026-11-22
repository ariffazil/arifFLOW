//! Constitutional grammar separation (WS2, 2026-11-22).
//!
//! Three vocabularies, three owners. arifFlow emits only the first and the
//! third. The second belongs to arifOS and is NEVER emitted from this crate —
//! the type exists here so the boundary is visible in the type system, not
//! only in prose.
//!
//! Before this module, `/check` answered with a bare `"Hold"` — the same word
//! arifOS uses for a constitutional HOLD. Two different authorities, one
//! string. A caller could not tell whether the FLOW plane blocked an actor or
//! the LAW plane refused an action.

use serde::{Deserialize, Deserializer, Serialize};

/// arifFlow's own gate state — may this actor move right now (flow plane).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MetabolicGateState {
    #[serde(rename = "ALLOW")]
    Allow,
    #[serde(rename = "FLOW_BLOCKED")]
    FlowBlocked,
    #[serde(rename = "THROTTLED")]
    Throttled,
    #[serde(rename = "COOLDOWN")]
    Cooldown,
}

impl MetabolicGateState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Allow => "ALLOW",
            Self::FlowBlocked => "FLOW_BLOCKED",
            Self::Throttled => "THROTTLED",
            Self::Cooldown => "COOLDOWN",
        }
    }

    /// Map the daemon's internal gate action (Debug-formatted) onto the
    /// canonical vocabulary. Unknown actions fail CLOSED — a state we cannot
    /// name must never be reported as permission to move.
    pub fn from_action(action: &str) -> Self {
        match action {
            "Allow" | "ALLOW" | "Allowed" => Self::Allow,
            "Throttle" | "THROTTLE" | "Throttled" => Self::Throttled,
            "Cooldown" | "COOLDOWN" => Self::Cooldown,
            _ => Self::FlowBlocked,
        }
    }

    /// [GRAMMAR 2026-11-22] Map a wire label onto the canonical vocabulary.
    /// Unknown labels fail CLOSED, by the same rule as `from_action`: a gate
    /// state we cannot name must never be reported as permission to move.
    /// Note what is NOT accepted here: arifOS's constitutional words (SEAL,
    /// HOLD, SABAR, VOID). A caller that speaks those is borrowing another
    /// organ's authority and is failed closed rather than translated.
    pub fn from_label(raw: &str) -> Self {
        match raw.trim().to_ascii_uppercase().as_str() {
            "ALLOW" | "ALLOWED" => Self::Allow,
            "FLOW_BLOCKED" => Self::FlowBlocked,
            "THROTTLED" | "THROTTLE" => Self::Throttled,
            "COOLDOWN" => Self::Cooldown,
            _ => Self::FlowBlocked,
        }
    }
}

/// [GRAMMAR 2026-11-22] Tolerant deserialiser for the optional gate-state field.
/// An unknown label becomes FLOW_BLOCKED (fail closed) instead of rejecting the
/// whole receipt — one unreadable field must not destroy the evidence, but it
/// must never widen permission either.
pub fn deserialize_gate_state_opt<'de, D>(deserializer: D) -> Result<Option<MetabolicGateState>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.map(|s| MetabolicGateState::from_label(&s)))
}

/// Observed diagnosis — what the flow plane measured. An observation, not a
/// verdict: no authority follows from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ObservedDiagnosis {
    #[serde(rename = "FLOWING")]
    Flowing,
    #[serde(rename = "COHERENCE_FRACTURE")]
    CoherenceFracture,
    #[serde(rename = "SIMULATION")]
    Simulation,
    #[serde(rename = "PATHOLOGICAL")]
    Pathological,
}

impl ObservedDiagnosis {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Flowing => "FLOWING",
            Self::CoherenceFracture => "COHERENCE_FRACTURE",
            Self::Simulation => "SIMULATION",
            Self::Pathological => "PATHOLOGICAL",
        }
    }
}

/// arifOS's verdict grammar (SEAL / HOLD / SABAR / VOID).
///
/// REFERENCE ONLY. arifFlow never emits these and never interprets them —
/// F2 (checkpoint, never judge) and F5 (receipt, never own memory).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ConstitutionalVerdict {
    #[serde(rename = "SEAL")]
    Seal,
    #[serde(rename = "HOLD")]
    Hold,
    #[serde(rename = "SABAR")]
    Sabar,
    #[serde(rename = "VOID")]
    Void,
}

impl ConstitutionalVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Seal => "SEAL",
            Self::Hold => "HOLD",
            Self::Sabar => "SABAR",
            Self::Void => "VOID",
        }
    }
}

/// The grammar manifest served by `/check` and `/health`: who owns what.
pub fn grammar_manifest() -> serde_json::Value {
    serde_json::json!({
        "metabolic_gate_state": {
            "owner": "arifFlow",
            "values": ["ALLOW", "FLOW_BLOCKED", "THROTTLED", "COOLDOWN"],
            "meaning": "may this actor move right now (flow plane decision)",
        },
        "constitutional_verdict": {
            "owner": "arifOS",
            "values": ["SEAL", "HOLD", "SABAR", "VOID"],
            "meaning": "constitutional authority (law plane) — never emitted by arifFlow",
        },
        "observed_diagnosis": {
            "owner": "arifFlow",
            "values": ["FLOWING", "COHERENCE_FRACTURE", "SIMULATION", "PATHOLOGICAL"],
            "meaning": "what the flow plane measured — an observation, not a verdict",
        },
    })
}

/// The governed lifecycle an emitter may declare (WS4).
///
/// v1 (2026-11-22): the DEPLOYED → OUTCOME_WITNESSED transition is ENFORCED
/// (an external witness must be named). Ordering across the remaining edges is
/// declared by the emitter and not yet enforced by the daemon — enforcement
/// needs per-session lifecycle state, which is deferred to v2.
pub const LIFECYCLE_STATES: [&str; 9] = [
    "PROPOSED",
    "AUTHORIZED",
    "EXECUTED",
    "DEPLOYED",
    "OUTCOME_WITNESSED",
    "HUMAN_ACCEPTED",
    "HUMAN_REJECTED",
    "HUMAN_DEFERRED",
    "CLOSED",
];

/// WS4 independent witness gate.
///
/// A receipt may declare `OUTCOME_WITNESSED` only if it names an external
/// witness. A builder cannot witness its own outcome: without this, "closed"
/// is a self-report, which is the exact failure the receipt chain exists to
/// prevent. Evaluated BEFORE any state mutation, so a rejected receipt leaves
/// no trace in the ledger, the enforcer, or the vault.
///
/// Returns `Err(reason)` to reject the receipt; `Ok(())` to admit it.
pub fn witness_gate(raw: &str) -> Result<(), String> {
    let v: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        // Unparseable bodies are rejected later by the normal parse path.
        Err(_) => return Ok(()),
    };
    let payload = match v.get("payload") {
        Some(p) if p.is_object() => p,
        _ => return Ok(()),
    };
    let state = payload
        .get("receipt_state")
        .and_then(|s| s.as_str())
        .unwrap_or("");
    if state == "OUTCOME_WITNESSED" {
        let witness = payload
            .get("witnessed_by")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .trim();
        if witness.is_empty() {
            return Err(
                "OUTCOME_WITNESSED_WITNESS_REQUIRED: receipt declares OUTCOME_WITNESSED but names \
                 no external witness (payload.witnessed_by). A builder cannot witness its own outcome."
                    .to_string(),
            );
        }
    }
    Ok(())
}
