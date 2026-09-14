use std::{
    fs,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use kubeweft_filesystem::{
    Availability, ContentStore, ContentStoreRegistry, EntryMetadata, FilesystemError,
    FilesystemService, InMemoryMetadataStore, LocalContentStore, NetworkKind, PowerSource,
    StorageNode,
};
use kubeweft_filesystem_network::{FilesystemPeerContentStore, RemoteContentStore};
use kubeweft_model::DeviceId;
use kubeweft_transport::{
    AgentConfig, AgentHandle, ClusterContentClient, ClusterStore, LocalControlClient,
};

const CAPACITY: u64 = 10 * 1024 * 1024;

fn temporary_directory(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "kubeweft-data-plane-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn device_id(directory: &Path) -> DeviceId {
    ClusterStore::open(directory, None)
        .unwrap()
        .load()
        .unwrap()
        .device_id()
        .clone()
}

fn start_agent(directory: &Path, name: &str, store: Arc<dyn ContentStore>) -> AgentHandle {
    let mut config = AgentConfig::new(directory);
    config.listen = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    config.discovery_port = None;
    config.device_name = Some(name.to_owned());
    config.content_store = Some(Arc::new(FilesystemPeerContentStore::new(store)));
    AgentHandle::start(config).unwrap()
}

fn node(
    device_id: DeviceId,
    power: PowerSource,
    availability: Availability,
    online: bool,
) -> StorageNode {
    StorageNode {
        device_id,
        online,
        free_space: CAPACITY,
        total_space: CAPACITY,
        power,
        availability,
        network: NetworkKind::Lan,
    }
}

#[test]
fn immutable_content_replicates_over_noise_and_remains_readable_from_another_device() {
    let desktop_directory = temporary_directory("desktop");
    let laptop_directory = temporary_directory("laptop");
    let phone_directory = temporary_directory("phone");
    let desktop_id = device_id(&desktop_directory);
    let laptop_id = device_id(&laptop_directory);
    let phone_id = device_id(&phone_directory);

    let desktop_store = Arc::new(
        LocalContentStore::open(desktop_directory.join("content/blobs"), CAPACITY).unwrap(),
    );
    let laptop_store = Arc::new(
        LocalContentStore::open(laptop_directory.join("content/blobs"), CAPACITY).unwrap(),
    );
    let phone_store =
        Arc::new(LocalContentStore::open(phone_directory.join("content/blobs"), CAPACITY).unwrap());
    let desktop = start_agent(&desktop_directory, "desktop", desktop_store.clone());
    let laptop = start_agent(&laptop_directory, "laptop", laptop_store.clone());
    let phone = start_agent(&phone_directory, "phone", phone_store);

    let desktop_control = LocalControlClient::local(&desktop_directory).unwrap();
    let laptop_control = LocalControlClient::local(&laptop_directory).unwrap();
    let phone_control = LocalControlClient::local(&phone_directory).unwrap();
    let (_, laptop_invite) = desktop_control.create_cluster("home").unwrap();
    laptop_control
        .join_cluster(desktop.advertised_endpoint(), laptop_invite)
        .unwrap();
    let phone_invite = desktop_control.create_invite().unwrap();
    phone_control
        .join_cluster(desktop.advertised_endpoint(), phone_invite)
        .unwrap();
    // Pull the latest roster before opening direct member-to-member streams.
    laptop_control.members().unwrap();

    let data_client = ClusterContentClient::local(&laptop_directory).unwrap();
    let expected = kubeweft_filesystem::ContentId::for_bytes(b"expected bytes");
    let rejected = data_client.put(&desktop_id, expected.as_str(), b"different bytes");
    assert!(matches!(
        rejected,
        Err(kubeweft_transport::TransportError::Remote { ref code, .. })
            if code == "content_integrity"
    ));
    assert!(!desktop_store.exists(&expected).unwrap());

    let metadata = Arc::new(InMemoryMetadataStore::new());
    let laptop_registry = Arc::new(ContentStoreRegistry::new());
    laptop_registry
        .register(
            node(
                laptop_id.clone(),
                PowerSource::Battery,
                Availability::Intermittent,
                true,
            ),
            laptop_store,
        )
        .unwrap();
    laptop_registry
        .register(
            node(
                desktop_id.clone(),
                PowerSource::Mains,
                Availability::Stable,
                true,
            ),
            Arc::new(RemoteContentStore::new(
                ClusterContentClient::local(&laptop_directory).unwrap(),
                desktop_id.clone(),
            )),
        )
        .unwrap();
    let laptop_fs = FilesystemService::new(metadata.clone(), laptop_registry);
    laptop_fs.mkdir("/home").unwrap();
    laptop_fs.mkdir("/home/alex").unwrap();
    let initial = laptop_fs.create("/home/alex/hello.bin").unwrap();
    let payload = (0..180_000)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let written = laptop_fs
        .write(
            "/home/alex/hello.bin",
            &payload,
            Some(initial.generation),
            &laptop_id,
        )
        .unwrap();
    let content_id = written.manifest.unwrap().content_id().unwrap().clone();
    assert_eq!(
        laptop_fs.reconciler().reconcile().unwrap().replicas_created,
        1
    );
    assert_eq!(desktop_store.get(&content_id).unwrap(), payload);

    laptop.shutdown().unwrap();
    let phone_registry = Arc::new(ContentStoreRegistry::new());
    phone_registry
        .register(
            node(
                laptop_id,
                PowerSource::Battery,
                Availability::Intermittent,
                false,
            ),
            Arc::new(RemoteContentStore::new(
                ClusterContentClient::local(&phone_directory).unwrap(),
                phone_id.clone(),
            )),
        )
        .unwrap();
    phone_registry
        .register(
            node(
                desktop_id.clone(),
                PowerSource::Mains,
                Availability::Stable,
                true,
            ),
            Arc::new(RemoteContentStore::new(
                ClusterContentClient::local(&phone_directory).unwrap(),
                desktop_id,
            )),
        )
        .unwrap();
    let phone_fs = FilesystemService::new(metadata, phone_registry);
    assert_eq!(
        phone_fs
            .read_from("/home/alex/hello.bin", &phone_id)
            .unwrap(),
        payload
    );

    desktop.shutdown().unwrap();
    assert!(matches!(
        phone_fs.stat("/home/alex/hello.bin").unwrap(),
        EntryMetadata::File(_)
    ));
    assert_eq!(
        phone_fs.read("/home/alex/hello.bin").unwrap_err(),
        FilesystemError::ContentUnavailable
    );

    phone.shutdown().unwrap();
    fs::remove_dir_all(desktop_directory).unwrap();
    fs::remove_dir_all(laptop_directory).unwrap();
    fs::remove_dir_all(phone_directory).unwrap();
}
