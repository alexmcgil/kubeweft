use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use kubeweft_filesystem::{
    Availability, ContentStore, ContentStoreRegistry, DurabilityStatus, EntryMetadata,
    FilesystemError, FilesystemEvent, FilesystemService, InMemoryContentStore,
    InMemoryMetadataStore, LocalContentStore, LocalMetadataStore, MetadataStore, NetworkKind,
    PowerSource, RecordingEventSink, ReplicaState, StorageNode,
};
use kubeweft_model::DeviceId;

const CAPACITY: u64 = 1024 * 1024;

fn device(name: &str) -> DeviceId {
    DeviceId::new(format!("device.{name}")).unwrap()
}

fn node(name: &str, power: PowerSource, availability: Availability, online: bool) -> StorageNode {
    StorageNode {
        device_id: device(name),
        online,
        free_space: CAPACITY,
        total_space: CAPACITY,
        power,
        availability,
        network: NetworkKind::Lan,
    }
}

fn add_node(registry: &ContentStoreRegistry, node: StorageNode) -> Arc<InMemoryContentStore> {
    let store = Arc::new(InMemoryContentStore::new(CAPACITY));
    registry.register(node, store.clone()).unwrap();
    store
}

fn filesystem(registry: Arc<ContentStoreRegistry>) -> FilesystemService {
    FilesystemService::new(Arc::new(InMemoryMetadataStore::new()), registry)
}

fn create_parent_tree(fs: &FilesystemService) {
    fs.mkdir("/home").unwrap();
    fs.mkdir("/home/alex").unwrap();
    fs.mkdir("/home/alex/Documents").unwrap();
}

#[test]
fn writes_on_laptop_replicates_to_desktop_and_reads_from_phone() {
    let registry = Arc::new(ContentStoreRegistry::new());
    let desktop_store = add_node(
        &registry,
        node("desktop", PowerSource::Mains, Availability::Stable, true),
    );
    add_node(
        &registry,
        node(
            "laptop",
            PowerSource::Battery,
            Availability::Intermittent,
            true,
        ),
    );
    add_node(
        &registry,
        node(
            "phone",
            PowerSource::Battery,
            Availability::Intermittent,
            true,
        ),
    );
    let fs = filesystem(Arc::clone(&registry));
    create_parent_tree(&fs);
    let initial = fs.create("/home/alex/Documents/hello.txt").unwrap();

    let written = fs
        .write(
            "/home/alex/Documents/hello.txt",
            b"hello kubeweft",
            Some(initial.generation),
            &device("laptop"),
        )
        .unwrap();
    let content_id = written
        .manifest
        .as_ref()
        .unwrap()
        .content_id()
        .unwrap()
        .clone();
    let before = fs
        .placement_for_path("/home/alex/Documents/hello.txt")
        .unwrap()
        .unwrap();
    assert_eq!(before.durability(), DurabilityStatus::Degraded);
    assert_eq!(before.replicas.len(), 1);

    let report = fs.reconciler().reconcile().unwrap();
    assert_eq!(report.replicas_created, 1);
    assert!(desktop_store.exists(&content_id).unwrap());
    let placement = fs
        .placement_for_path("/home/alex/Documents/hello.txt")
        .unwrap()
        .unwrap();
    assert_eq!(placement.durability(), DurabilityStatus::Healthy);
    assert!(
        placement
            .replicas
            .iter()
            .any(|replica| replica.node_id == device("laptop"))
    );
    assert!(
        placement
            .replicas
            .iter()
            .any(|replica| replica.node_id == device("desktop"))
    );

    registry.set_online(&device("laptop"), false).unwrap();
    assert_eq!(
        fs.read_from("/home/alex/Documents/hello.txt", &device("phone"))
            .unwrap(),
        b"hello kubeweft"
    );

    registry.set_online(&device("desktop"), false).unwrap();
    assert!(matches!(
        fs.stat("/home/alex/Documents/hello.txt").unwrap(),
        EntryMetadata::File(_)
    ));
    assert_eq!(
        fs.read("/home/alex/Documents/hello.txt").unwrap_err(),
        FilesystemError::ContentUnavailable
    );

    registry.set_online(&device("desktop"), true).unwrap();
    assert_eq!(
        fs.read("/home/alex/Documents/hello.txt").unwrap(),
        b"hello kubeweft"
    );
}

#[test]
fn stale_generation_cannot_silently_overwrite_a_file() {
    let registry = Arc::new(ContentStoreRegistry::new());
    add_node(
        &registry,
        node(
            "laptop",
            PowerSource::Battery,
            Availability::Intermittent,
            true,
        ),
    );
    let fs = filesystem(registry);
    fs.mkdir("/home").unwrap();
    let initial = fs.create("/home/foo.txt").unwrap();
    let client_a_generation = initial.generation;
    let client_b_generation = initial.generation;

    let committed = fs
        .write(
            "/home/foo.txt",
            b"client A",
            Some(client_a_generation),
            &device("laptop"),
        )
        .unwrap();
    assert_eq!(committed.generation, 2);
    assert_eq!(
        fs.write(
            "/home/foo.txt",
            b"client B",
            Some(client_b_generation),
            &device("laptop"),
        )
        .unwrap_err(),
        FilesystemError::Conflict
    );
    assert_eq!(fs.read("/home/foo.txt").unwrap(), b"client A");
}

#[test]
fn degraded_replica_set_is_repaired_when_a_better_node_appears() {
    let registry = Arc::new(ContentStoreRegistry::new());
    add_node(
        &registry,
        node(
            "laptop",
            PowerSource::Battery,
            Availability::Intermittent,
            true,
        ),
    );
    let events = Arc::new(RecordingEventSink::default());
    let metadata: Arc<dyn MetadataStore> = Arc::new(InMemoryMetadataStore::new());
    let fs = FilesystemService::with_components(
        metadata,
        Arc::clone(&registry),
        Arc::new(kubeweft_filesystem::DefaultPlacementPolicy),
        events.clone(),
        2,
    );
    fs.mkdir("/home").unwrap();
    let initial = fs.create("/home/foo.txt").unwrap();
    fs.write(
        "/home/foo.txt",
        b"available with one copy",
        Some(initial.generation),
        &device("laptop"),
    )
    .unwrap();

    let first = fs.reconciler().reconcile().unwrap();
    assert_eq!(first.replicas_created, 0);
    assert_eq!(first.degraded_contents.len(), 1);

    add_node(
        &registry,
        node("desktop", PowerSource::Mains, Availability::Stable, true),
    );
    let second = fs.reconciler().reconcile().unwrap();
    assert_eq!(second.replicas_created, 1);
    assert_eq!(
        fs.placement_for_path("/home/foo.txt")
            .unwrap()
            .unwrap()
            .durability(),
        DurabilityStatus::Healthy
    );
    assert!(
        events
            .events()
            .iter()
            .any(|event| matches!(event, FilesystemEvent::DurabilityRestored { .. }))
    );
}

#[test]
fn corrupted_replica_is_skipped_and_recorded() {
    let registry = Arc::new(ContentStoreRegistry::new());
    let laptop_store = add_node(
        &registry,
        node(
            "laptop",
            PowerSource::Battery,
            Availability::Intermittent,
            true,
        ),
    );
    add_node(
        &registry,
        node("desktop", PowerSource::Mains, Availability::Stable, true),
    );
    let fs = filesystem(registry);
    fs.mkdir("/home").unwrap();
    let initial = fs.create("/home/foo.txt").unwrap();
    let written = fs
        .write(
            "/home/foo.txt",
            b"healthy bytes",
            Some(initial.generation),
            &device("laptop"),
        )
        .unwrap();
    fs.reconciler().reconcile().unwrap();
    let content_id = written.manifest.unwrap().content_id().unwrap().clone();
    laptop_store
        .overwrite_unchecked(&content_id, b"corrupt bytes".to_vec())
        .unwrap();

    assert_eq!(fs.read("/home/foo.txt").unwrap(), b"healthy bytes");
    let placement = fs.placement_for_path("/home/foo.txt").unwrap().unwrap();
    assert!(placement.replicas.iter().any(|replica| {
        replica.node_id == device("laptop") && replica.state == ReplicaState::Corrupted
    }));
}

#[test]
fn namespace_operations_preserve_identity_across_rename() {
    let fs = filesystem(Arc::new(ContentStoreRegistry::new()));
    fs.mkdir("/home").unwrap();
    fs.mkdir("/home/alex").unwrap();
    let created = fs.create("/home/alex/a.txt").unwrap();

    fs.rename("/home/alex/a.txt", "/home/alex/b.txt").unwrap();
    assert_eq!(
        fs.stat("/home/alex/a.txt").unwrap_err(),
        FilesystemError::NotFound
    );
    let renamed = match fs.stat("/home/alex/b.txt").unwrap() {
        EntryMetadata::File(metadata) => metadata,
        EntryMetadata::Directory(_) => panic!("expected file"),
    };
    assert_eq!(renamed.id, created.id);
    assert_eq!(fs.list("/home/alex").unwrap()[0].name, "b.txt");
    fs.remove("/home/alex/b.txt").unwrap();
    assert!(fs.list("/home/alex").unwrap().is_empty());
}

#[test]
fn local_backends_survive_reopening() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("kubeweft-filesystem-{nonce}"));
    let metadata_path = root.join("metadata.json");
    let blobs_path = root.join("blobs");

    {
        let metadata: Arc<dyn MetadataStore> =
            Arc::new(LocalMetadataStore::open(&metadata_path).unwrap());
        let namespace = kubeweft_filesystem::Namespace::new(metadata);
        namespace.mkdir("/home").unwrap();
        namespace.create("/home/persisted.txt").unwrap();

        let blobs = LocalContentStore::open(&blobs_path, CAPACITY).unwrap();
        let content_id = blobs.put(b"persistent blob").unwrap();
        assert_eq!(blobs.get(&content_id).unwrap(), b"persistent blob");
    }

    let reopened: Arc<dyn MetadataStore> =
        Arc::new(LocalMetadataStore::open(&metadata_path).unwrap());
    assert!(matches!(
        kubeweft_filesystem::Namespace::new(reopened)
            .stat("/home/persisted.txt")
            .unwrap(),
        EntryMetadata::File(_)
    ));
    let blobs = LocalContentStore::open(&blobs_path, CAPACITY).unwrap();
    let content_id = kubeweft_filesystem::ContentId::for_bytes(b"persistent blob");
    assert_eq!(blobs.get(&content_id).unwrap(), b"persistent blob");

    std::fs::remove_dir_all(root).unwrap();
}
