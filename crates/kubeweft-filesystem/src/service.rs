use std::sync::Arc;

use kubeweft_model::DeviceId;

use crate::{
    ContentManifest, ContentPlacement, ContentRetention, ContentStoreRegistry,
    DefaultPlacementPolicy, DurabilityStatus, EntryMetadata, EventSink, FileMetadata, FileVersion,
    FilesystemError, FilesystemEvent, ListedEntry, MetadataStore, Namespace, NamespaceTarget,
    PlacementPolicy, Replica, ReplicaState, event::NoopEventSink, namespace::now_millis,
};

#[derive(Clone)]
pub struct FilesystemService {
    namespace: Namespace,
    registry: Arc<ContentStoreRegistry>,
    policy: Arc<dyn PlacementPolicy>,
    events: Arc<dyn EventSink>,
    desired_replica_count: usize,
}

impl FilesystemService {
    pub fn new(metadata: Arc<dyn MetadataStore>, registry: Arc<ContentStoreRegistry>) -> Self {
        Self::with_components(
            metadata,
            registry,
            Arc::new(DefaultPlacementPolicy),
            Arc::new(NoopEventSink),
            2,
        )
    }

    pub fn with_components(
        metadata: Arc<dyn MetadataStore>,
        registry: Arc<ContentStoreRegistry>,
        policy: Arc<dyn PlacementPolicy>,
        events: Arc<dyn EventSink>,
        desired_replica_count: usize,
    ) -> Self {
        Self {
            namespace: Namespace::new(metadata),
            registry,
            policy,
            events,
            desired_replica_count,
        }
    }

    pub fn namespace(&self) -> &Namespace {
        &self.namespace
    }

    pub fn registry(&self) -> &Arc<ContentStoreRegistry> {
        &self.registry
    }

    pub fn placement_policy(&self) -> &Arc<dyn PlacementPolicy> {
        &self.policy
    }

    pub fn event_sink(&self) -> &Arc<dyn EventSink> {
        &self.events
    }

    pub fn desired_replica_count(&self) -> usize {
        self.desired_replica_count
    }

    pub fn reconciler(&self) -> crate::ReplicationReconciler {
        crate::ReplicationReconciler::new(
            Arc::clone(self.namespace.metadata_store()),
            Arc::clone(&self.registry),
            Arc::clone(&self.policy),
            Arc::clone(&self.events),
        )
    }

    pub fn mkdir(&self, path: &str) -> Result<(), FilesystemError> {
        self.namespace.mkdir(path).map(|_| ())
    }

    pub fn create(&self, path: &str) -> Result<FileMetadata, FilesystemError> {
        let file_id = self.namespace.create(path)?;
        self.events.emit(FilesystemEvent::FileCreated {
            path: path.to_owned(),
            file_id,
        });
        match self.namespace.stat(path)? {
            EntryMetadata::File(metadata) => Ok(metadata),
            EntryMetadata::Directory(_) => Err(FilesystemError::IsDirectory),
        }
    }

    pub fn stat(&self, path: &str) -> Result<EntryMetadata, FilesystemError> {
        self.namespace.stat(path)
    }

    pub fn list(&self, path: &str) -> Result<Vec<ListedEntry>, FilesystemError> {
        self.namespace.list(path)
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<(), FilesystemError> {
        self.namespace.rename(from, to)
    }

    pub fn remove(&self, path: &str) -> Result<(), FilesystemError> {
        self.namespace.remove(path)
    }

    /// Commits an immutable content object and atomically advances file metadata.
    /// A supplied generation protects a client's previously-read version.
    pub fn write(
        &self,
        path: &str,
        data: &[u8],
        expected_generation: Option<FileVersion>,
        source_node: &DeviceId,
    ) -> Result<FileMetadata, FilesystemError> {
        let target = self.namespace.resolve(path)?;
        let file_id = match target {
            NamespaceTarget::File(id) => id,
            NamespaceTarget::Directory(_) => return Err(FilesystemError::IsDirectory),
        };

        let content_id = self.registry.put_on(source_node, data)?;
        let mut snapshot = self.namespace.metadata_store().load()?;
        let previous_content = snapshot
            .files
            .get(&file_id)
            .and_then(|file| file.manifest.as_ref())
            .map(|manifest| {
                manifest
                    .chunks
                    .iter()
                    .map(|chunk| chunk.content_id.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let file = snapshot.files.get_mut(&file_id).ok_or_else(|| {
            FilesystemError::MetadataUnavailable("namespace file has no metadata".into())
        })?;
        if expected_generation.is_some_and(|generation| generation != file.generation) {
            return Err(FilesystemError::Conflict);
        }
        file.generation += 1;
        file.modified_at_millis = now_millis();
        file.size = u64::try_from(data.len()).map_err(|_| FilesystemError::NoSpace)?;
        file.manifest = Some(ContentManifest::single(content_id.clone(), file.size));
        let committed = file.clone();

        let placement = snapshot
            .placements
            .entry(content_id.clone())
            .or_insert_with(|| ContentPlacement {
                content_id: content_id.clone(),
                desired_replica_count: self.desired_replica_count,
                replicas: Vec::new(),
            });
        placement.desired_replica_count = self.desired_replica_count;
        let replica_created = match placement
            .replicas
            .iter_mut()
            .find(|replica| replica.node_id == *source_node)
        {
            Some(replica) => {
                let changed = replica.state != ReplicaState::Healthy;
                replica.state = ReplicaState::Healthy;
                changed
            }
            None => {
                placement.replicas.push(Replica {
                    node_id: source_node.clone(),
                    state: ReplicaState::Healthy,
                });
                true
            }
        };
        let durability = placement.durability();
        snapshot
            .retentions
            .insert(content_id.clone(), ContentRetention::LiveReference);
        for previous in previous_content {
            let still_referenced = snapshot.files.values().any(|file| {
                file.manifest.as_ref().is_some_and(|manifest| {
                    manifest
                        .chunks
                        .iter()
                        .any(|chunk| chunk.content_id == previous)
                })
            });
            if !still_referenced
                && snapshot.retentions.get(&previous) != Some(&ContentRetention::Snapshot)
            {
                snapshot
                    .retentions
                    .insert(previous, ContentRetention::GarbageCandidate);
            }
        }

        // A failed CAS can leave an unreferenced immutable blob; future GC owns it.
        self.namespace.metadata_store().compare_and_swap(snapshot)?;
        self.events.emit(FilesystemEvent::FileVersionCommitted {
            file_id,
            generation: committed.generation,
            content_id: content_id.clone(),
        });
        if replica_created {
            self.events.emit(FilesystemEvent::ReplicaCreated {
                content_id: content_id.clone(),
                node_id: source_node.clone(),
            });
        }
        if durability == DurabilityStatus::Degraded {
            self.events
                .emit(FilesystemEvent::DurabilityDegraded { content_id });
        }
        Ok(committed)
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>, FilesystemError> {
        self.read_internal(path)
    }

    /// Requesting-node context is accepted for future routing and authorization;
    /// logical path resolution and replica choice remain cluster-wide.
    pub fn read_from(
        &self,
        path: &str,
        _requesting_node: &DeviceId,
    ) -> Result<Vec<u8>, FilesystemError> {
        self.read_internal(path)
    }

    pub fn placement_for_path(
        &self,
        path: &str,
    ) -> Result<Option<ContentPlacement>, FilesystemError> {
        let target = self.namespace.resolve(path)?;
        let id = match target {
            NamespaceTarget::File(id) => id,
            NamespaceTarget::Directory(_) => return Err(FilesystemError::IsDirectory),
        };
        let snapshot = self.namespace.metadata_store().load()?;
        let Some(content_id) = snapshot
            .files
            .get(&id)
            .and_then(|file| file.manifest.as_ref())
            .and_then(ContentManifest::content_id)
        else {
            return Ok(None);
        };
        Ok(snapshot.placements.get(content_id).cloned())
    }

    fn read_internal(&self, path: &str) -> Result<Vec<u8>, FilesystemError> {
        let target = self.namespace.resolve(path)?;
        let file_id = match target {
            NamespaceTarget::File(id) => id,
            NamespaceTarget::Directory(_) => return Err(FilesystemError::IsDirectory),
        };
        let snapshot = self.namespace.metadata_store().load()?;
        let file = snapshot.files.get(&file_id).ok_or_else(|| {
            FilesystemError::MetadataUnavailable("namespace file has no metadata".into())
        })?;
        let Some(manifest) = &file.manifest else {
            return Ok(Vec::new());
        };

        let mut result = Vec::with_capacity(usize::try_from(manifest.size).unwrap_or(0));
        for chunk in &manifest.chunks {
            let placement = snapshot
                .placements
                .get(&chunk.content_id)
                .ok_or(FilesystemError::ContentUnavailable)?;
            let mut chunk_data = None;
            for replica in &placement.replicas {
                let Ok(node) = self.registry.node(&replica.node_id) else {
                    self.mark_replica(&chunk.content_id, &replica.node_id, ReplicaState::Missing);
                    continue;
                };
                if !node.online {
                    self.mark_replica(&chunk.content_id, &replica.node_id, ReplicaState::Offline);
                    self.events.emit(FilesystemEvent::ReplicaUnavailable {
                        content_id: chunk.content_id.clone(),
                        node_id: replica.node_id.clone(),
                    });
                    continue;
                }
                let Ok(bytes) = self.registry.get_from(&replica.node_id, &chunk.content_id) else {
                    self.mark_replica(&chunk.content_id, &replica.node_id, ReplicaState::Missing);
                    continue;
                };
                if !chunk.content_id.matches(&bytes) {
                    self.mark_replica(&chunk.content_id, &replica.node_id, ReplicaState::Corrupted);
                    self.events.emit(FilesystemEvent::ReplicaCorrupted {
                        content_id: chunk.content_id.clone(),
                        node_id: replica.node_id.clone(),
                    });
                    continue;
                }
                self.mark_replica(&chunk.content_id, &replica.node_id, ReplicaState::Healthy);
                chunk_data = Some(bytes);
                break;
            }
            result.extend(chunk_data.ok_or(FilesystemError::ContentUnavailable)?);
        }
        Ok(result)
    }

    fn mark_replica(&self, content_id: &crate::ContentId, node_id: &DeviceId, state: ReplicaState) {
        for _ in 0..4 {
            let Ok(mut snapshot) = self.namespace.metadata_store().load() else {
                return;
            };
            let Some(replica) = snapshot
                .placements
                .get_mut(content_id)
                .and_then(|placement| {
                    placement
                        .replicas
                        .iter_mut()
                        .find(|replica| replica.node_id == *node_id)
                })
            else {
                return;
            };
            if replica.state == state {
                return;
            }
            replica.state = state;
            match self.namespace.metadata_store().compare_and_swap(snapshot) {
                Ok(()) => return,
                Err(FilesystemError::Conflict) => continue,
                Err(_) => return,
            }
        }
    }
}
