//! Declarative placement and route planning.

use kubeweft_model::DeviceId;

/// A request to plan a route between two devices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteRequest {
    source_device_id: DeviceId,
    destination_device_id: DeviceId,
}

impl RouteRequest {
    pub fn new(source_device_id: DeviceId, destination_device_id: DeviceId) -> Self {
        Self {
            source_device_id,
            destination_device_id,
        }
    }

    pub fn source_device_id(&self) -> &DeviceId {
        &self.source_device_id
    }

    pub fn destination_device_id(&self) -> &DeviceId {
        &self.destination_device_id
    }
}
