use kubeweft_model::{CapabilityId, DeviceId, JobId};
use kubeweft_scheduler::{ClusterState, DeviceState, JobRequest, PlanStep, Scheduler};

#[test]
fn allocates_a_job_to_the_first_device_advertising_its_capability() {
    let cluster = ClusterState::new(vec![DeviceState::new(
        DeviceId::new("device.workstation").unwrap(),
        vec![CapabilityId::new("core.compute.cpu").unwrap()],
    )]);
    let request = JobRequest::new(
        JobId::new("job.example").unwrap(),
        CapabilityId::new("core.compute.cpu").unwrap(),
    );

    let plan = Scheduler::plan(&cluster, &request);

    assert_eq!(
        plan.steps(),
        &[PlanStep::StartAllocation {
            job_id: JobId::new("job.example").unwrap(),
            device_id: DeviceId::new("device.workstation").unwrap(),
        }]
    );
}

#[test]
fn returns_an_empty_plan_when_no_device_advertises_the_requested_capability() {
    let cluster = ClusterState::new(vec![DeviceState::new(
        DeviceId::new("device.workstation").unwrap(),
        vec![CapabilityId::new("core.compute.gpu").unwrap()],
    )]);
    let request = JobRequest::new(
        JobId::new("job.example").unwrap(),
        CapabilityId::new("core.compute.cpu").unwrap(),
    );

    let plan = Scheduler::plan(&cluster, &request);

    assert!(plan.steps().is_empty());
}
