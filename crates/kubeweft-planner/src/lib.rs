//! Higher-level placement and route-planning boundary definitions.
//!
//! This crate owns declarative planning requests and results. It must not own
//! scheduler execution, networking, transport selection, or platform adapters.

use kubeweft_model::DeviceId;

/// A request to plan a route between two devices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteRequest {
    source_device_id: DeviceId,
    destination_device_id: DeviceId,
}

impl RouteRequest {
    /// Creates a route request between the supplied devices.
    pub fn new(source_device_id: DeviceId, destination_device_id: DeviceId) -> Self {
        Self {
            source_device_id,
            destination_device_id,
        }
    }

    /// Returns the route's source device.
    pub fn source_device_id(&self) -> &DeviceId {
        &self.source_device_id
    }

    /// Returns the route's destination device.
    pub fn destination_device_id(&self) -> &DeviceId {
        &self.destination_device_id
    }
}
