use std::{fmt, str::FromStr};

use kubeweft_model::DeviceId;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use uuid::Uuid;

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

uuid_id!(FileId);
uuid_id!(DirectoryId);
uuid_id!(LeaseId);

/// Monotonic generation used for optimistic file updates.
pub type FileVersion = u64;

impl DirectoryId {
    pub(crate) const fn root() -> Self {
        Self(Uuid::nil())
    }
}

/// Cryptographic identity of immutable bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentId(String);

impl ContentId {
    pub fn for_bytes(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        Self(format!("sha256:{digest:x}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn matches(&self, bytes: &[u8]) -> bool {
        *self == Self::for_bytes(bytes)
    }
}

impl fmt::Display for ContentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for ContentId {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let hash = value.strip_prefix("sha256:").ok_or(())?;
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(());
        }
        Ok(Self(value.to_ascii_lowercase()))
    }
}

impl Serialize for ContentId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ContentId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(|()| serde::de::Error::custom("invalid SHA-256 content id"))
    }
}

/// One immutable object referenced by a logical file manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkReference {
    pub content_id: ContentId,
    pub offset: u64,
    pub size: u64,
}

/// Logical content description. MVP files contain one chunk; the model permits more.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentManifest {
    pub size: u64,
    pub chunks: Vec<ChunkReference>,
}

impl ContentManifest {
    pub fn single(content_id: ContentId, size: u64) -> Self {
        Self {
            size,
            chunks: vec![ChunkReference {
                content_id,
                offset: 0,
                size,
            }],
        }
    }

    pub fn content_id(&self) -> Option<&ContentId> {
        (self.chunks.len() == 1).then(|| &self.chunks[0].content_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    pub id: FileId,
    pub size: u64,
    pub generation: FileVersion,
    pub created_at_millis: u64,
    pub modified_at_millis: u64,
    pub manifest: Option<ContentManifest>,
    /// Reserved coordination state. Generation CAS is the active MVP protection.
    pub write_lease: Option<WriteLease>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteLease {
    pub id: LeaseId,
    pub holder: DeviceId,
    pub expires_at_millis: u64,
    pub fencing_token: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryMetadata {
    pub id: DirectoryId,
    pub generation: u64,
    pub created_at_millis: u64,
    pub modified_at_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum NamespaceTarget {
    File(FileId),
    Directory(DirectoryId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamespaceEntry {
    pub parent: DirectoryId,
    pub name: String,
    pub target: NamespaceTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerSource {
    Mains,
    Battery,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Availability {
    Stable,
    Intermittent,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkKind {
    Lan,
    Wifi,
    Cellular,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageNode {
    pub device_id: DeviceId,
    pub online: bool,
    pub free_space: u64,
    pub total_space: u64,
    pub power: PowerSource,
    pub availability: Availability,
    pub network: NetworkKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicaState {
    Healthy,
    Pending,
    Offline,
    Missing,
    Corrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replica {
    pub node_id: DeviceId,
    pub state: ReplicaState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DurabilityStatus {
    Healthy,
    Degraded,
}

/// Whether immutable content should still receive durability maintenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentRetention {
    LiveReference,
    Snapshot,
    GarbageCandidate,
}

impl ContentRetention {
    pub fn is_retained(self) -> bool {
        matches!(self, Self::LiveReference | Self::Snapshot)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentPlacement {
    pub content_id: ContentId,
    pub desired_replica_count: usize,
    pub replicas: Vec<Replica>,
}

impl ContentPlacement {
    pub fn durability(&self) -> DurabilityStatus {
        let present = self
            .replicas
            .iter()
            .filter(|replica| {
                matches!(replica.state, ReplicaState::Healthy | ReplicaState::Offline)
            })
            .count();
        if present >= self.desired_replica_count {
            DurabilityStatus::Healthy
        } else {
            DurabilityStatus::Degraded
        }
    }
}
