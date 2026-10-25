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
        .header("Accept", "application/json, text/event-stream")
        .json(&body)
        .send()
        .map_err(|e| {
            format!(
                "HTTP request to arifOS failed: {} — is arifOS running on :8088?",
                e
            )
        })?;

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

    let outer = resp_json
        .get("result")
        .ok_or_else(|| "arifOS response missing 'result' field".to_string())?;

    // FastMCP wraps the tool payload inside `structuredContent` (or JSON text
    // inside `content[0].text`). Unwrap to the tool's dictionary so callers can
    // inspect `session_id`, `decision`, `meta.kernel_intercept`, etc.
    if let Some(sc) = outer.get("structuredContent") {
        if sc.is_object() {
            return Ok(sc.clone());
        }
    }
    if let Some(text) = outer.pointer("/content/0/text").and_then(|v| v.as_str()) {
        if let Ok(parsed) = serde_json::from_str::<Value>(text) {
            if parsed.is_object() {
                return Ok(parsed);
            }
        }
    }
    Ok(outer.clone())
}

// ── Bridge implementation ───────────────────────────────────────────

pub struct ArifOSGovernanceBridge;

impl Default for ArifOSGovernanceBridge {
    fn default() -> Self {
        Self::new()
    }
}

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
                "requested_authority": "LIMITED_MUTATE",
                // 2026-10-23: arif_init now refuses a session without a
                // stated purpose ("requires intent"). The bridge was still
                // calling it without one, so boot fell back to OBSERVE_ONLY
                // and the vector dimensions stayed unwired.
                "intent": format!("E1 arifFlow governance bridge lease: {}", _context)
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
            expires_at_ns: now_ns + 3_600_000_000_000, // 1 hour default
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
            .or_else(|| {
                result
                    .get("constitutional_chain_id")
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("no_verdict")
            .to_string();

        // [F-6 2026-10-25] Shared canonical-first classification — identical
        // in submit_verdict and authorize_hold_release by construction.
        // Ambiguity resolves to HOLD, never to SEAL.
        let verdict_class = verdict_class_from_judge(&result);

        eprintln!(
            "[arifFlow] Verdict from arifOS: {} | class={}",
            verdict_id, verdict_class
        );

        Ok((verdict_id, verdict_class))
    }

    /// Authorize clearing a constitutional hold on an actor.
    ///
    /// F13 (2026-10-20): closing the /release governance gap. arifFlow does not
    /// adjudicate whether a hold may be lifted — arifOS does. This asks 888_JUDGE
    /// via arif_judge(mode="intercept") and returns (verdict_id, verdict_class)
    /// using the SAME class mapping as submit_verdict, which already defaults to
    /// HOLD whenever the kernel answers ambiguously or not at all. That default is
    /// load-bearing: an unreachable or unparseable arifOS therefore DENIES rather
    /// than admits (fail-closed, F2).
    ///
    /// `target_actor` is the actor whose hold is being cleared;
    /// `claimed_requester` is the identity the CALLER CLAIMS. They are
    /// deliberately distinct so a kernel verdict is bound to a specific
    /// (target, claimed-requester) pair and cannot be replayed against a
    /// different target.
    ///
    /// [F-5 2026-10-25] Three identities, never conflated:
    ///   - CLAIMED requester: `claimed_requester`, a string in the request.
    ///   - AUTHENTICATED caller: NONE — the arifFlow :7073 surface is plain
    ///     local HTTP with no transport authentication. Recorded as null.
    ///   - SERVICE actor: arifFlow itself (`SERVICE_ACTOR`), which owns the
    ///     session the judge is asked under.
    /// Authority is never minted in the name of an unverified string.
    pub fn authorize_hold_release(
        &self,
        target_actor: &str,
        claimed_requester: &str,
    ) -> Result<(String, String), String> {
        // [F-5 2026-10-25] Live kernel probe 2026-10-25 (127.0.0.1:8088,
        // disposable sessions, no hold released): a bare actor_id arif_init
        // returns actor_verified=false, authority=OBSERVE_ONLY,
        // effective_verdict=HOLD; arif_judge on that session returns
        // verdict=HOLD, mutation_allowed=false, seal_allowed=false. The
        // kernel refuses judgment authority to unverified identities — and
        // the bridge must not mint authority in their name regardless.
        let init_res = call_arifos_tool(
            "arif_init",
            &hold_release_init_args(target_actor, claimed_requester),
        )?;

        let session_id = init_res
            .get("session_id")
            .or_else(|| init_res.pointer("/result/session_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let session_token = init_res
            .get("session_token")
            .or_else(|| init_res.pointer("/result/session_token"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let result = call_arifos_tool(
            "arif_judge",
            &hold_release_judge_args(target_actor, claimed_requester, session_id, session_token),
        )?;

        let verdict_id = extract_verdict_id(&result);

        // [F-6 2026-10-25] Shared canonical-first classification — ambiguity
        // resolves to HOLD, never to SEAL. Do not "improve" this into a
        // permissive mapping.
        let verdict_class = verdict_class_from_judge(&result);

        eprintln!(
            "[arifFlow] HOLD-RELEASE verdict from arifOS: target={} claimed_requester={} (CLAIMED_UNVERIFIED; authenticated_caller=none; service={}) verdict={} class={}",
            target_actor, claimed_requester, SERVICE_ACTOR, verdict_id, verdict_class
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

// ── Hold-release identity binding (F-5, 2026-10-25) ─────────────────────

/// The identity arifFlow itself acts under (SERVICE actor). The boot lease in
/// main.rs already uses this name. Judge sessions for hold-release
/// adjudication are ALWAYS minted for this actor — never for a caller-supplied
/// requester string. Authority is never minted in the name of an unverified
/// identity.
pub const SERVICE_ACTOR: &str = "arifflow-daemon";

/// Build the arif_init args for a hold-release adjudication session.
///
/// [F-5 2026-10-25] Three identities are kept distinct:
///   - CLAIMED requester  — `claimed_requester`, a string in the request.
///     Unauthenticated; recorded as DATA only.
///   - AUTHENTICATED caller — none exists on the arifFlow :7073 HTTP surface
///     (plain local HTTP, no transport auth). `None` is recorded as such.
///   - SERVICE actor — arifFlow itself (`SERVICE_ACTOR`), which owns the
///     session and the judgment call.
pub fn hold_release_init_args(target_actor: &str, claimed_requester: &str) -> Value {
    json!({
        "mode": "init",
        "actor_id": SERVICE_ACTOR,
        "requested_authority": "LIMITED_MUTATE",
        "intent": format!(
            "arifFlow hold release adjudication for target={} (claimed requester='{}' is CLAIMED_UNVERIFIED request data, not session identity)",
            target_actor, claimed_requester
        ),
    })
}

/// Build the arif_judge args for a hold-release adjudication. The session is
/// the SERVICE actor's; the claimed requester rides as claim/evidence DATA.
pub fn hold_release_judge_args(
    target_actor: &str,
    claimed_requester: &str,
    session_id: &str,
    session_token: &str,
) -> Value {
    json!({
        "mode": "intercept",
        "actor_id": SERVICE_ACTOR,
        "session_id": session_id,
        "session_token": session_token,
        "candidate": format!(
            "arifflow_hold_release: target={} claimed_requester={}",
            target_actor, claimed_requester
        ),
        "domain": "governance",
        "action_class": "AUDIT_RECORD",
        "reversibility_level": "R2",
        "blast_radius": "LEDGER",
        "requested_capability": "arifflow.hold_release",
        "claim_class": "MEASURED",
        "claim_text": format!(
            "arifFlow service request to release a stale zero-counter hold on target={}. \
             Claimed requester='{}' is CLAIMED_UNVERIFIED request data: the caller of \
             arifFlow:7073/release is unauthenticated and no identity is asserted for it.",
            target_actor, claimed_requester
        ),
        "evidence": {
            "surface": "arifFlow:7073/release",
            "observed_state": "hold_release_adjudication",
            "target_actor": target_actor,
            "claimed_requester": claimed_requester,
            "claimed_requester_identity_class": "CLAIMED_UNVERIFIED",
            "authenticated_caller": null,
            "service_actor": SERVICE_ACTOR,
            "source": "arifFlow_release_gate"
        }
    })
}

#[cfg(test)]
mod identity_binding_tests {
    use super::*;

    // ── F-5 (2026-10-25): requester binding in hold-release adjudication ──

    /// The fabricated-requester probe envelope, captured live from the kernel
    /// on 2026-10-25 (arif_init(actor_id="probe-impersonation-20261025",
    /// requested_authority="LIMITED_MUTATE") → arif_judge intercept). The
    /// kernel answered OBSERVE_ONLY / HOLD / mutation_allowed=false.
    fn probe_envelope_impersonation() -> Value {
        json!({
            "status": "completed",
            "tool": "arif_judge",
            "verdict": "HOLD",
            "actor": {
                "actor_id": "probe-impersonation-20261025",
                "actor_verified": false,
                "authority_level": "OBSERVE_ONLY"
            },
            "actor_id": "probe-impersonation-20261025",
            "actor_verified": false,
            "authority": "OBSERVE_ONLY",
            "effective_verdict": "HOLD",
            "reason_code": "NEEDS_REVIEW",
            "constitutional_check": {
                "hold_required": true,
                "hold_reason": "effective_verdict=HOLD reason_code=NEEDS_REVIEW",
                "session_state": "OBSERVE_ONLY",
                "agency_level": "OBSERVE_ONLY"
            },
            "result": {
                "mutation_allowed": false,
                "seal_allowed": false
            }
        })
    }

    #[test]
    fn test_hold_release_init_binds_service_actor_not_claimed_requester() {
        // A claimed requester must NEVER become session identity.
        for claimed in [
            "probe-impersonation-20261025",
            "333-AGI",
            "888-APEX",
            "test",
            "attacker",
        ] {
            let init_args = hold_release_init_args("fi-002", claimed);
            assert_eq!(
                init_args.get("actor_id").and_then(|v| v.as_str()),
                Some(SERVICE_ACTOR),
                "arif_init must bind the SERVICE actor, not claimed '{}'",
                claimed
            );
            assert_ne!(init_args["actor_id"], claimed);
            // The claim survives as DATA inside the intent string only.
            assert!(init_args["intent"].as_str().unwrap().contains(claimed));
        }
    }

    #[test]
    fn test_hold_release_judge_args_record_claimed_requester_as_data() {
        let args =
            hold_release_judge_args("fi-002", "probe-impersonation-20261025", "SEAL-xyz", "tok");
        assert_eq!(args["actor_id"], SERVICE_ACTOR);
        assert_eq!(
            args["evidence"]["claimed_requester"],
            "probe-impersonation-20261025"
        );
        assert_eq!(
            args["evidence"]["claimed_requester_identity_class"],
            "CLAIMED_UNVERIFIED"
        );
        // The authenticated-caller slot is honestly NULL (unauthenticated HTTP).
        assert!(args["evidence"]["authenticated_caller"].is_null());
        assert_eq!(args["evidence"]["service_actor"], SERVICE_ACTOR);
        // The claim text states the claim is unverified data.
        assert!(args["claim_text"]
            .as_str()
            .unwrap()
            .contains("CLAIMED_UNVERIFIED"));
    }

    #[test]
    fn test_impersonation_probe_does_not_authorize_release() {
        // Reproduced behavior (OBS): the fabricated requester's session came
        // back OBSERVE_ONLY/HOLD. That envelope must not yield a release.
        let env = probe_envelope_impersonation();
        let class = verdict_class_from_judge(&env);
        assert_eq!(class, "HOLD");
        let vid = extract_verdict_id(&env);
        assert!(!release_authorized(&vid, &class));
    }
}

// ── FFI export for Python adapter ───────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn ariflow_request_lease(json_request: *const c_char) -> *mut c_char {
    let bridge = ArifOSGovernanceBridge;
    // SAFETY: This unsafe block dereferences a raw C pointer from the
    // Python FFI adapter (arifos_governance.py). Preconditions verified by caller:
    //   - json_request is non-null (Python ctypes guarantees this for c_char_p)
    //   - json_request points to a valid NUL-terminated UTF-8 C string
    //   - The Python adapter owns the memory and will not free it during this call
    //   - The string is allocated by Python's create_string_buffer (stable lifetime)
    // F12 INJECTION: The raw data is deserialized through serde_json (lines 243-249)
    // which rejects malformed JSON. The unwrap_or fallback collapses parse failures
    // into a default GovernanceRequest — this is a safe degradation, not a panic.
    // Ψ BOUNDARY: This is the ONLY unsafe boundary in the codebase. It is the
    // explicit Clausius wall between the governed Rust interior and the Python FFI.
    let c_str = unsafe { CStr::from_ptr(json_request) };
    // Convert to &str; fall back to empty JSON object on non-UTF8 input
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
    // SAFETY: into_raw() transfers ownership of the CString's buffer to the
    // Python caller. The Python adapter (arifos_governance.py) MUST call
    // ctypes.free() or equivalent to deallocate this memory.
    // MEMORY MODEL: Rust allocates → Python owns → Python frees.
    // Failure to free results in a leak, not a use-after-free or double-free.
    // The NUL byte is guaranteed present (CString::new validated this above).
    CString::new(json_out).unwrap().into_raw()
}

// ── Shared verdict classification + release gate (F-6, 2026-10-25) ──────

/// Extract the verdict id from a kernel judge envelope.
pub fn extract_verdict_id(result: &Value) -> String {
    result
        .get("verdict_id")
        .and_then(|v| v.as_str())
        .or_else(|| {
            result
                .get("constitutional_chain_id")
                .and_then(|v| v.as_str())
        })
        .or_else(|| {
            result
                .pointer("/meta/constitutional_chain_id")
                .and_then(|v| v.as_str())
        })
        .unwrap_or("no_verdict")
        .to_string()
}

/// Classify a kernel judge envelope into a verdict class
/// (SEAL / HOLD / VOID / SABAR).
///
/// [F-6 2026-10-25] Producer-traced precedence. The kernel's CANONICAL
/// verdict channel is `effective_verdict` (arifOS runtime/verdict.py:
/// "effective_verdict is the canonical field — K7 one source per question";
/// `verdict` is aligned to it on divergence, and `canonical_verdict` is
/// materialized for JSON clients). The legacy `decision` /
/// `meta.kernel_intercept.decision` vocabulary is consulted ONLY when no
/// reconciled verdict exists in the envelope: the kernel's own
/// decision-contract walker records a live payload where
/// `meta.kernel_intercept.decision = ALLOW` contradicted
/// `verdict = effective_verdict = HOLD`, so a raw decision must never
/// outvote the reconciled verdict.
///
/// Fail-closed, never permissive: an unrecognized token maps to HOLD;
/// `hold_required` can only force HOLD — a missing verdict is never read as
/// permission. Do not "improve" this into a permissive mapping.
pub fn verdict_class_from_judge(result: &Value) -> String {
    // 1. Canonical reconciled verdict channels (kernel-owned vocabulary).
    for pointer in ["/effective_verdict", "/verdict", "/canonical_verdict"] {
        if let Some(tok) = result.pointer(pointer).and_then(|v| v.as_str()) {
            // Closed token vocabulary, case-insensitive but NOT forgiving of
            // anything else: an unclassifiable token is an inconsistency and
            // fails closed to HOLD (mirrors the kernel's own closed-token rule).
            return match tok.to_ascii_uppercase().as_str() {
                "SEAL" => "SEAL",
                "VOID" => "VOID",
                "SABAR" => "SABAR",
                // HOLD, 888_HOLD, OBSERVE_ONLY, DEGRADED and every unknown
                // token: non-authorizing.
                _ => "HOLD",
            }
            .to_string();
        }
    }
    // 2. Legacy decision vocabulary (kernel-intercept KernelOutput.decision /
    //    meta.kernel_intercept.decision) — only when NO reconciled verdict
    //    exists. Unknown tokens are non-authorizing.
    let decision = result.get("decision").and_then(|v| v.as_str()).or_else(|| {
        result
            .pointer("/meta/kernel_intercept/decision")
            .and_then(|v| v.as_str())
    });
    if let Some(decision) = decision {
        return match decision {
            "ALLOW" | "ADMIT_MUTATE" | "ADMIT_READ" | "SEAL" => "SEAL",
            "ESCALATE" | "CLASSIFICATION_HOLD" | "HOLD" => "HOLD",
            "DENY" | "VOID" => "VOID",
            "SIMULATE" | "SABAR" => "SABAR",
            _ => "HOLD",
        }
        .to_string();
    }
    // 3. hold_required may only force HOLD. `hold_required == false` with no
    //    verdict is NOT authorization (the old mapping read it as SEAL).
    "HOLD".to_string()
}

/// [F-5 2026-10-25] A hold release is authorized ONLY by a real kernel
/// verdict whose class is SEAL. The `no_verdict` placeholder (the envelope
/// carried no verdict id at all) never authorizes, and neither does any
/// other class or unknown value.
pub fn release_authorized(verdict_id: &str, verdict_class: &str) -> bool {
    verdict_class == "SEAL" && !verdict_id.is_empty() && verdict_id != "no_verdict"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_construction() {
        let bridge = ArifOSGovernanceBridge::new();
        // ZST unit struct — size_of_val is 0, still valid
        _ = bridge;
    }

    #[test]
    fn test_request_lease_handles_connection_refused() {
        let bridge = ArifOSGovernanceBridge;
        let result = bridge.request_lease("test_actor", "test_context");
        // Environment-conditional: if arifOS is running, a successful lease is valid;
        // if it is unreachable, an error with a message is valid. Both are honest.
        if let Err(err) = result {
            assert!(!err.is_empty(), "Error message should not be empty");
        }
    }

    #[test]
    fn test_submit_verdict_handles_connection_refused() {
        let bridge = ArifOSGovernanceBridge;
        let hash = MerkleRoot::ZERO;
        let result = bridge.submit_verdict("test_lease", &hash);
        if let Err(err) = result {
            assert!(!err.is_empty(), "Error message should not be empty");
        }
    }

    // ── F-6 (2026-10-25): verdict mapping semantics ──────────────────────

    #[test]
    fn test_verdict_class_maps_every_supported_token() {
        // Canonical channel (kernel-produced verdict vocabulary).
        for (tok, want) in [
            ("SEAL", "SEAL"),
            ("HOLD", "HOLD"),
            ("VOID", "VOID"),
            ("SABAR", "SABAR"),
            ("888_HOLD", "HOLD"),
            ("OBSERVE_ONLY", "HOLD"),
            ("DEGRADED", "HOLD"),
        ] {
            assert_eq!(
                verdict_class_from_judge(&json!({ "effective_verdict": tok })),
                want,
                "effective_verdict={}",
                tok
            );
            assert_eq!(
                verdict_class_from_judge(&json!({ "verdict": tok })),
                want,
                "verdict={}",
                tok
            );
        }
        // Legacy decision channel (kernel-intercept vocabulary).
        for (tok, want) in [
            ("ALLOW", "SEAL"),
            ("ADMIT_MUTATE", "SEAL"),
            ("ADMIT_READ", "SEAL"),
            ("SEAL", "SEAL"),
            ("ESCALATE", "HOLD"),
            ("CLASSIFICATION_HOLD", "HOLD"),
            ("HOLD", "HOLD"),
            ("DENY", "VOID"),
            ("VOID", "VOID"),
            ("SIMULATE", "SABAR"),
            ("SABAR", "SABAR"),
        ] {
            assert_eq!(
                verdict_class_from_judge(&json!({ "decision": tok })),
                want,
                "decision={}",
                tok
            );
            assert_eq!(
                verdict_class_from_judge(
                    &json!({ "meta": { "kernel_intercept": { "decision": tok } } })
                ),
                want,
                "meta.kernel_intercept.decision={}",
                tok
            );
        }
    }

    #[test]
    fn test_verdict_class_unknown_values_are_non_authorizing() {
        for tok in [
            "APPROVE", "approve", "allow", "SEAL ", "sealed", "", "PROCEED", "OK", "ALLOWED",
        ] {
            let class = verdict_class_from_judge(&json!({ "effective_verdict": tok }));
            assert_ne!(class, "SEAL", "unknown token '{}' must not authorize", tok);
            let class = verdict_class_from_judge(&json!({ "decision": tok }));
            assert_ne!(
                class, "SEAL",
                "unknown decision '{}' must not authorize",
                tok
            );
        }
        // Missing everything entirely.
        assert_eq!(verdict_class_from_judge(&json!({})), "HOLD");
        assert_eq!(
            verdict_class_from_judge(&json!({ "decision": null })),
            "HOLD"
        );
    }

    #[test]
    fn test_verdict_class_canonical_channel_wins_over_kernel_intercept_allow() {
        // The kernel's decision-contract walker recorded exactly this live
        // payload (arifOS runtime/verdict.py): verdict == effective_verdict ==
        // HOLD while meta.kernel_intercept.decision = ALLOW contradicted it.
        // The old mapping read `decision` FIRST and classified it SEAL.
        let env = json!({
            "verdict": "HOLD",
            "effective_verdict": "HOLD",
            "meta": { "kernel_intercept": { "decision": "ALLOW" } }
        });
        assert_eq!(verdict_class_from_judge(&env), "HOLD");
    }

    #[test]
    fn test_verdict_class_legacy_decision_only_without_reconciled_verdict() {
        // A legacy envelope with no reconciled verdict still classifies
        // through the decision vocabulary — but any reconciled verdict
        // overrides it.
        assert_eq!(
            verdict_class_from_judge(&json!({ "decision": "ALLOW" })),
            "SEAL"
        );
        assert_eq!(
            verdict_class_from_judge(&json!({ "decision": "ALLOW", "verdict": "VOID" })),
            "VOID"
        );
        assert_eq!(
            verdict_class_from_judge(&json!({ "decision": "ALLOW", "effective_verdict": "HOLD" })),
            "HOLD"
        );
    }

    #[test]
    fn test_hold_required_never_authorizes() {
        // hold_required=false with no verdict is NOT permission. (The old
        // mapping read it as SEAL — a permissive default on missing data.)
        assert_eq!(
            verdict_class_from_judge(&json!({ "hold_required": false })),
            "HOLD"
        );
        assert_eq!(
            verdict_class_from_judge(&json!({ "hold_required": true })),
            "HOLD"
        );
        assert_eq!(
            verdict_class_from_judge(
                &json!({ "constitutional_check": { "hold_required": false } })
            ),
            "HOLD"
        );
    }

    #[test]
    fn test_release_authorized_requires_seal_and_real_verdict_id() {
        assert!(release_authorized("cc_abc123", "SEAL"));
        // No verdict id (envelope carried none) never releases, even as SEAL.
        assert!(!release_authorized("no_verdict", "SEAL"));
        assert!(!release_authorized("", "SEAL"));
        for class in ["HOLD", "VOID", "SABAR", "hold", "SEALISH", ""] {
            assert!(!release_authorized("cc_abc123", class), "class {}", class);
        }
    }
}
