//! Core for Kubeweft's cluster-owned logical filesystem.
//!
//! This crate owns namespace and file semantics while storage adapters and
//! authoritative metadata implementations remain behind traits. It has no
//! operating-system mount or network transport dependencies.

mod content;
mod domain;
mod error;
mod event;
mod metadata;
mod namespace;
mod path;
mod placement;
mod replication;
mod service;

pub use content::{ContentStore, ContentStoreRegistry, InMemoryContentStore, LocalContentStore};
pub use domain::{
    Availability, ChunkReference, ContentId, ContentManifest, ContentPlacement, ContentRetention,
    DirectoryId, DirectoryMetadata, DurabilityStatus, FileId, FileMetadata, FileType, FileVersion,
    LeaseId, NamespaceEntry, NamespaceTarget, NetworkKind, PowerSource, Replica, ReplicaState,
    StorageNode, WriteLease,
};
pub use error::FilesystemError;
pub use event::{EventSink, FilesystemEvent, RecordingEventSink};
pub use metadata::{InMemoryMetadataStore, LocalMetadataStore, MetadataSnapshot, MetadataStore};
pub use namespace::{EntryMetadata, ListedEntry, Namespace};
pub use placement::{DefaultPlacementPolicy, PlacementPolicy, PlacementRequest};
pub use replication::{ReconcileReport, ReplicationReconciler};
pub use service::FilesystemService;
