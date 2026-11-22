//! SOT provenance manifest + explicit daemon-path capability state
//! (WS1 / WS3, 2026-11-22).
//!
//! Two failure classes are addressed here:
//!
//! 1. **Provenance drift.** The live daemon could not say which source commit,
//!    which binary, or which formula version it was running. `/health` now
//!    carries a manifest that binds all of it in one machine-readable block.
//! 2. **Silent claim drift.** The public README listed capabilities that are
//!    compiled but NOT invoked on the daemon path. Instead of wiring them into
//!    a live production path without a gated window, they are now surfaced
//!    explicitly in `/health` with their real status.

use serde::Serialize;

pub const SCHEMA_VERSION: &str = "arifFlow.manifest.v1";
pub const FORMULA_VERSION: &str = "qg.v0.3.1-vector";
pub const RECEIPT_FORMAT_VERSION: &str = "FlowReceipt.v1";
pub const POLICY_VERSION_DEFAULT: &str = "fq_policy.yaml:F13_RATIFIED_2026-09-12";

#[derive(Debug, Clone, Serialize)]
pub struct ProvenanceManifest {
    /// Version of this manifest schema itself.
    pub schema_version: String,
    /// Source commit the running binary was built from.
    pub repo_sha: String,
    /// Human-facing release id (crate version unless overridden at deploy).
    pub release_id: String,
    /// Digest of the running binary.
    pub binary_digest_hex: String,
    /// Which digest algorithm produced `binary_digest_hex`.
    pub digest_algorithm: String,
    /// The FQ / 7D-vector formula in force.
    pub formula_version: String,
    /// The FQ policy in force.
    pub policy_version: String,
    /// Receipt format version (hash chain + JCS body).
    pub receipt_format_version: String,
    /// When this process started.
    pub deployed_at: String,
    /// Who witnessed this deployment.
    pub witnessed_by: String,
}

impl ProvenanceManifest {
    pub fn current() -> Self {
        let (digest, algo) = binary_digest();
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            repo_sha: resolve_repo_sha(),
            release_id: env_or("ARIFLOW_RELEASE_ID", env!("CARGO_PKG_VERSION")),
            binary_digest_hex: digest,
            digest_algorithm: algo,
            formula_version: FORMULA_VERSION.to_string(),
            policy_version: env_or("ARIFLOW_POLICY_VERSION", POLICY_VERSION_DEFAULT),
            receipt_format_version: RECEIPT_FORMAT_VERSION.to_string(),
            deployed_at: env_or("ARIFLOW_DEPLOYED_AT", &process_started_at().unwrap_or_else(|| "UNKNOWN".to_string())),
            witnessed_by: env_or("ARIFLOW_WITNESSED_BY", "self"),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

fn env_or(key: &str, fallback: &str) -> String {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => fallback.to_string(),
    }
}

/// Resolve the source commit: deploy-time env wins, then the local git HEAD.
fn resolve_repo_sha() -> String {
    if let Ok(s) = std::env::var("ARIFLOW_REPO_SHA") {
        if !s.trim().is_empty() {
            return s.trim().to_string();
        }
    }
    let dir = std::env::var("ARIFLOW_REPO_DIR").unwrap_or_else(|_| "/root/arifFlow".to_string());
    let git = std::path::Path::new(&dir).join(".git");
    let head = match std::fs::read_to_string(git.join("HEAD")) {
        Ok(h) => h,
        Err(_) => return "UNKNOWN".to_string(),
    };
    let head = head.trim();
    if let Some(r) = head.strip_prefix("ref: ") {
        if let Ok(sha) = std::fs::read_to_string(git.join(r)) {
            let s = sha.trim();
            if !s.is_empty() {
                return s.to_string();
            }
        }
        if let Ok(packed) = std::fs::read_to_string(git.join("packed-refs")) {
            for line in packed.lines() {
                if let Some((sha, name)) = line.split_once(' ') {
                    if name.trim() == r {
                        return sha.trim().to_string();
                    }
                }
            }
        }
        "UNKNOWN".to_string()
    } else if head.is_empty() {
        "UNKNOWN".to_string()
    } else {
        head.to_string()
    }
}

/// Digest the running binary. blake3 is a direct dependency; the algorithm is
/// reported alongside the digest so a reader never has to guess.
fn binary_digest() -> (String, String) {
    match std::env::current_exe() {
        Ok(p) => match std::fs::read(&p) {
            Ok(bytes) => (blake3::hash(&bytes).to_hex().to_string(), "blake3".to_string()),
            Err(_) => ("UNKNOWN".to_string(), "blake3".to_string()),
        },
        Err(_) => ("UNKNOWN".to_string(), "blake3".to_string()),
    }
}

/// Process start time from /proc (Linux): now - (uptime - starttime/HZ).
fn process_started_at() -> Option<String> {
    let uptime = std::fs::read_to_string("/proc/uptime").ok()?;
    let up_s: f64 = uptime.split_whitespace().next()?.parse().ok()?;
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let close = stat.rfind(')')?;
    let rest: Vec<&str> = stat[close + 1..].split_whitespace().collect();
    // Field 22 (starttime, in clock ticks) — field 3 is the first after comm.
    let start_ticks: f64 = rest.get(19)?.parse().ok()?;
    const HZ: f64 = 100.0;
    let since_boot = start_ticks / HZ;
    if since_boot > up_s {
        return None;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs_f64();
    let started = now - (up_s - since_boot);
    let secs = started as i64;
    let nanos = ((started - secs as f64) * 1e9) as u32;
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nanos).map(|d| d.to_rfc3339())
}

// ── WS3: explicit daemon-path capability state ─────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityState {
    /// `active` | `compiled_inactive` | `experimental_non_daemon` | `scheduler_path_only`
    pub status: String,
    /// Whether the path is currently enabled in this process.
    pub enabled: bool,
    /// Why it is in that state — read this, not the label.
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Capabilities {
    pub cross_organ_bridge_arifos: CapabilityState,
    pub cross_organ_bridge_aforge: CapabilityState,
    pub merkle_checkpointing: CapabilityState,
    pub bsp_scheduler: CapabilityState,
}

impl Capabilities {
    pub fn current() -> Self {
        Self {
            cross_organ_bridge_arifos: CapabilityState {
                status: "compiled_inactive".to_string(),
                enabled: false,
                reason: "arifOS (:8088) authorization probe is compiled (main.rs health probe) but is \
                         NOT invoked from the daemon ingest/enforce path. Wiring it changes the live \
                         authorization path and therefore requires a gated production window (F13/888). \
                         Reserved flag: ARIFLOW_BRIDGE_ARIFOS."
                    .to_string(),
            },
            cross_organ_bridge_aforge: CapabilityState {
                status: "compiled_inactive".to_string(),
                enabled: false,
                reason: "A-FORGE (:7071) dispatch probe is compiled but NOT invoked from the daemon \
                         ingest/enforce path. Wiring it changes the live execution path and requires a \
                         gated production window (F13/888). Reserved flag: ARIFLOW_BRIDGE_AFORGE."
                    .to_string(),
            },
            merkle_checkpointing: CapabilityState {
                status: "scheduler_path_only".to_string(),
                enabled: false,
                reason: "Merkle roots are computed on the scheduler path only; the daemon receipt \
                         pipeline does not checkpoint per super-step. Reserved flag: \
                         ARIFLOW_MERKLE_DAEMON."
                    .to_string(),
            },
            bsp_scheduler: CapabilityState {
                status: "experimental_non_daemon".to_string(),
                enabled: false,
                reason: "BSP scheduler is invoked from stdin_protocol_loop only — never from the \
                         daemon HTTP path. The governed parallel-execution design is therefore \
                         UNEXERCISED at runtime. Reserved flag: ARIFLOW_BSP_DAEMON."
                    .to_string(),
            },
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// What every FQ-shaped number in `/health` actually measures.
///
/// The federation has at least three FQ-shaped numbers computed over three
/// different populations. Unlabelled, they are read as one thing and produce
/// false conclusions ("healthy" vs "held") depending on which one was quoted.
pub fn metric_semantics() -> serde_json::Value {
    serde_json::json!({
        "fq.quotient": {
            "population": "sliding receipt window (last 100 receipts, all actors)",
            "unit": "ratio",
            "formula": "verify_count / execute_count",
            "direction": "higher_is_healthier",
            "formula_version": FORMULA_VERSION,
        },
        "fq.per_actor[*].quotient": {
            "population": "per-actor enforcement window (ACTOR_FQ_WINDOW of that actor's own steps)",
            "unit": "ratio",
            "formula": "verify_count / execute_count",
            "direction": "higher_is_healthier",
            "note": "P3 2026-11-22: read-only (T0Observe) and session-orphan verifies are excluded \
                     from the numerator; see excluded_verify_count.",
        },
        "vector.dimensions.fq.value": {
            "population": "federation roll-up of the per-actor quotients (minimum gated harmonic mean)",
            "unit": "ratio",
            "direction": "higher_is_healthier",
        },
        "fq.receipts": {
            "population": "in-memory receipt store capacity — NOT the ledger total",
            "unit": "count",
            "note": "The durable ledger is /var/lib/arifflow/receipts.jsonl.",
        },
        "fq.legacy_diagnosis": {
            "population": "verify share of the window",
            "unit": "percent",
            "note": "VERIFICATION DOMINANCE / EXECUTION DOMINANCE / BALANCED — deprecated as a health \
                     indicator; use the top-level verdict.",
        },
        "constitutional_verdict": {
            "owner": "arifOS",
            "note": "arifFlow never emits SEAL/HOLD/SABAR/VOID.",
        },
        "metabolic_gate_state": {
            "owner": "arifFlow",
            "values": ["ALLOW", "FLOW_BLOCKED", "THROTTLED", "COOLDOWN"],
        },
        "observed_diagnosis": {
            "owner": "arifFlow",
            "values": ["FLOWING", "COHERENCE_FRACTURE", "SIMULATION", "PATHOLOGICAL"],
        },
    })
}
