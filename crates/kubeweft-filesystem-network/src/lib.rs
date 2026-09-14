//! Adapters between the transport data plane and filesystem content stores.
//!
//! Namespace and metadata never cross this boundary. Peers exchange immutable
//! content addressed blobs only.

use std::{str::FromStr, sync::Arc};

use kubeweft_filesystem::{ContentId, ContentStore, FilesystemError};
use kubeweft_model::DeviceId;
use kubeweft_transport::{ClusterContentClient, PeerContentStore, TransportError};

/// Exposes a filesystem content store to authorized cluster peers.
pub struct FilesystemPeerContentStore {
    store: Arc<dyn ContentStore>,
}

impl FilesystemPeerContentStore {
    pub fn new(store: Arc<dyn ContentStore>) -> Self {
        Self { store }
    }
}

impl PeerContentStore for FilesystemPeerContentStore {
    fn get(&self, content_id: &str) -> Result<Vec<u8>, TransportError> {
        let content_id = parse_content_id(content_id)?;
        let bytes = self.store.get(&content_id).map_err(map_filesystem_error)?;
        if !content_id.matches(&bytes) {
            return Err(TransportError::ContentIntegrity);
        }
        Ok(bytes)
    }

    fn put(&self, content_id: &str, data: &[u8]) -> Result<(), TransportError> {
        let expected = parse_content_id(content_id)?;
        if !expected.matches(data) {
            return Err(TransportError::ContentIntegrity);
        }
        let actual = self.store.put(data).map_err(map_filesystem_error)?;
        if actual != expected {
            return Err(TransportError::ContentIntegrity);
        }
        Ok(())
    }

    fn exists(&self, content_id: &str) -> Result<bool, TransportError> {
        let content_id = parse_content_id(content_id)?;
        if !self
            .store
            .exists(&content_id)
            .map_err(map_filesystem_error)?
        {
            return Ok(false);
        }
        match self.store.get(&content_id) {
            Ok(bytes) => Ok(content_id.matches(&bytes)),
            Err(FilesystemError::ContentUnavailable) => Ok(false),
            Err(error) => Err(map_filesystem_error(error)),
        }
    }
}

/// Presents one remote cluster member as a filesystem content store.
pub struct RemoteContentStore {
    client: ClusterContentClient,
    target: DeviceId,
}

impl RemoteContentStore {
    pub fn new(client: ClusterContentClient, target: DeviceId) -> Self {
        Self { client, target }
    }
}

impl ContentStore for RemoteContentStore {
    fn put(&self, data: &[u8]) -> Result<ContentId, FilesystemError> {
        let content_id = ContentId::for_bytes(data);
        self.client
            .put(&self.target, content_id.as_str(), data)
            .map_err(map_transport_error)?;
        Ok(content_id)
    }

    fn get(&self, content_id: &ContentId) -> Result<Vec<u8>, FilesystemError> {
        let bytes = self
            .client
            .get(&self.target, content_id.as_str())
            .map_err(map_transport_error)?;
        if !content_id.matches(&bytes) {
            return Err(FilesystemError::ContentUnavailable);
        }
        Ok(bytes)
    }

    fn exists(&self, content_id: &ContentId) -> Result<bool, FilesystemError> {
        self.client
            .exists(&self.target, content_id.as_str())
            .map_err(map_transport_error)
    }

    fn delete(&self, _content_id: &ContentId) -> Result<(), FilesystemError> {
        Err(FilesystemError::Storage(
            "remote content deletion is reserved for a future garbage collector".into(),
        ))
    }
}

fn parse_content_id(value: &str) -> Result<ContentId, TransportError> {
    ContentId::from_str(value).map_err(|()| TransportError::ContentIntegrity)
}

fn map_filesystem_error(error: FilesystemError) -> TransportError {
    match error {
        FilesystemError::ContentUnavailable => TransportError::ContentUnavailable,
        FilesystemError::NoSpace => TransportError::NoSpace,
        other => TransportError::InvalidMessage(other.to_string()),
    }
}

fn map_transport_error(error: TransportError) -> FilesystemError {
    match error {
        TransportError::ContentUnavailable
        | TransportError::ContentIntegrity
        | TransportError::AgentUnavailable => FilesystemError::ContentUnavailable,
        TransportError::NoSpace => FilesystemError::NoSpace,
        TransportError::Remote { code, .. }
            if matches!(
                code.as_str(),
                "content_unavailable" | "content_integrity" | "agent_unavailable"
            ) =>
        {
            FilesystemError::ContentUnavailable
        }
        TransportError::Remote { code, .. } if code == "no_space" => FilesystemError::NoSpace,
        other => FilesystemError::Storage(other.to_string()),
    }
}
