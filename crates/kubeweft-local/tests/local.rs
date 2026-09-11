use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use kubeweft_local::LocalFilesystem;

fn temporary_directory() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "kubeweft-local-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn local_filesystem_persists_content_between_clients() {
    let directory = temporary_directory();
    let first = LocalFilesystem::open(&directory).unwrap();
    let second = LocalFilesystem::open(&directory).unwrap();
    first.bootstrap_home("alex").unwrap();
    second.service().create("/home/alex/hello.txt").unwrap();
    first
        .service()
        .write(
            "/home/alex/hello.txt",
            b"hello kubeweft",
            Some(1),
            first.device_id(),
        )
        .unwrap();

    let reopened = LocalFilesystem::open(&directory).unwrap();
    assert_eq!(
        reopened.service().read("/home/alex/hello.txt").unwrap(),
        b"hello kubeweft"
    );
    fs::remove_dir_all(directory).unwrap();
}
