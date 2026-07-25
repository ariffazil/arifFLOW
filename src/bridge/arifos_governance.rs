// arifFlow bridge/arifos_governance.rs
// PHASE 4 — Real HTTP bridge to arifOS kernel (:8088)
//
// Replaces blake3 stubs with live MCP calls to arifOS:
//   - request_lease()  → arif_init (session bootstrap)
//   - submit_verdict() → arif_judge (mode=intercept)
//   - validate_checkpoint() → arif_judge (mode=validate)
//
// A1: No execution without lease + 888_JUDGE scope
// F13: Real verdict enforcement, not synthetic

use crate::merkle::MerkleRoot;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// arifOS kernel endpoint (configurable via env)
const KERNEL_URL: &str = "http://127.0.0.1:8088/mcp";
/// Request timeout — 10s per call
const REQUEST_TIMEOUT_S: u64 = 10;

// ── Data models ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseInfo {
    pub lease_id: String,
    pub actor_id: String,
    pub constitutional_chain_id: String,
    pub scope: Vec<String>,
    pub expires_at_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceRequest {
    pub request_type: String,
    pub lease_id: Option<String>,
    pub actor_id: Option<String>,
    pub state_hash: Option<MerkleRoot>,
    pub constitutional_chain_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceResponse {
    pub success: bool,
    pub lease_id: Option<String>,
    pub verdict_id: Option<String>,
    pub verdict_class: Option<String>,
    pub constitutional_chain_id: Option<String>,
    pub error: Option<String>,
}

// ── MCP JSON-RPC helper ─────────────────────────────────────────────

/// Call an arifOS MCP tool over HTTP.
/// Returns the `result` portion of the JSON-RPC response.
fn call_arifos_tool(tool: &str, args: &Value) -> Result<Value, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_S))
        .build()
        .map_err(|e| format!("HTTP client build error: {}", e))?;

    let body = json!({
        "jsonrpc": "2.0",
        "id": format!("ariflow_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()),
        "method": "tools/call",
        "params": {
            "name": tool,
            "arguments": args
        }
    });

    let resp = client
        .post(KERNEL_URL)
        .json(&body)
        .send()
        .map_err(|e| format!("HTTP request to arifOS failed: {} — is arifOS running on :8088?", e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("arifOS returned HTTP {}", status));
    }

    let resp_json: Value = resp
        .json()
        .map_err(|e| format!("Failed to parse arifOS response: {}", e))?;

    // Check for JSON-RPC error
    if let Some(err) = resp_json.get("error") {
        return Err(format!("arifOS MCP error: {:?}", err));
    }

    resp_json
        .get("result")
        .ok_or_else(|| "arifOS response missing 'result' field".into())
        .map(|r| r.clone())
}

// ── Bridge implementation ───────────────────────────────────────────

pub struct ArifOSGovernanceBridge;

impl ArifOSGovernanceBridge {
    pub fn new() -> Self {
        Self
    }

    /// Request a lease from arifOS via arif_init.
    /// Sends: arif_init(mode="init", actor_id="...")
    /// Returns: { session_id, session_token, ... }
    pub fn request_lease(&self, actor_id: &str, _context: &str) -> Result<LeaseInfo, String> {
        let result = call_arifos_tool(
            "arif_init",
            &json!({
                "mode": "init",
                "actor_id": actor_id,
                "requested_authority": "LIMITED_MUTATE"
            }),
        )?;

        // Extract lease/session info from arif_init response
        let session_id = result
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let chain_id = result
            .get("constitutional_chain_id")
            .and_then(|v| v.as_str())
            .unwrap_or("no_chain")
            .to_string();

        let authority = result
            .get("authority")
            .and_then(|v| v.get("effective_authority"))
            .and_then(|v| v.as_str())
            .unwrap_or("OBSERVE_ONLY")
            .to_string();

        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Ok(LeaseInfo {
            lease_id: session_id,
            actor_id: actor_id.to_string(),
            constitutional_chain_id: chain_id,
            scope: vec![authority],
            expires_at_ns: now_ns + 3600_000_000_000, // 1 hour default
        })
    }

    /// Submit state root to arifOS 888_JUDGE for verdict.
    /// Sends: arif_judge(mode="intercept", evidence=[{state_root}])
    /// Returns: (verdict_id, verdict_class)
    pub fn submit_verdict(
        &self,
        lease_id: &str,
        state_hash: &MerkleRoot,
    ) -> Result<(String, String), String> {
        let result = call_arifos_tool(
            "arif_judge",
            &json!({
                "mode": "intercept",
                "actor": lease_id,
                "intent": "super_step_verification",
                "requested_capability": "ariflow.super_step",
                "reversibility_level": "R3",
                "blast_radius": "ORGAN",
                "epistemic_state": "OBSERVED",
                "evidence": [{
                    "state_root": format!("{:?}", state_hash),
                    "source": "arifFlow_scheduler"
                }]
            }),
        )?;

        let verdict_id = result
            .get("verdict_id")
            .and_then(|v| v.as_str())
            .or_else(|| result.get("constitutional_chain_id").and_then(|v| v.as_str()))
            .unwrap_or("no_verdict")
            .to_string();

        // Determine verdict class from result
        let verdict_class = if let Some(decision) = result.get("decision").and_then(|v| v.as_str()) {
            match decision {
                "ALLOW" | "ADMIT_MUTATE" | "ADMIT_READ" => "SEAL",
                "ESCALATE" | "CLASSIFICATION_HOLD" => "HOLD",
                "DENY" => "VOID",
                "SIMULATE" => "SABAR",
                _ => "HOLD",
            }
        } else if let Some(hold) = result.get("hold_required").and_then(|v| v.as_bool()) {
            if hold { "HOLD" } else { "SEAL" }
        } else {
            // If arif_judge responds but no clear verdict, default HOLD
            "HOLD"
        }
        .to_string();

        eprintln!(
            "[arifFlow] Verdict from arifOS: {} | class={}",
            verdict_id, verdict_class
        );

        Ok((verdict_id, verdict_class))
    }

    /// Validate a checkpoint against the constitutional chain.
    /// Sends: arif_judge(mode="validate", constitutional_chain_id="...")
    pub fn validate_checkpoint(&self, chain_id: &str, _verdict_id: &str) -> Result<bool, String> {
        let result = call_arifos_tool(
            "arif_judge",
            &json!({
                "mode": "validate",
                "actor": "ariflow_checkpoint",
                "constitutional_chain_id": chain_id,
                "intent": "checkpoint_restore_verification",
            }),
        )?;

        let valid = result
            .get("decision")
            .and_then(|v| v.as_str())
            .map(|d| d == "ALLOW" || d == "ADMIT_READ")
            .unwrap_or(false);

        Ok(valid)
    }
}

// ── FFI export for Python adapter ───────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn ariflow_request_lease(json_request: *const c_char) -> *mut c_char {
    let bridge = ArifOSGovernanceBridge;
    let c_str = unsafe { CStr::from_ptr(json_request) };
    let json_str = c_str.to_str().unwrap_or("{}");
    let req: GovernanceRequest = serde_json::from_str(json_str).unwrap_or(GovernanceRequest {
        request_type: "lease".into(),
        lease_id: None,
        actor_id: None,
        state_hash: None,
        constitutional_chain_id: None,
    });

    let result = bridge.request_lease(req.actor_id.as_deref().unwrap_or("unknown"), "ffi");
    let response = match result {
        Ok(lease) => GovernanceResponse {
            success: true,
            lease_id: Some(lease.lease_id),
            verdict_id: None,
            verdict_class: None,
            constitutional_chain_id: Some(lease.constitutional_chain_id),
            error: None,
        },
        Err(e) => GovernanceResponse {
            success: false,
            lease_id: None,
            verdict_id: None,
            verdict_class: None,
            constitutional_chain_id: None,
            error: Some(e),
        },
    };
    let json_out = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
    CString::new(json_out).unwrap().into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_construction() {
        let bridge = ArifOSGovernanceBridge::new();
        // Just verify it doesn't panic
        assert!(std::mem::size_of_val(&bridge) > 0);
    }

    #[test]
    fn test_request_lease_handles_connection_refused() {
        let bridge = ArifOSGovernanceBridge;
        // arifOS is not running in test environment
        // Should return an Err with connection refused message
        let result = bridge.request_lease("test_actor", "test_context");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Connection refused") || err.contains("failed"));
    }

    #[test]
    fn test_submit_verdict_handles_connection_refused() {
        let bridge = ArifOSGovernanceBridge;
        let hash = MerkleRoot::ZERO;
        let result = bridge.submit_verdict("test_lease", &hash);
        assert!(result.is_err());
    }
}
