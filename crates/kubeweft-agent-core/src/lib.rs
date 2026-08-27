//! Platform-independent agent-coordination boundary definitions.
//!
//! This crate owns generic agent descriptors and coordination contracts. It
//! must not own platform integration, process management, or transport code.

use kubeweft_model::DeviceId;

/// A platform-independent description of an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDescriptor {
    device_id: DeviceId,
}

impl AgentDescriptor {
    /// Creates a descriptor for the agent serving a device.
    pub fn new(device_id: DeviceId) -> Self {
        Self { device_id }
    }

    /// Returns the device served by the agent.
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }
}
