use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::{
    ContentId, ContentPlacement, DirectoryId, DirectoryMetadata, FileId, FileMetadata,
    FilesystemError, NamespaceEntry,
};

/// Complete authoritative metadata state for the MVP.
///
/// The snapshot/CAS boundary can later be implemented by a transactional KV or
/// consensus-backed store without changing namespace and filesystem semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataSnapshot {
    pub revision: u64,
    pub root: DirectoryId,
    pub directories: BTreeMap<DirectoryId, DirectoryMetadata>,
    pub files: BTreeMap<FileId, FileMetadata>,
    pub entries: Vec<NamespaceEntry>,
    pub placements: BTreeMap<ContentId, ContentPlacement>,
}

impl Default for MetadataSnapshot {
    fn default() -> Self {
        let root = DirectoryId::root();
        let root_metadata = DirectoryMetadata {
            id: root,
            generation: 1,
            created_at_millis: 0,
            modified_at_millis: 0,
        };
        Self {
            revision: 0,
            root,
            directories: BTreeMap::from([(root, root_metadata)]),
            files: BTreeMap::new(),
            entries: Vec::new(),
            placements: BTreeMap::new(),
        }
    }
}

pub trait MetadataStore: Send + Sync {
    fn load(&self) -> Result<MetadataSnapshot, FilesystemError>;

    /// Atomically installs `replacement` if its revision is still current.
    fn compare_and_swap(&self, replacement: MetadataSnapshot) -> Result<(), FilesystemError>;
}

#[derive(Debug, Default)]
pub struct InMemoryMetadataStore {
    state: Mutex<MetadataSnapshot>,
}

impl InMemoryMetadataStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MetadataStore for InMemoryMetadataStore {
    fn load(&self) -> Result<MetadataSnapshot, FilesystemError> {
        self.state
            .lock()
            .map(|state| state.clone())
            .map_err(|_| FilesystemError::MetadataUnavailable("metadata lock poisoned".into()))
    }

    fn compare_and_swap(&self, mut replacement: MetadataSnapshot) -> Result<(), FilesystemError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| FilesystemError::MetadataUnavailable("metadata lock poisoned".into()))?;
        if state.revision != replacement.revision {
            return Err(FilesystemError::Conflict);
        }
        replacement.revision += 1;
        *state = replacement;
        Ok(())
    }
}

/// Persistent local backend using atomic whole-snapshot replacement.
/// It is suitable for development, but does not claim production crash recovery.
#[derive(Debug)]
pub struct LocalMetadataStore {
    path: PathBuf,
    lock_path: PathBuf,
    process_lock: Mutex<()>,
}

impl LocalMetadataStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, FilesystemError> {
        let path = path.as_ref().to_owned();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|error| FilesystemError::MetadataUnavailable(error.to_string()))?;
        }
        if path.exists() {
            Self::read_snapshot(&path)?;
        }
        let lock_path = path.with_extension("lock");
        Ok(Self {
            path,
            lock_path,
            process_lock: Mutex::new(()),
        })
    }

    fn read_snapshot(path: &Path) -> Result<MetadataSnapshot, FilesystemError> {
        if !path.exists() {
            return Ok(MetadataSnapshot::default());
        }
        let bytes = fs::read(path)
            .map_err(|error| FilesystemError::MetadataUnavailable(error.to_string()))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| FilesystemError::MetadataUnavailable(error.to_string()))
    }

    fn lock_file(&self) -> Result<fs::File, FilesystemError> {
        fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&self.lock_path)
            .map_err(|error| FilesystemError::MetadataUnavailable(error.to_string()))
    }

    fn persist(&self, state: &MetadataSnapshot) -> Result<(), FilesystemError> {
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|error| FilesystemError::MetadataUnavailable(error.to_string()))?;
        }
        let temporary = self.path.with_extension("tmp");
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|error| FilesystemError::MetadataUnavailable(error.to_string()))?;
        let mut file = fs::File::create(&temporary)
            .map_err(|error| FilesystemError::MetadataUnavailable(error.to_string()))?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| FilesystemError::MetadataUnavailable(error.to_string()))?;
        fs::rename(&temporary, &self.path)
            .map_err(|error| FilesystemError::MetadataUnavailable(error.to_string()))
    }
}

impl MetadataStore for LocalMetadataStore {
    fn load(&self) -> Result<MetadataSnapshot, FilesystemError> {
        let _process_guard = self
            .process_lock
            .lock()
            .map_err(|_| FilesystemError::MetadataUnavailable("metadata lock poisoned".into()))?;
        let lock = self.lock_file()?;
        FileExt::lock_shared(&lock)
            .map_err(|error| FilesystemError::MetadataUnavailable(error.to_string()))?;
        let result = Self::read_snapshot(&self.path);
        FileExt::unlock(&lock)
            .map_err(|error| FilesystemError::MetadataUnavailable(error.to_string()))?;
        result
    }

    fn compare_and_swap(&self, mut replacement: MetadataSnapshot) -> Result<(), FilesystemError> {
        let _process_guard = self
            .process_lock
            .lock()
            .map_err(|_| FilesystemError::MetadataUnavailable("metadata lock poisoned".into()))?;
        let lock = self.lock_file()?;
        FileExt::lock_exclusive(&lock)
            .map_err(|error| FilesystemError::MetadataUnavailable(error.to_string()))?;
        let current = Self::read_snapshot(&self.path)?;
        if current.revision != replacement.revision {
            FileExt::unlock(&lock)
                .map_err(|error| FilesystemError::MetadataUnavailable(error.to_string()))?;
            return Err(FilesystemError::Conflict);
        }
        replacement.revision += 1;
        let result = self.persist(&replacement);
        FileExt::unlock(&lock)
            .map_err(|error| FilesystemError::MetadataUnavailable(error.to_string()))?;
        result
    }
}
