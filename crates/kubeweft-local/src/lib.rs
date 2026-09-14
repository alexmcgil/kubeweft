//! Local application wiring for runnable Kubeweft clients.
//!
//! This crate chooses the development metadata and blob adapters. Namespace and
//! content semantics remain in `kubeweft-filesystem`.

use std::{path::Path, sync::Arc};

use kubeweft_filesystem::{
    Availability, ContentStoreRegistry, EntryMetadata, FilesystemError, FilesystemService,
    LocalContentStore, LocalMetadataStore, NetworkKind, PowerSource, StorageNode,
};
use kubeweft_model::DeviceId;
use kubeweft_transport::ClusterStore;

const LOCAL_CAPACITY: u64 = 100 * 1024 * 1024 * 1024;

#[derive(Clone)]
pub struct LocalFilesystem {
    service: FilesystemService,
    device_id: DeviceId,
}

impl LocalFilesystem {
    pub fn open(data_directory: impl AsRef<Path>) -> Result<Self, FilesystemError> {
        let data_directory = data_directory.as_ref();
        std::fs::create_dir_all(data_directory)?;

        let metadata = Arc::new(LocalMetadataStore::open(
            data_directory.join("metadata.json"),
        )?);
        let registry = Arc::new(ContentStoreRegistry::new());
        let device_id = ClusterStore::open(data_directory, None)
            .and_then(|store| store.load())
            .map_err(|error| FilesystemError::Storage(error.to_string()))?
            .device_id()
            .clone();
        registry.register(
            StorageNode {
                device_id: device_id.clone(),
                online: true,
                free_space: LOCAL_CAPACITY,
                total_space: LOCAL_CAPACITY,
                power: PowerSource::Mains,
                availability: Availability::Stable,
                network: NetworkKind::Lan,
            },
            Arc::new(LocalContentStore::open(
                data_directory.join("content").join("blobs"),
                LOCAL_CAPACITY,
            )?),
        )?;

        Ok(Self {
            service: FilesystemService::new(metadata, registry),
            device_id,
        })
    }

    pub fn service(&self) -> &FilesystemService {
        &self.service
    }

    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    pub fn ensure_directory_tree(&self, path: &str) -> Result<(), FilesystemError> {
        if path == "/" {
            return Ok(());
        }
        let mut current = String::new();
        for component in path
            .strip_prefix('/')
            .ok_or(FilesystemError::InvalidPath)?
            .split('/')
        {
            if component.is_empty() {
                return Err(FilesystemError::InvalidPath);
            }
            current.push('/');
            current.push_str(component);
            match self.service.stat(&current) {
                Ok(EntryMetadata::Directory(_)) => {}
                Ok(EntryMetadata::File(_)) => return Err(FilesystemError::NotDirectory),
                Err(FilesystemError::NotFound) => self.service.mkdir(&current)?,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    pub fn bootstrap_home(&self, user: &str) -> Result<String, FilesystemError> {
        if user.is_empty()
            || !user
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(FilesystemError::InvalidPath);
        }
        let home = format!("/home/{user}");
        self.ensure_directory_tree(&home)?;
        Ok(home)
    }
}
