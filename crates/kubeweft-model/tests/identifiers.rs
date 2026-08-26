use kubeweft_model::{CapabilityId, DeviceId, JobId};

#[test]
fn accepts_namespaced_capability_identifier() {
    let id = CapabilityId::new("core.application.launch").unwrap();
    assert_eq!(id.as_str(), "core.application.launch");
    assert_eq!(id.to_string(), "core.application.launch");
}

#[test]
fn rejects_identifier_without_namespace() {
    let error = CapabilityId::new("launch").unwrap_err();
    assert_eq!(
        error.to_string(),
        "identifier must contain at least one dot-separated namespace"
    );
}

#[test]
fn rejects_empty_identifier_segment() {
    assert!(DeviceId::new("device..phone").is_err());
    assert!(JobId::new("jobs.").is_err());
}
