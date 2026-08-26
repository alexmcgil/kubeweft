use kubeweft_model::{CapabilityId, DeviceId, InvalidIdentifier, JobId};

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

#[test]
fn rejects_invalid_identifier_characters() {
    for value in ["core.Application", "core.café", "core.application/launch"] {
        assert_eq!(
            CapabilityId::new(value).unwrap_err(),
            InvalidIdentifier::InvalidCharacter,
            "expected {value:?} to be rejected"
        );
    }
}
