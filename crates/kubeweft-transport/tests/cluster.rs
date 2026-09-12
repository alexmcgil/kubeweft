use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use kubeweft_transport::{
    AgentConfig, AgentHandle, ClusterRole, ClusterStore, ControlClient, TransportError, discover,
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

fn start_agent(directory: &PathBuf, name: &str) -> AgentHandle {
    let mut config = AgentConfig::new(directory);
    config.listen = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
    config.discovery_port = None;
    config.device_name = Some(name.to_owned());
    AgentHandle::start(config).unwrap()
}

#[test]
fn agents_create_and_join_an_authenticated_cluster() {
    let coordinator_directory = temporary_directory("coordinator");
    let laptop_directory = temporary_directory("laptop");
    let phone_directory = temporary_directory("phone");
    let coordinator = start_agent(&coordinator_directory, "desktop");
    let laptop = start_agent(&laptop_directory, "laptop");
    let phone = start_agent(&phone_directory, "phone");

    let coordinator_client = ControlClient::local(&coordinator_directory).unwrap();
    let laptop_client = ControlClient::local(&laptop_directory).unwrap();
    let phone_client = ControlClient::local(&phone_directory).unwrap();
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
