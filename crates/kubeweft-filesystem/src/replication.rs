use std::sync::Arc;

use kubeweft_model::DeviceId;

use crate::{
    ContentId, ContentPlacement, ContentStoreRegistry, DurabilityStatus, EventSink,
    FilesystemError, FilesystemEvent, MetadataStore, PlacementPolicy, PlacementRequest, Replica,
    ReplicaState,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    pub replicas_created: usize,
    pub degraded_contents: Vec<ContentId>,
}

pub struct ReplicationReconciler {
    metadata: Arc<dyn MetadataStore>,
    registry: Arc<ContentStoreRegistry>,
    policy: Arc<dyn PlacementPolicy>,
    events: Arc<dyn EventSink>,
}

impl ReplicationReconciler {
    pub fn new(
        metadata: Arc<dyn MetadataStore>,
        registry: Arc<ContentStoreRegistry>,
        policy: Arc<dyn PlacementPolicy>,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            metadata,
            registry,
            policy,
            events,
        }
    }

    /// Performs one deterministic desired-vs-actual reconciliation pass.
    pub fn reconcile(&self) -> Result<ReconcileReport, FilesystemError> {
        let content_ids = self
            .metadata
            .load()?
            .placements
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        let mut report = ReconcileReport::default();
        for content_id in content_ids {
            report.replicas_created += self.reconcile_content(&content_id)?;
            let snapshot = self.metadata.load()?;
            if snapshot
                .placements
                .get(&content_id)
                .is_some_and(|placement| placement.durability() == DurabilityStatus::Degraded)
            {
                report.degraded_contents.push(content_id);
            }
        }
        Ok(report)
    }

    fn reconcile_content(&self, content_id: &ContentId) -> Result<usize, FilesystemError> {
        let original = self
            .metadata
            .load()?
            .placements
            .get(content_id)
            .cloned()
            .ok_or(FilesystemError::ContentUnavailable)?;
        let before = original.durability();
        let refreshed = self.refresh_replicas(&original)?;
        self.store_placement(refreshed.clone())?;

        let nodes = self.registry.nodes()?;
        let desired_nodes = self.policy.desired_nodes(PlacementRequest {
            size: self.content_size(content_id),
            available_nodes: &nodes,
            existing_replicas: &refreshed.replicas,
            desired_replica_count: refreshed.desired_replica_count,
        });

        let mut created = 0;
        for destination in desired_nodes {
            let current = self
                .metadata
                .load()?
                .placements
                .get(content_id)
                .cloned()
                .ok_or(FilesystemError::ContentUnavailable)?;
            if current.replicas.iter().any(|replica| {
                replica.node_id == destination
                    && matches!(replica.state, ReplicaState::Healthy | ReplicaState::Offline)
            }) {
                continue;
            }
            let Some((source, bytes)) = self.find_source(content_id, &current)? else {
                break;
            };
            self.events.emit(FilesystemEvent::ReplicationStarted {
                content_id: content_id.clone(),
                source: source.clone(),
                destination: destination.clone(),
            });

            let destination_store = self.registry.store(&destination)?;
            if destination_store.exists(content_id)? {
                destination_store.delete(content_id)?;
            }
            let written_id = self.registry.put_on(&destination, &bytes)?;
            if written_id != *content_id {
                return Err(FilesystemError::Storage(
                    "content store returned an unexpected content id".into(),
                ));
            }
            self.upsert_replica(content_id, &destination, ReplicaState::Healthy)?;
            created += 1;
            self.events.emit(FilesystemEvent::ReplicaCreated {
                content_id: content_id.clone(),
                node_id: destination.clone(),
            });
            self.events.emit(FilesystemEvent::ReplicationCompleted {
                content_id: content_id.clone(),
                source,
                destination,
            });
        }

        let after = self
            .metadata
            .load()?
            .placements
            .get(content_id)
            .map(ContentPlacement::durability)
            .unwrap_or(DurabilityStatus::Degraded);
        match (before, after) {
            (DurabilityStatus::Healthy, DurabilityStatus::Degraded) => {
                self.events.emit(FilesystemEvent::DurabilityDegraded {
                    content_id: content_id.clone(),
                });
            }
            (DurabilityStatus::Degraded, DurabilityStatus::Healthy) => {
                self.events.emit(FilesystemEvent::DurabilityRestored {
                    content_id: content_id.clone(),
                });
            }
            _ => {}
        }
        Ok(created)
    }

    fn refresh_replicas(
        &self,
        placement: &ContentPlacement,
    ) -> Result<ContentPlacement, FilesystemError> {
        let mut refreshed = placement.clone();
        for replica in &mut refreshed.replicas {
            let Ok(node) = self.registry.node(&replica.node_id) else {
                replica.state = ReplicaState::Missing;
                continue;
            };
            if !node.online {
                // Offline means the recorded copy still exists but cannot be checked now.
                if !matches!(
                    replica.state,
                    ReplicaState::Missing | ReplicaState::Corrupted
                ) {
                    replica.state = ReplicaState::Offline;
                }
                continue;
            }
            let store = self.registry.store(&replica.node_id)?;
            if !store.exists(&placement.content_id)? {
                replica.state = ReplicaState::Missing;
                continue;
            }
            let bytes = match store.get(&placement.content_id) {
                Ok(bytes) => bytes,
                Err(FilesystemError::ContentUnavailable) => {
                    replica.state = ReplicaState::Missing;
                    continue;
                }
                Err(error) => return Err(error),
            };
            if placement.content_id.matches(&bytes) {
                replica.state = ReplicaState::Healthy;
            } else {
                replica.state = ReplicaState::Corrupted;
                self.events.emit(FilesystemEvent::ReplicaCorrupted {
                    content_id: placement.content_id.clone(),
                    node_id: replica.node_id.clone(),
                });
            }
        }
        Ok(refreshed)
    }

    fn find_source(
        &self,
        content_id: &ContentId,
        placement: &ContentPlacement,
    ) -> Result<Option<(DeviceId, Vec<u8>)>, FilesystemError> {
        for replica in &placement.replicas {
            if replica.state != ReplicaState::Healthy {
                continue;
            }
            let Ok(bytes) = self.registry.get_from(&replica.node_id, content_id) else {
                continue;
            };
            if content_id.matches(&bytes) {
                return Ok(Some((replica.node_id.clone(), bytes)));
            }
        }
        Ok(None)
    }

    fn content_size(&self, content_id: &ContentId) -> u64 {
        self.metadata
            .load()
            .ok()
            .and_then(|snapshot| {
                snapshot.files.values().find_map(|file| {
                    file.manifest.as_ref().and_then(|manifest| {
                        manifest
                            .chunks
                            .iter()
                            .find(|chunk| chunk.content_id == *content_id)
                            .map(|chunk| chunk.size)
                    })
                })
            })
            .unwrap_or(0)
    }

    fn upsert_replica(
        &self,
        content_id: &ContentId,
        node_id: &DeviceId,
        state: ReplicaState,
    ) -> Result<(), FilesystemError> {
        self.mutate_placement(content_id, |placement| {
            match placement
                .replicas
                .iter_mut()
                .find(|replica| replica.node_id == *node_id)
            {
                Some(replica) => replica.state = state,
                None => placement.replicas.push(Replica {
                    node_id: node_id.clone(),
                    state,
                }),
            }
        })
    }

    fn store_placement(&self, placement: ContentPlacement) -> Result<(), FilesystemError> {
        let content_id = placement.content_id.clone();
        self.mutate_placement(&content_id, |current| *current = placement.clone())
    }

    fn mutate_placement(
        &self,
        content_id: &ContentId,
        operation: impl Fn(&mut ContentPlacement),
    ) -> Result<(), FilesystemError> {
        for _ in 0..8 {
            let mut snapshot = self.metadata.load()?;
            let placement = snapshot
                .placements
                .get_mut(content_id)
                .ok_or(FilesystemError::ContentUnavailable)?;
            operation(placement);
            match self.metadata.compare_and_swap(snapshot) {
                Ok(()) => return Ok(()),
                Err(FilesystemError::Conflict) => continue,
                Err(error) => return Err(error),
            }
        }
        Err(FilesystemError::Conflict)
    }
}
