use std::process::Command;

#[test]
fn version_is_reported_without_starting_the_daemon() {
    let output = Command::new(env!("CARGO_BIN_EXE_kubeweft-agent"))
        .arg("--version")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(output.stdout, b"kubeweft-agent 0.1.0\n");
}
