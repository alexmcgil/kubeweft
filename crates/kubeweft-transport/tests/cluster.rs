use std::{
    fs,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use kubeweft_transport::{
    AgentConfig, AgentHandle, ClusterRole, ClusterStore, LocalControlClient, PresenceState,
    TransportError, discover,
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

fn stored_device_id(directory: &Path) -> String {
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("cluster.json")).unwrap()).unwrap();
    state["device_id"].as_str().unwrap().to_owned()
}

fn start_agent(directory: &PathBuf, name: &str) -> AgentHandle {
    let mut config = AgentConfig::new(directory);
    config.listen = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
    config.discovery_port = None;
    config.device_name = Some(name.to_owned());
    config.presence_interval = Duration::from_millis(50);
    AgentHandle::start(config).unwrap()
}

fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if predicate() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(predicate(), "condition did not become true before timeout");
}

#[test]
fn authenticated_presence_tracks_members_without_changing_membership() {
    let desktop_directory = temporary_directory("presence-desktop");
    let laptop_directory = temporary_directory("presence-laptop");
    let desktop = start_agent(&desktop_directory, "desktop");
    let laptop = start_agent(&laptop_directory, "laptop");
    let desktop_client = LocalControlClient::local(&desktop_directory).unwrap();
    let laptop_client = LocalControlClient::local(&laptop_directory).unwrap();
    let (_, invite) = desktop_client.create_cluster("home").unwrap();
    laptop_client
        .join_cluster(desktop.advertised_endpoint(), invite)
        .unwrap();

    wait_until(Duration::from_secs(3), || {
        desktop_client.presence().is_ok_and(|records| {
            records.len() == 2
                && records
                    .iter()
                    .all(|record| record.state == PresenceState::Online)
        })
    });

    laptop.shutdown().unwrap();
    wait_until(Duration::from_secs(3), || {
        desktop_client.presence().is_ok_and(|records| {
            records.iter().any(|record| {
                record.member.device_name == "laptop" && record.state == PresenceState::Offline
            })
        })
    });
    assert_eq!(desktop_client.members().unwrap().1.len(), 2);

    desktop.shutdown().unwrap();
    fs::remove_dir_all(desktop_directory).unwrap();
    fs::remove_dir_all(laptop_directory).unwrap();
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
        "version": 4,
        "request": {"type": "create_cluster", "name": "must-not-exist"}
    }))
    .unwrap();
    let mut stream = std::net::TcpStream::connect(agent.endpoint()).unwrap();
    stream
        .write_all(&u32::try_from(payload.len()).unwrap().to_be_bytes())
        .unwrap();
    stream.write_all(&payload).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut response = [0_u8; 4];
    assert!(stream.read_exact(&mut response).is_err());
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
fn non_loopback_listener_uses_the_secure_peer_transport_without_an_insecure_flag() {
    let directory = temporary_directory("secure-non-loopback");
    let mut config = AgentConfig::new(&directory);
    config.listen = (Ipv4Addr::UNSPECIFIED, 0).into();
    config.discovery_port = None;
    AgentHandle::start(config).unwrap().shutdown().unwrap();
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn device_identity_is_private_and_survives_restart() {
    let directory = temporary_directory("identity-persistence");
    let first = start_agent(&directory, "desktop");
    let first_id = LocalControlClient::local(&directory)
        .unwrap()
        .status()
        .unwrap()
        .device_id;
    assert_eq!(first_id.as_str(), stored_device_id(&directory));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_eq!(
            fs::metadata(directory.join("identity.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    first.shutdown().unwrap();

    let second = start_agent(&directory, "ignored");
    assert_eq!(
        LocalControlClient::local(&directory)
            .unwrap()
            .status()
            .unwrap()
            .device_id,
        first_id
    );
    second.shutdown().unwrap();
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn pairing_code_pins_the_coordinator_identity() {
    let expected_directory = temporary_directory("pin-expected");
    let impostor_directory = temporary_directory("pin-impostor");
    let joining_directory = temporary_directory("pin-joining");
    let expected = start_agent(&expected_directory, "expected");
    let impostor = start_agent(&impostor_directory, "impostor");
    let joining = start_agent(&joining_directory, "joining");
    let expected_client = LocalControlClient::local(&expected_directory).unwrap();
    let impostor_client = LocalControlClient::local(&impostor_directory).unwrap();
    expected_client.create_cluster("expected-cluster").unwrap();
    impostor_client.create_cluster("impostor-cluster").unwrap();
    let pairing_code = expected_client.create_invite().unwrap();

    let mismatch = LocalControlClient::local(&joining_directory)
        .unwrap()
        .join_cluster(impostor.advertised_endpoint(), &pairing_code);
    assert!(
        matches!(&mismatch, Err(TransportError::Remote { code, .. }) if code == "peer_identity_mismatch"),
        "unexpected identity mismatch result: {mismatch:?}"
    );

    LocalControlClient::local(&joining_directory)
        .unwrap()
        .join_cluster(expected.advertised_endpoint(), pairing_code)
        .unwrap();

    expected.shutdown().unwrap();
    impostor.shutdown().unwrap();
    joining.shutdown().unwrap();
    fs::remove_dir_all(expected_directory).unwrap();
    fs::remove_dir_all(impostor_directory).unwrap();
    fs::remove_dir_all(joining_directory).unwrap();
}

#[test]
fn a_membership_credential_cannot_be_replayed_by_another_device_identity() {
    let coordinator_directory = temporary_directory("replay-coordinator");
    let member_directory = temporary_directory("replay-member");
    let attacker_directory = temporary_directory("replay-attacker");
    let coordinator = start_agent(&coordinator_directory, "coordinator");
    let member = start_agent(&member_directory, "member");
    let attacker = start_agent(&attacker_directory, "attacker");
    let coordinator_client = LocalControlClient::local(&coordinator_directory).unwrap();
    let (_, pairing_code) = coordinator_client.create_cluster("home").unwrap();
    LocalControlClient::local(&member_directory)
        .unwrap()
        .join_cluster(coordinator.advertised_endpoint(), pairing_code)
        .unwrap();
    attacker.shutdown().unwrap();

    let member_state: serde_json::Value =
        serde_json::from_slice(&fs::read(member_directory.join("cluster.json")).unwrap()).unwrap();
    let mut attacker_state: serde_json::Value =
        serde_json::from_slice(&fs::read(attacker_directory.join("cluster.json")).unwrap())
            .unwrap();
    attacker_state["cluster"] = member_state["cluster"].clone();
    fs::write(
        attacker_directory.join("cluster.json"),
        serde_json::to_vec_pretty(&attacker_state).unwrap(),
    )
    .unwrap();

    let attacker = start_agent(&attacker_directory, "ignored");
    let replay = LocalControlClient::local(&attacker_directory)
        .unwrap()
        .members();
    assert!(
        matches!(&replay, Err(TransportError::Remote { code, .. }) if code == "unauthorized"),
        "unexpected credential replay result: {replay:?}"
    );

    coordinator.shutdown().unwrap();
    member.shutdown().unwrap();
    attacker.shutdown().unwrap();
    fs::remove_dir_all(coordinator_directory).unwrap();
    fs::remove_dir_all(member_directory).unwrap();
    fs::remove_dir_all(attacker_directory).unwrap();
}

#[test]
fn pairing_and_membership_secrets_are_encrypted_on_the_peer_connection() {
    let coordinator_directory = temporary_directory("encrypted-coordinator");
    let joining_directory = temporary_directory("encrypted-joining");
    let coordinator = start_agent(&coordinator_directory, "coordinator");
    let joining = start_agent(&joining_directory, "joining");
    let coordinator_client = LocalControlClient::local(&coordinator_directory).unwrap();
    let (_, pairing_code) = coordinator_client.create_cluster("home").unwrap();

    let proxy = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let proxy_endpoint = proxy.local_addr().unwrap();
    let coordinator_endpoint = coordinator.advertised_endpoint();
    let recorded = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let proxy_recording = std::sync::Arc::clone(&recorded);
    let proxy_thread = std::thread::spawn(move || {
        let (client, _) = proxy.accept().unwrap();
        let server = std::net::TcpStream::connect(coordinator_endpoint).unwrap();
        let mut client_reader = client.try_clone().unwrap();
        let mut server_writer = server.try_clone().unwrap();
        let server_recording = std::sync::Arc::clone(&proxy_recording);
        let client_to_server = std::thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            loop {
                let count = client_reader.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                proxy_recording
                    .lock()
                    .unwrap()
                    .extend_from_slice(&buffer[..count]);
                server_writer.write_all(&buffer[..count]).unwrap();
            }
        });
        let mut server_reader = server;
        let mut client_writer = client;
        let mut buffer = [0_u8; 4096];
        loop {
            let count = server_reader.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            server_recording
                .lock()
                .unwrap()
                .extend_from_slice(&buffer[..count]);
            client_writer.write_all(&buffer[..count]).unwrap();
        }
        client_to_server.join().unwrap();
    });

    LocalControlClient::local(&joining_directory)
        .unwrap()
        .join_cluster(proxy_endpoint, &pairing_code)
        .unwrap();
    proxy_thread.join().unwrap();
    let credential = stored_member_credential(&coordinator_directory, "joining");
    let bytes = recorded.lock().unwrap();
    assert!(!contains_subslice(&bytes, pairing_code.as_bytes()));
    assert!(!contains_subslice(&bytes, credential.as_bytes()));

    coordinator.shutdown().unwrap();
    joining.shutdown().unwrap();
    fs::remove_dir_all(coordinator_directory).unwrap();
    fs::remove_dir_all(joining_directory).unwrap();
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|candidate| candidate == needle)
}
