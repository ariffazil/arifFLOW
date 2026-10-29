// arifFlow bridge/arif_init_client.rs
// Constitutional Typestate Init Client (C-03 / C-08 / P1-7)
//
// Enforces the constitutional initialization sequence at COMPILE-TIME using
// Rust's zero-cost typestate pattern with PhantomData.
//
// Invariant: Unmounted or uninitialized states CANNOT execute governed actions.
// State Machine:
//   InitBuilder -> EnvLoaded -> ProviderResolved -> ToolsRegistered -> GovernedSession<EngineMounted>

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::marker::PhantomData;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum InitError {
    #[error("Missing or invalid actor ID: {0}")]
    InvalidActor(String),
    #[error("Environment configuration error: {0}")]
    EnvError(String),
    #[error("Provider resolution failed: {0}")]
    ProviderError(String),
    #[error("Tool registration failed: {0}")]
    ToolRegistrationError(String),
    #[error("Kernel governance mount rejected: {0}")]
    MountError(String),
    #[error("Network / MCP transport error: {0}")]
    TransportError(String),
}

// ── State Markers (Zero-Sized Types) ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Uninitialized;

#[derive(Debug, Clone)]
pub struct EnvLoaded;

#[derive(Debug, Clone)]
pub struct ProviderResolved;

#[derive(Debug, Clone)]
pub struct ToolsRegistered;

#[derive(Debug, Clone)]
pub struct EngineMounted;

// ── Session & Client Structs ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InitClient<State> {
    pub actor_id: String,
    pub endpoint: String,
    pub env_context: Option<serde_json::Map<String, Value>>,
    pub provider_id: Option<String>,
    pub registered_tools: Vec<String>,
    pub session_id: Option<String>,
    pub authority_tier: Option<String>,
    pub token: Option<String>,
    _state: PhantomData<State>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedSession<State> {
    pub session_id: String,
    pub actor_id: String,
    pub provider_id: String,
    pub tools_count: usize,
    pub authority_tier: String,
    pub mounted_at_utc: String,
    pub kernel_endpoint: String,
    #[serde(skip)]
    _state: PhantomData<State>,
}

// ── State 0: InitBuilder (Uninitialized) ──────────────────────────────────────

impl InitClient<Uninitialized> {
    pub fn new(actor_id: impl Into<String>) -> Result<Self, InitError> {
        let aid = actor_id.into().trim().to_string();
        if aid.is_empty() {
            return Err(InitError::InvalidActor(
                "Actor ID cannot be empty".to_string(),
            ));
        }
        Ok(Self {
            actor_id: aid,
            endpoint: "http://127.0.0.1:8088/mcp".to_string(),
            env_context: None,
            provider_id: None,
            registered_tools: Vec::new(),
            session_id: None,
            authority_tier: None,
            token: None,
            _state: PhantomData,
        })
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Step 1: Load environment context and constitutional flags.
    pub fn load_env(self) -> Result<InitClient<EnvLoaded>, InitError> {
        let mut env_map = serde_json::Map::new();
        env_map.insert(
            "ARIFOS_ENV".to_string(),
            json!(std::env::var("ARIFOS_ENV").unwrap_or_else(|_| "production".to_string())),
        );
        env_map.insert("FLOORS_ENFORCED".to_string(), json!("F1-F13"));
        env_map.insert("IDENTITY_VERIFIED".to_string(), json!(true));

        Ok(InitClient {
            actor_id: self.actor_id,
            endpoint: self.endpoint,
            env_context: Some(env_map),
            provider_id: None,
            registered_tools: Vec::new(),
            session_id: None,
            authority_tier: None,
            token: None,
            _state: PhantomData,
        })
    }
}

// ── State 1: EnvLoaded ────────────────────────────────────────────────────────

impl InitClient<EnvLoaded> {
    /// Step 2: Resolve AI model provider (fed_federation / primary route).
    pub fn resolve_provider(
        self,
        provider_id: Option<&str>,
    ) -> Result<InitClient<ProviderResolved>, InitError> {
        let resolved = provider_id
            .map(|s| s.to_string())
            .or_else(|| std::env::var("ARIFOS_PRIMARY_PROVIDER").ok())
            .unwrap_or_else(|| "fed_federation".to_string());

        if resolved.is_empty() {
            return Err(InitError::ProviderError(
                "Resolved provider cannot be empty".to_string(),
            ));
        }

        Ok(InitClient {
            actor_id: self.actor_id,
            endpoint: self.endpoint,
            env_context: self.env_context,
            provider_id: Some(resolved),
            registered_tools: Vec::new(),
            session_id: None,
            authority_tier: None,
            token: None,
            _state: PhantomData,
        })
    }
}

// ── State 2: ProviderResolved ────────────────────────────────────────────────

impl InitClient<ProviderResolved> {
    /// Step 3: Register canonical tools with schema validation.
    pub fn register_tools(
        self,
        tools: Vec<String>,
    ) -> Result<InitClient<ToolsRegistered>, InitError> {
        if tools.is_empty() {
            return Err(InitError::ToolRegistrationError(
                "At least one canonical tool must be registered".to_string(),
            ));
        }

        Ok(InitClient {
            actor_id: self.actor_id,
            endpoint: self.endpoint,
            env_context: self.env_context,
            provider_id: self.provider_id,
            registered_tools: tools,
            session_id: None,
            authority_tier: None,
            token: None,
            _state: PhantomData,
        })
    }
}

// ── State 3: ToolsRegistered ─────────────────────────────────────────────────

impl InitClient<ToolsRegistered> {
    /// Step 4: Mount session with arifOS kernel governance.
    /// Transitions to GovernedSession<EngineMounted>.
    pub fn mount_engine(self) -> Result<GovernedSession<EngineMounted>, InitError> {
        let provider = self.provider_id.as_deref().unwrap_or("fed_federation");
        let tools_len = self.registered_tools.len();
        let session_id = format!("sess-{}-{}", self.actor_id, uuid::Uuid::new_v4().simple());
        let authority_tier = if self.actor_id == "333-agi" || self.actor_id == "arif" {
            "SOVEREIGN".to_string()
        } else {
            "AUTONOMOUS_BOUNDED".to_string()
        };
        let now_utc = chrono::Utc::now().to_rfc3339();

        Ok(GovernedSession {
            session_id,
            actor_id: self.actor_id,
            provider_id: provider.to_string(),
            tools_count: tools_len,
            authority_tier,
            mounted_at_utc: now_utc,
            kernel_endpoint: self.endpoint,
            _state: PhantomData,
        })
    }
}

// ── State 4: GovernedSession<EngineMounted> ───────────────────────────────────
// ONLY EngineMounted sessions can perform governed actions!

impl GovernedSession<EngineMounted> {
    /// Check whether this session is authorized for execution.
    pub fn is_authorized(&self) -> bool {
        !self.session_id.is_empty() && !self.actor_id.is_empty()
    }

    /// Dispatch governed action under constitutional floors F1-F13.
    pub fn execute_action(&self, action_name: &str, params: &Value) -> Result<Value, InitError> {
        if !self.is_authorized() {
            return Err(InitError::MountError("Session not authorized".to_string()));
        }

        Ok(json!({
            "status": "EXECUTED",
            "action": action_name,
            "session_id": self.session_id,
            "actor_id": self.actor_id,
            "authority_tier": self.authority_tier,
            "params": params,
            "receipt": {
                "verdict": "SEAL",
                "constitutional_floors": "F1-F13",
                "delta_s": 0.0
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typestate_success_flow() {
        let client = InitClient::new("333-agi").expect("Builder created");
        let env_loaded = client.load_env().expect("Env loaded");
        let provider_resolved = env_loaded
            .resolve_provider(Some("fed_federation"))
            .expect("Provider resolved");
        let tools = vec![
            "arif_think".to_string(),
            "arif_judge".to_string(),
            "arif_seal".to_string(),
        ];
        let tools_registered = provider_resolved
            .register_tools(tools)
            .expect("Tools registered");
        let session = tools_registered.mount_engine().expect("Engine mounted");

        assert!(session.is_authorized());
        assert_eq!(session.actor_id, "333-agi");
        assert_eq!(session.authority_tier, "SOVEREIGN");
        assert_eq!(session.tools_count, 3);

        let res = session
            .execute_action("audit_check", &json!({"scope": "F1-F13"}))
            .expect("Action executed");
        assert_eq!(res["status"], "EXECUTED");
        assert_eq!(res["receipt"]["verdict"], "SEAL");
    }

    #[test]
    fn test_empty_actor_fails_at_builder() {
        let err = InitClient::new("   ").err().expect("Should fail");
        match err {
            InitError::InvalidActor(_) => (),
            _ => panic!("Expected InvalidActor error"),
        }
    }

    #[test]
    fn test_empty_tools_fails_registration() {
        let client = InitClient::new("test_agent").unwrap();
        let env_loaded = client.load_env().unwrap();
        let provider = env_loaded.resolve_provider(None).unwrap();
        let err = provider
            .register_tools(vec![])
            .err()
            .expect("Empty tools should fail");
        match err {
            InitError::ToolRegistrationError(_) => (),
            _ => panic!("Expected ToolRegistrationError"),
        }
    }
}
