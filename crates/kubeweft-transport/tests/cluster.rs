use std::{
    fs,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use kubeweft_transport::{
    AgentConfig, AgentHandle, ClusterRole, ClusterStore, LocalControlClient, TransportError,
    discover,
};

fn temporary_directory(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "kubeweft-transport-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn stored_member_credential(directory: &Path, device_name: &str) -> String {
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("cluster.json")).unwrap()).unwrap();
    state["cluster"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|member| member["member"]["device_name"] == device_name)
        .unwrap()["credential"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn stored_own_credential(directory: &Path) -> String {
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("cluster.json")).unwrap()).unwrap();
    state["cluster"]["credential"].as_str().unwrap().to_owned()
}

fn start_agent(directory: &PathBuf, name: &str) -> AgentHandle {
    let mut config = AgentConfig::new(directory);
    config.listen = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
    config.discovery_port = None;
    config.device_name = Some(name.to_owned());
    AgentHandle::start(config).unwrap()
}

#[test]
fn agents_create_join_and_one_use_invite_rejects_a_different_transaction() {
    let coordinator_directory = temporary_directory("coordinator");
    let laptop_directory = temporary_directory("laptop");
    let phone_directory = temporary_directory("phone");
    let coordinator = start_agent(&coordinator_directory, "desktop");
    let laptop = start_agent(&laptop_directory, "laptop");
    let phone = start_agent(&phone_directory, "phone");

    let coordinator_client = LocalControlClient::local(&coordinator_directory).unwrap();
    let laptop_client = LocalControlClient::local(&laptop_directory).unwrap();
    let phone_client = LocalControlClient::local(&phone_directory).unwrap();
    let (cluster, invite) = coordinator_client.create_cluster("home").unwrap();

    let joined = laptop_client
        .join_cluster(coordinator.advertised_endpoint(), &invite)
        .unwrap();
    assert_eq!(joined, cluster);
    assert_eq!(
        laptop_client.status().unwrap().role,
        Some(ClusterRole::Member)
    );

    let stale_invite = phone_client.join_cluster(coordinator.advertised_endpoint(), invite);
    assert!(
        matches!(
            &stale_invite,
            Err(TransportError::Remote { code, .. }) if code == "invalid_invite"
        ),
        "unexpected stale invite result: {stale_invite:?}"
    );
    let next_invite = coordinator_client.create_invite().unwrap();
    phone_client
        .join_cluster(coordinator.advertised_endpoint(), next_invite)
        .unwrap();

    for client in [&coordinator_client, &laptop_client, &phone_client] {
        let (listed_cluster, members) = client.members().unwrap();
        assert_eq!(listed_cluster, cluster);
        assert_eq!(
            members
                .iter()
                .map(|member| member.device_name.as_str())
                .collect::<Vec<_>>(),
            ["desktop", "laptop", "phone"]
        );
    }

    coordinator.shutdown().unwrap();
    laptop.shutdown().unwrap();
    phone.shutdown().unwrap();

    let persisted = ClusterStore::open(&laptop_directory, None)
        .unwrap()
        .load()
        .unwrap()
        .status();
    assert_eq!(persisted.cluster, Some(cluster));
    assert_eq!(persisted.role, Some(ClusterRole::Member));

    fs::remove_dir_all(coordinator_directory).unwrap();
    fs::remove_dir_all(laptop_directory).unwrap();
    fs::remove_dir_all(phone_directory).unwrap();
}

#[test]
fn lan_discovery_reports_a_running_agent() {
    let desktop_directory = temporary_directory("discovery-desktop");
    let laptop_directory = temporary_directory("discovery-laptop");
    let probe = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let discovery_port = probe.local_addr().unwrap().port();
    drop(probe);

    let start_discoverable = |directory: &PathBuf, name: &str| {
        let mut config = AgentConfig::new(directory);
        config.listen = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        config.discovery_port = Some(discovery_port);
        config.device_name = Some(name.to_owned());
        AgentHandle::start(config).unwrap()
    };
    let desktop = start_discoverable(&desktop_directory, "discoverable-desktop");
    let laptop = start_discoverable(&laptop_directory, "discoverable-laptop");

    let found = discover(discovery_port, Duration::from_millis(900)).unwrap();
    let mut names = found
        .iter()
        .map(|item| item.device_name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, ["discoverable-desktop", "discoverable-laptop"]);
    assert!(found.iter().any(|item| {
        item.device_name == "discoverable-desktop" && item.endpoint == desktop.advertised_endpoint()
    }));
    assert!(found.iter().any(|item| {
        item.device_name == "discoverable-laptop" && item.endpoint == laptop.advertised_endpoint()
    }));

    desktop.shutdown().unwrap();
    laptop.shutdown().unwrap();
    fs::remove_dir_all(desktop_directory).unwrap();
    fs::remove_dir_all(laptop_directory).unwrap();
}

#[test]
fn duplicate_agent_data_directory_is_rejected_until_shutdown() {
    let directory = temporary_directory("exclusive-directory");
    let first = start_agent(&directory, "first");
    let mut second_config = AgentConfig::new(&directory);
    second_config.listen = (Ipv4Addr::LOCALHOST, 0).into();
    second_config.discovery_port = None;
    assert!(matches!(
        AgentHandle::start(second_config),
        Err(TransportError::AgentAlreadyRunning)
    ));

    first.shutdown().unwrap();
    let replacement = start_agent(&directory, "replacement");
    replacement.shutdown().unwrap();
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn accepted_join_returns_the_same_credential_when_the_client_retries() {
    let coordinator_directory = temporary_directory("retry-coordinator");
    let laptop_directory = temporary_directory("retry-laptop");
    let mut coordinator_config = AgentConfig::new(&coordinator_directory);
    coordinator_config.listen = (Ipv4Addr::LOCALHOST, 0).into();
    coordinator_config.discovery_port = None;
    coordinator_config.device_name = Some("desktop".into());
    coordinator_config.drop_remote_join_responses = 1;
    let coordinator = AgentHandle::start(coordinator_config).unwrap();
    let laptop = start_agent(&laptop_directory, "laptop");
    let coordinator_client = LocalControlClient::local(&coordinator_directory).unwrap();
    let laptop_client = LocalControlClient::local(&laptop_directory).unwrap();
    let (_, invite) = coordinator_client.create_cluster("home").unwrap();

    assert!(
        laptop_client
            .join_cluster(coordinator.advertised_endpoint(), &invite)
            .is_err()
    );
    let accepted_credential = stored_member_credential(&coordinator_directory, "laptop");
    laptop_client
        .join_cluster(coordinator.advertised_endpoint(), invite)
        .unwrap();
    assert_eq!(
        stored_own_credential(&laptop_directory),
        accepted_credential
    );
    assert_eq!(coordinator_client.members().unwrap().1.len(), 2);

    coordinator.shutdown().unwrap();
    laptop.shutdown().unwrap();
    fs::remove_dir_all(coordinator_directory).unwrap();
    fs::remove_dir_all(laptop_directory).unwrap();
}

#[test]
fn pending_join_survives_agent_restart_after_an_accepted_request() {
    let coordinator_directory = temporary_directory("restart-coordinator");
    let laptop_directory = temporary_directory("restart-laptop");
    let mut coordinator_config = AgentConfig::new(&coordinator_directory);
    coordinator_config.listen = (Ipv4Addr::LOCALHOST, 0).into();
    coordinator_config.discovery_port = None;
    coordinator_config.device_name = Some("desktop".into());
    let coordinator = AgentHandle::start(coordinator_config).unwrap();
    let mut laptop_config = AgentConfig::new(&laptop_directory);
    laptop_config.listen = (Ipv4Addr::LOCALHOST, 0).into();
    laptop_config.discovery_port = None;
    laptop_config.device_name = Some("laptop".into());
    laptop_config.fail_join_after_remote_accepts = 1;
    let laptop = AgentHandle::start(laptop_config).unwrap();
    let coordinator_client = LocalControlClient::local(&coordinator_directory).unwrap();
    let (_, invite) = coordinator_client.create_cluster("home").unwrap();
    assert!(
        LocalControlClient::local(&laptop_directory)
            .unwrap()
            .join_cluster(coordinator.advertised_endpoint(), &invite)
            .is_err()
    );
    let accepted_credential = stored_member_credential(&coordinator_directory, "laptop");
    laptop.shutdown().unwrap();

    let laptop = start_agent(&laptop_directory, "ignored-after-first-start");
    LocalControlClient::local(&laptop_directory)
        .unwrap()
        .join_cluster(coordinator.advertised_endpoint(), invite)
        .unwrap();
    assert_eq!(
        stored_own_credential(&laptop_directory),
        accepted_credential
    );

    coordinator.shutdown().unwrap();
    laptop.shutdown().unwrap();
    fs::remove_dir_all(coordinator_directory).unwrap();
    fs::remove_dir_all(laptop_directory).unwrap();
}

#[test]
fn loopback_tcp_cannot_invoke_local_privileged_control() {
    let directory = temporary_directory("local-boundary");
    let agent = start_agent(&directory, "desktop");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_eq!(
            fs::metadata(directory.join("agent.sock"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let payload = serde_json::to_vec(&serde_json::json!({
        "version": 2,
        "request": {"type": "create_cluster", "name": "must-not-exist"}
    }))
    .unwrap();
    let mut stream = std::net::TcpStream::connect(agent.endpoint()).unwrap();
    stream
        .write_all(&u32::try_from(payload.len()).unwrap().to_be_bytes())
        .unwrap();
    stream.write_all(&payload).unwrap();
    let mut length = [0; 4];
    stream.read_exact(&mut length).unwrap();
    let mut response = vec![0; u32::from_be_bytes(length) as usize];
    stream.read_exact(&mut response).unwrap();
    let response: serde_json::Value = serde_json::from_slice(&response).unwrap();
    assert_eq!(response["response"]["type"], "error");
    assert!(
        LocalControlClient::local(&directory)
            .unwrap()
            .status()
            .unwrap()
            .cluster
            .is_none()
    );

    agent.shutdown().unwrap();
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn non_loopback_plaintext_listener_requires_explicit_opt_in() {
    let directory = temporary_directory("plaintext-opt-in");
    let mut config = AgentConfig::new(&directory);
    config.listen = (Ipv4Addr::UNSPECIFIED, 0).into();
    config.discovery_port = None;
    assert!(matches!(
        AgentHandle::start(config),
        Err(TransportError::InsecurePlaintextLanOptInRequired)
    ));
    let mut explicitly_allowed = AgentConfig::new(&directory);
    explicitly_allowed.listen = (Ipv4Addr::UNSPECIFIED, 0).into();
    explicitly_allowed.discovery_port = None;
    explicitly_allowed.insecure_plaintext_lan = true;
    AgentHandle::start(explicitly_allowed)
        .unwrap()
        .shutdown()
        .unwrap();
    fs::remove_dir_all(directory).unwrap();
}
