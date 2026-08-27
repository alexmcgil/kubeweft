use std::process::Command;

fn kubeweft() -> Command {
    Command::new(env!("CARGO_BIN_EXE_kubeweft"))
}

#[test]
fn version_is_reported() {
    let output = kubeweft().arg("--version").output().unwrap();

    assert!(output.status.success());
    assert_eq!(output.stdout, b"kubeweft 0.1.0\n");
}

#[test]
fn doctor_describes_placeholder_checks() {
    let output = kubeweft().arg("doctor").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(stdout.contains("cluster connectivity"));
    assert!(stdout.contains("platform capabilities"));
    assert!(stdout.contains("placeholder"));
}

#[test]
fn unknown_command_is_rejected() {
    let output = kubeweft().arg("unknown").output().unwrap();

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stderr, b"unknown command: unknown\n");
}

#[test]
fn doctor_rejects_a_trailing_argument() {
    let output = kubeweft().args(["doctor", "unknown"]).output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stderr, b"unknown command: unknown\n");
}

#[test]
fn version_rejects_a_trailing_argument() {
    let output = kubeweft().args(["--version", "unknown"]).output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stderr, b"unknown command: unknown\n");
}
