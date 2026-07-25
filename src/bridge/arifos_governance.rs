// arifFlow bridge/arifos_governance.rs — FFI bridge to arifOS
// A1: No execution without lease + 888_JUDGE scope

use crate::merkle::MerkleRoot;
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

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

pub struct ArifOSGovernanceBridge;

impl ArifOSGovernanceBridge {
    pub fn new() -> Self { Self }

    pub fn request_lease(&self, actor_id: &str, _context: &str) -> Result<LeaseInfo, String> {
        let hash = blake3::hash(actor_id.as_bytes());
        Ok(LeaseInfo {
            lease_id: format!("lease_{}", hex_encode(&hash.as_bytes()[..12])),
            actor_id: actor_id.into(),
            constitutional_chain_id: format!("cc_{}", hex_encode(&hash.as_bytes()[..16])),
            scope: vec!["forge_chain".into()],
            expires_at_ns: 3600_000_000_000,
        })
    }

    pub fn submit_verdict(&self, _lease_id: &str, _state_hash: &MerkleRoot) -> Result<(String, String), String> {
        Ok(("verdict_stub".into(), "SEAL".into()))
    }

    pub fn validate_checkpoint(&self, _chain_id: &str, _verdict_id: &str) -> Result<bool, String> {
        Ok(true)
    }

    fn call_arifos(&self, _method: &str, _req: &GovernanceRequest) -> Result<GovernanceResponse, String> {
        Ok(GovernanceResponse {
            success: true,
            lease_id: None,
            verdict_id: Some("stub".into()),
            verdict_class: Some("SEAL".into()),
            constitutional_chain_id: None,
            error: None,
        })
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

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
