// arifFlow bridge/mod.rs — FFI bridges to arifOS and A-FORGE
//
// arifFlow communicates with arifOS (governance) and A-FORGE (execution)
// via FFI bridges. State crosses planes only through signed envelopes (A2).

pub mod aforge_executor;
pub mod arif_init_client;
pub mod arifos_governance;

pub use aforge_executor::{AForgeExecutorBridge, ExecutionRequest, ExecutionResponse};
pub use arif_init_client::{
    EngineMounted, EnvLoaded, GovernedSession, InitClient, InitError, ProviderResolved,
    ToolsRegistered, Uninitialized,
};
pub use arifos_governance::{
    ArifOSGovernanceBridge, GovernanceRequest, GovernanceResponse, LeaseInfo,
};
