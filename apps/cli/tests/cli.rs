use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

fn kubeweft() -> Command {
    Command::new(env!("CARGO_BIN_EXE_kubeweft"))
}

fn temporary_directory() -> PathBuf {
    std::env::temp_dir().join(format!(
        "kubeweft-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
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
    assert!(stdout.contains("local filesystem: ready"));
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
    assert_eq!(output.stderr, b"unknown command: doctor\n");
}

#[test]
fn version_rejects_a_trailing_argument() {
    let output = kubeweft().args(["--version", "unknown"]).output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stderr, b"unknown command: --version\n");
}

#[test]
fn filesystem_commands_share_persistent_state() {
    let directory = temporary_directory();
    let data_directory = directory.to_str().unwrap();

    let init = kubeweft()
        .args(["--data-dir", data_directory, "fs", "init", "alex"])
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let create = kubeweft()
        .args([
            "--data-dir",
            data_directory,
            "fs",
            "create",
            "/home/alex/hello.txt",
        ])
        .output()
        .unwrap();
    assert!(create.status.success());
    assert_eq!(create.stdout, b"1\n");

    let mut child = kubeweft()
        .args([
            "--data-dir",
            data_directory,
            "fs",
            "write",
            "/home/alex/hello.txt",
            "--expect",
            "1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"hello kubeweft")
        .unwrap();
    let write = child.wait_with_output().unwrap();
    assert!(write.status.success());
    assert_eq!(write.stdout, b"2\n");

    let read = kubeweft()
        .args([
            "--data-dir",
            data_directory,
            "fs",
            "cat",
            "/home/alex/hello.txt",
        ])
        .output()
        .unwrap();
    assert!(read.status.success());
    assert_eq!(read.stdout, b"hello kubeweft");

    let stale = kubeweft()
        .args([
            "--data-dir",
            data_directory,
            "fs",
            "write",
            "/home/alex/hello.txt",
            "--expect",
            "1",
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(stale.status.code(), Some(1));
    assert_eq!(
        stale.stderr,
        b"filesystem error: file generation conflict\n"
    );

    fs::remove_dir_all(directory).unwrap();
}
