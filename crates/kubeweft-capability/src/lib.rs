//! Generic capability descriptions and provider contracts.
//!
//! This crate owns capability-facing abstractions. It must not own platform
//! implementations, device discovery, scheduling, or operating-system actions.

use kubeweft_model::CapabilityId;

/// A capability that a provider may advertise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDescriptor {
    id: CapabilityId,
}

impl CapabilityDescriptor {
    /// Creates a descriptor for the supplied capability identifier.
    pub fn new(id: CapabilityId) -> Self {
        Self { id }
    }

    /// Returns the advertised capability identifier.
    pub fn id(&self) -> &CapabilityId {
        &self.id
    }
}

/// Supplies generic capability descriptions without prescribing an implementation.
pub trait CapabilityProvider {
    /// Returns the capabilities currently advertised by this provider.
    fn capabilities(&self) -> &[CapabilityDescriptor];
}
