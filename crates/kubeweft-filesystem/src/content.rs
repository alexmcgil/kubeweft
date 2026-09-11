use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
};

use kubeweft_model::DeviceId;

use crate::{ContentId, FilesystemError, StorageNode};

pub trait ContentStore: Send + Sync {
    fn put(&self, data: &[u8]) -> Result<ContentId, FilesystemError>;
    fn get(&self, content_id: &ContentId) -> Result<Vec<u8>, FilesystemError>;
    fn exists(&self, content_id: &ContentId) -> Result<bool, FilesystemError>;
    fn delete(&self, content_id: &ContentId) -> Result<(), FilesystemError>;
}

#[derive(Debug)]
pub struct InMemoryContentStore {
    blobs: Mutex<BTreeMap<ContentId, Vec<u8>>>,
    capacity: u64,
}

impl InMemoryContentStore {
    pub fn new(capacity: u64) -> Self {
        Self {
            blobs: Mutex::new(BTreeMap::new()),
            capacity,
        }
    }

    /// Test/adapter hook that simulates media corruption without changing the key.
    pub fn overwrite_unchecked(
        &self,
        content_id: &ContentId,
        data: Vec<u8>,
    ) -> Result<(), FilesystemError> {
        self.blobs
            .lock()
            .map_err(|_| FilesystemError::Storage("content lock poisoned".into()))?
            .insert(content_id.clone(), data);
        Ok(())
    }
}

impl ContentStore for InMemoryContentStore {
    fn put(&self, data: &[u8]) -> Result<ContentId, FilesystemError> {
        let content_id = ContentId::for_bytes(data);
        let mut blobs = self
            .blobs
            .lock()
            .map_err(|_| FilesystemError::Storage("content lock poisoned".into()))?;
        if blobs
            .get(&content_id)
            .is_some_and(|existing| content_id.matches(existing))
        {
            return Ok(content_id);
        }
        let used: u64 = blobs
            .iter()
            .filter(|(id, _)| **id != content_id)
            .map(|(_, blob)| u64::try_from(blob.len()).unwrap_or(u64::MAX))
            .sum();
        let size = u64::try_from(data.len()).map_err(|_| FilesystemError::NoSpace)?;
        if used.saturating_add(size) > self.capacity {
            return Err(FilesystemError::NoSpace);
        }
        blobs.insert(content_id.clone(), data.to_vec());
        Ok(content_id)
    }

    fn get(&self, content_id: &ContentId) -> Result<Vec<u8>, FilesystemError> {
        self.blobs
            .lock()
            .map_err(|_| FilesystemError::Storage("content lock poisoned".into()))?
            .get(content_id)
            .cloned()
            .ok_or(FilesystemError::ContentUnavailable)
    }

    fn exists(&self, content_id: &ContentId) -> Result<bool, FilesystemError> {
        Ok(self
            .blobs
            .lock()
            .map_err(|_| FilesystemError::Storage("content lock poisoned".into()))?
            .contains_key(content_id))
    }

    fn delete(&self, content_id: &ContentId) -> Result<(), FilesystemError> {
        self.blobs
            .lock()
            .map_err(|_| FilesystemError::Storage("content lock poisoned".into()))?
            .remove(content_id);
        Ok(())
    }
}

#[derive(Debug)]
pub struct LocalContentStore {
    root: PathBuf,
    capacity: u64,
    write_lock: Mutex<()>,
}

impl LocalContentStore {
    pub fn open(root: impl AsRef<Path>, capacity: u64) -> Result<Self, FilesystemError> {
        fs::create_dir_all(root.as_ref())?;
        Ok(Self {
            root: root.as_ref().to_owned(),
            capacity,
            write_lock: Mutex::new(()),
        })
    }

    fn path_for(&self, content_id: &ContentId) -> PathBuf {
        self.root.join(
            content_id
                .as_str()
                .strip_prefix("sha256:")
                .unwrap_or(content_id.as_str()),
        )
    }

    fn used_space(&self) -> Result<u64, FilesystemError> {
        fs::read_dir(&self.root)?.try_fold(0_u64, |total, entry| {
            let metadata = entry?.metadata()?;
            Ok(total.saturating_add(if metadata.is_file() {
                metadata.len()
            } else {
                0
            }))
        })
    }
}

impl ContentStore for LocalContentStore {
    fn put(&self, data: &[u8]) -> Result<ContentId, FilesystemError> {
        let content_id = ContentId::for_bytes(data);
        let destination = self.path_for(&content_id);
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| FilesystemError::Storage("content lock poisoned".into()))?;
        if destination.exists() {
            let existing = fs::read(&destination)?;
            if content_id.matches(&existing) {
                return Ok(content_id);
            }
            fs::remove_file(&destination)?;
        }
        let size = u64::try_from(data.len()).map_err(|_| FilesystemError::NoSpace)?;
        if self.used_space()?.saturating_add(size) > self.capacity {
            return Err(FilesystemError::NoSpace);
        }
        let temporary = destination.with_extension("tmp");
        let mut file = fs::File::create(&temporary)?;
        file.write_all(data)?;
        file.sync_all()?;
        fs::rename(temporary, destination)?;
        Ok(content_id)
    }

    fn get(&self, content_id: &ContentId) -> Result<Vec<u8>, FilesystemError> {
        fs::read(self.path_for(content_id)).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                FilesystemError::ContentUnavailable
            } else {
                error.into()
            }
        })
    }

    fn exists(&self, content_id: &ContentId) -> Result<bool, FilesystemError> {
        Ok(self.path_for(content_id).exists())
    }

    fn delete(&self, content_id: &ContentId) -> Result<(), FilesystemError> {
        match fs::remove_file(self.path_for(content_id)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[derive(Clone)]
struct RegisteredNode {
    state: StorageNode,
    store: Arc<dyn ContentStore>,
}

#[derive(Clone, Default)]
pub struct ContentStoreRegistry {
    nodes: Arc<RwLock<BTreeMap<DeviceId, RegisteredNode>>>,
}

impl ContentStoreRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &self,
        node: StorageNode,
        store: Arc<dyn ContentStore>,
    ) -> Result<(), FilesystemError> {
        self.nodes
            .write()
            .map_err(|_| FilesystemError::Storage("registry lock poisoned".into()))?
            .insert(
                node.device_id.clone(),
                RegisteredNode { state: node, store },
            );
        Ok(())
    }

    pub fn nodes(&self) -> Result<Vec<StorageNode>, FilesystemError> {
        Ok(self
            .nodes
            .read()
            .map_err(|_| FilesystemError::Storage("registry lock poisoned".into()))?
            .values()
            .map(|node| node.state.clone())
            .collect())
    }

    pub fn node(&self, node_id: &DeviceId) -> Result<StorageNode, FilesystemError> {
        self.nodes
            .read()
            .map_err(|_| FilesystemError::Storage("registry lock poisoned".into()))?
            .get(node_id)
            .map(|node| node.state.clone())
            .ok_or(FilesystemError::NodeNotFound)
    }

    pub fn set_online(&self, node_id: &DeviceId, online: bool) -> Result<(), FilesystemError> {
        self.nodes
            .write()
            .map_err(|_| FilesystemError::Storage("registry lock poisoned".into()))?
            .get_mut(node_id)
            .ok_or(FilesystemError::NodeNotFound)?
            .state
            .online = online;
        Ok(())
    }

    pub fn store(&self, node_id: &DeviceId) -> Result<Arc<dyn ContentStore>, FilesystemError> {
        self.nodes
            .read()
            .map_err(|_| FilesystemError::Storage("registry lock poisoned".into()))?
            .get(node_id)
            .map(|node| Arc::clone(&node.store))
            .ok_or(FilesystemError::NodeNotFound)
    }

    pub fn put_on(&self, node_id: &DeviceId, data: &[u8]) -> Result<ContentId, FilesystemError> {
        if !self.node(node_id)?.online {
            return Err(FilesystemError::ContentUnavailable);
        }
        self.store(node_id)?.put(data)
    }

    pub fn get_from(
        &self,
        node_id: &DeviceId,
        content_id: &ContentId,
    ) -> Result<Vec<u8>, FilesystemError> {
        if !self.node(node_id)?.online {
            return Err(FilesystemError::ContentUnavailable);
        }
        self.store(node_id)?.get(content_id)
    }
}
