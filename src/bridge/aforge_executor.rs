// arifFlow bridge/aforge_executor.rs
// FFI Bridge to A-FORGE — ACT 7-phase executor invocation
//
// arifFlow schedules nodes. A-FORGE executes them. No business logic
// in arifFlow — it only schedules and records.

use serde::{Deserialize, Serialize};
use std::ffi::c_char;

/// Execution request sent to A-FORGE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub node_id: String,
    pub topology: String,
    pub envelope_json: String,
    pub lease_id: String,
    pub actor_id: String,
}

/// Response from A-FORGE after node execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResponse {
    pub success: bool,
    pub result_hash: [u8; 32],
    pub receipt: String,
    pub error: Option<String>,
}

/// Bridge to A-FORGE execution subsystem
pub struct AForgeExecutorBridge {
    /// Function pointer for FFI call to A-FORGE
    execute_fn: Option<extern "C" fn(*const c_char) -> *mut c_char>,
}

impl AForgeExecutorBridge {
    pub fn new() -> Self {
        Self { execute_fn: None }
    }

    /// Register the FFI function pointer (called from Python adapter)
    pub fn register(&mut self, execute: extern "C" fn(*const c_char) -> *mut c_char) {
        self.execute_fn = Some(execute);
    }

    /// Schedule a node for execution via A-FORGE
    pub fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResponse, String> {
        let json = serde_json::to_string(&request)
            .map_err(|e| format!("Serialization error: {}", e))?;

        // In production: call registered FFI function
        // For now, return stub for testing
        Ok(ExecutionResponse {
            success: true,
            result_hash: *blake3::hash(json.as_bytes()).as_bytes(),
            receipt: format!("receipt_{}", &request.node_id[..8.min(request.node_id.len())]),
            error: None,
        })
    }

    pub fn is_registered(&self) -> bool {
        self.execute_fn.is_some()
    }
}
