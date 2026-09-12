use std::{
    fs,
    io::Write,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use kubeweft_transport::{AgentConfig, AgentHandle};

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
fn doctor_reports_available_local_components() {
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

#[test]
fn cluster_commands_create_join_and_list_members() {
    let desktop_directory = temporary_directory();
    let laptop_directory = temporary_directory();
    let start_agent = |directory: &PathBuf, name: &str| {
        let mut config = AgentConfig::new(directory);
        config.listen = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        config.discovery_port = None;
        config.device_name = Some(name.to_owned());
        AgentHandle::start(config).unwrap()
    };
    let desktop = start_agent(&desktop_directory, "desktop");
    let laptop = start_agent(&laptop_directory, "laptop");

    let create = kubeweft()
        .args([
            "--data-dir",
            desktop_directory.to_str().unwrap(),
            "cluster",
            "create",
            "home",
        ])
        .output()
        .unwrap();
    assert!(create.status.success());
    let create = String::from_utf8(create.stdout).unwrap();
    let lines = create.lines().collect::<Vec<_>>();
    let coordinator = lines[1].split('\t').nth(1).unwrap();
    let invite = lines[2].split('\t').nth(1).unwrap();

    let join = kubeweft()
        .args([
            "--data-dir",
            laptop_directory.to_str().unwrap(),
            "cluster",
            "join",
            coordinator,
            invite,
        ])
        .output()
        .unwrap();
    assert!(
        join.status.success(),
        "{}",
        String::from_utf8_lossy(&join.stderr)
    );

    let members = kubeweft()
        .args([
            "--data-dir",
            laptop_directory.to_str().unwrap(),
            "cluster",
            "members",
        ])
        .output()
        .unwrap();
    assert!(members.status.success());
    let members = String::from_utf8(members.stdout).unwrap();
    assert!(members.contains("\tdesktop\t"));
    assert!(members.contains("\tlaptop\t"));

    desktop.shutdown().unwrap();
    laptop.shutdown().unwrap();
    fs::remove_dir_all(desktop_directory).unwrap();
    fs::remove_dir_all(laptop_directory).unwrap();
}
