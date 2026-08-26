//! Pure job-placement boundary.
//!
//! This crate will own deterministic planning over supplied cluster state. It
//! must not own execution, resource scoring, preemption, I/O, async runtimes,
//! or platform metadata.

use kubeweft_model::{CapabilityId, DeviceId, JobId};

/// The advertised state of one schedulable device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceState {
    id: DeviceId,
    capabilities: Vec<CapabilityId>,
}

impl DeviceState {
    /// Creates state for a device and the capabilities it advertises.
    pub fn new(id: DeviceId, capabilities: Vec<CapabilityId>) -> Self {
        Self { id, capabilities }
    }

    /// Returns the device identifier.
    pub fn id(&self) -> &DeviceId {
        &self.id
    }

    /// Returns the capabilities advertised by the device.
    pub fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }
}

/// The scheduler-visible state of a cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterState {
    devices: Vec<DeviceState>,
}

impl ClusterState {
    /// Creates cluster state from devices in deterministic selection order.
    pub fn new(devices: Vec<DeviceState>) -> Self {
        Self { devices }
    }

    /// Returns devices in deterministic selection order.
    pub fn devices(&self) -> &[DeviceState] {
        &self.devices
    }
}

/// A job that requires a single capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRequest {
    job_id: JobId,
    required_capability: CapabilityId,
}

impl JobRequest {
    /// Creates a request for a job requiring the supplied capability.
    pub fn new(job_id: JobId, required_capability: CapabilityId) -> Self {
        Self {
            job_id,
            required_capability,
        }
    }

    /// Returns the requested job identifier.
    pub fn job_id(&self) -> &JobId {
        &self.job_id
    }

    /// Returns the capability required to run the job.
    pub fn required_capability(&self) -> &CapabilityId {
        &self.required_capability
    }
}

/// One action in a pure job-placement plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanStep {
    /// Starts an allocation of a job on a selected device.
    StartAllocation {
        /// The job to allocate.
        job_id: JobId,
        /// The device selected for the allocation.
        device_id: DeviceId,
    },
}

/// A pure plan containing ordered allocation steps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    steps: Vec<PlanStep>,
}

impl Plan {
    /// Creates a plan containing the supplied ordered steps.
    pub fn new(steps: Vec<PlanStep>) -> Self {
        Self { steps }
    }

    /// Returns the plan's ordered steps.
    pub fn steps(&self) -> &[PlanStep] {
        &self.steps
    }
}

/// Produces deterministic pure plans from cluster state and job requests.
pub struct Scheduler;

impl Scheduler {
    /// Selects the first device advertising the requested capability.
    pub fn plan(cluster: &ClusterState, request: &JobRequest) -> Plan {
        let steps = cluster
            .devices()
            .iter()
            .find(|device| {
                device
                    .capabilities()
                    .contains(request.required_capability())
            })
            .map(|device| PlanStep::StartAllocation {
                job_id: request.job_id().clone(),
                device_id: device.id().clone(),
            })
            .into_iter()
            .collect();

        Plan::new(steps)
    }
}
