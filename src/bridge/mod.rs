// arifFlow bridge/mod.rs — FFI bridges to arifOS and A-FORGE
//
// arifFlow communicates with arifOS (governance) and A-FORGE (execution)
// via FFI bridges. State crosses planes only through signed envelopes (A2).

pub mod arifos_governance;
pub mod aforge_executor;

pub use arifos_governance::{ArifOSGovernanceBridge, GovernanceRequest, GovernanceResponse, LeaseInfo};
pub use aforge_executor::{AForgeExecutorBridge, ExecutionRequest, ExecutionResponse};
