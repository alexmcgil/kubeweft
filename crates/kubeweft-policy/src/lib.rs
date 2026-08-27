//! Capability authorization and placement-policy boundary definitions.
//!
//! This crate owns policy decisions, not a policy language, credential store,
//! scheduler implementation, or platform authorization mechanism.

/// The outcome of a capability authorization decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationDecision {
    /// The requested action is authorized.
    Allow,
    /// The requested action is not authorized.
    Deny,
}
