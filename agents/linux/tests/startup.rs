use std::process::Command;

#[test]
fn startup_reports_unconfigured_adapters() {
    let output = Command::new(env!("CARGO_BIN_EXE_kubeweft-agent"))
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        b"kubeweft-agent: platform adapters are not configured\n"
    );
}
