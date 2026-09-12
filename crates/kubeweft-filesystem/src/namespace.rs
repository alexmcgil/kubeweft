use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    ContentRetention, DirectoryId, DirectoryMetadata, FileId, FileMetadata, FileType,
    FilesystemError, MetadataSnapshot, MetadataStore, NamespaceEntry, NamespaceTarget,
    path::FsPath,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryMetadata {
    File(FileMetadata),
    Directory(DirectoryMetadata),
}

impl EntryMetadata {
    pub fn file_type(&self) -> FileType {
        match self {
            Self::File(_) => FileType::File,
            Self::Directory(_) => FileType::Directory,
        }
    }

    pub fn generation(&self) -> u64 {
        match self {
            Self::File(metadata) => metadata.generation,
            Self::Directory(metadata) => metadata.generation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedEntry {
    pub name: String,
    pub metadata: EntryMetadata,
}

#[derive(Clone)]
pub struct Namespace {
    store: Arc<dyn MetadataStore>,
}

impl Namespace {
    pub fn new(store: Arc<dyn MetadataStore>) -> Self {
        Self { store }
    }

    pub fn metadata_store(&self) -> &Arc<dyn MetadataStore> {
        &self.store
    }

    pub fn resolve(&self, path: &str) -> Result<NamespaceTarget, FilesystemError> {
        let path = FsPath::parse(path)?;
        let snapshot = self.store.load()?;
        Self::resolve_components(&snapshot, path.components())
    }

    pub fn stat(&self, path: &str) -> Result<EntryMetadata, FilesystemError> {
        let path = FsPath::parse(path)?;
        let snapshot = self.store.load()?;
        let target = Self::resolve_components(&snapshot, path.components())?;
        Self::metadata_for(&snapshot, target)
    }

    pub fn list(&self, path: &str) -> Result<Vec<ListedEntry>, FilesystemError> {
        let path = FsPath::parse(path)?;
        let snapshot = self.store.load()?;
        let directory = match Self::resolve_components(&snapshot, path.components())? {
            NamespaceTarget::Directory(id) => id,
            NamespaceTarget::File(_) => return Err(FilesystemError::NotDirectory),
        };
        let mut result = snapshot
            .entries
            .iter()
            .filter(|entry| entry.parent == directory)
            .map(|entry| {
                Ok(ListedEntry {
                    name: entry.name.clone(),
                    metadata: Self::metadata_for(&snapshot, entry.target)?,
                })
            })
            .collect::<Result<Vec<_>, FilesystemError>>()?;
        result.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(result)
    }

    pub fn mkdir(&self, path: &str) -> Result<DirectoryId, FilesystemError> {
        let path = FsPath::parse(path)?;
        let (parents, name) = path.split_parent()?;
        let id = DirectoryId::new();
        self.retry_mutation(|snapshot| {
            let parent = Self::resolve_directory(snapshot, parents)?;
            Self::ensure_absent(snapshot, parent, name)?;
            let now = now_millis();
            snapshot.directories.insert(
                id,
                DirectoryMetadata {
                    id,
                    generation: 1,
                    created_at_millis: now,
                    modified_at_millis: now,
                },
            );
            snapshot.entries.push(NamespaceEntry {
                parent,
                name: name.to_owned(),
                target: NamespaceTarget::Directory(id),
            });
            Self::touch_directory(snapshot, parent);
            Ok(id)
        })
    }

    pub fn create(&self, path: &str) -> Result<FileId, FilesystemError> {
        let path = FsPath::parse(path)?;
        let (parents, name) = path.split_parent()?;
        let id = FileId::new();
        self.retry_mutation(|snapshot| {
            let parent = Self::resolve_directory(snapshot, parents)?;
            Self::ensure_absent(snapshot, parent, name)?;
            let now = now_millis();
            snapshot.files.insert(
                id,
                FileMetadata {
                    id,
                    size: 0,
                    generation: 1,
                    created_at_millis: now,
                    modified_at_millis: now,
                    manifest: None,
                    write_lease: None,
                },
            );
            snapshot.entries.push(NamespaceEntry {
                parent,
                name: name.to_owned(),
                target: NamespaceTarget::File(id),
            });
            Self::touch_directory(snapshot, parent);
            Ok(id)
        })
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<(), FilesystemError> {
        let from = FsPath::parse(from)?;
        let to = FsPath::parse(to)?;
        let (from_parents, from_name) = from.split_parent()?;
        let (to_parents, to_name) = to.split_parent()?;
        self.retry_mutation(|snapshot| {
            let from_parent = Self::resolve_directory(snapshot, from_parents)?;
            let to_parent = Self::resolve_directory(snapshot, to_parents)?;
            Self::ensure_absent(snapshot, to_parent, to_name)?;
            let index = snapshot
                .entries
                .iter()
                .position(|entry| entry.parent == from_parent && entry.name == from_name)
                .ok_or(FilesystemError::NotFound)?;
            let target = snapshot.entries[index].target;
            if let NamespaceTarget::Directory(directory) = target {
                if Self::is_descendant(snapshot, to_parent, directory) {
                    return Err(FilesystemError::InvalidPath);
                }
            }
            snapshot.entries[index].parent = to_parent;
            snapshot.entries[index].name = to_name.to_owned();
            Self::touch_directory(snapshot, from_parent);
            if to_parent != from_parent {
                Self::touch_directory(snapshot, to_parent);
            }
            Ok(())
        })
    }

    pub fn remove(&self, path: &str) -> Result<(), FilesystemError> {
        let path = FsPath::parse(path)?;
        let (parents, name) = path.split_parent()?;
        self.retry_mutation(|snapshot| {
            let parent = Self::resolve_directory(snapshot, parents)?;
            let index = snapshot
                .entries
                .iter()
                .position(|entry| entry.parent == parent && entry.name == name)
                .ok_or(FilesystemError::NotFound)?;
            match snapshot.entries[index].target {
                NamespaceTarget::File(id) => {
                    let removed = snapshot.files.remove(&id);
                    if let Some(manifest) = removed.and_then(|file| file.manifest) {
                        for chunk in manifest.chunks {
                            if !snapshot.is_content_live_referenced(&chunk.content_id)
                                && snapshot.retentions.get(&chunk.content_id)
                                    != Some(&ContentRetention::Snapshot)
                            {
                                snapshot
                                    .retentions
                                    .insert(chunk.content_id, ContentRetention::GarbageCandidate);
                            }
                        }
                    }
                }
                NamespaceTarget::Directory(id) => {
                    if snapshot.entries.iter().any(|entry| entry.parent == id) {
                        return Err(FilesystemError::DirectoryNotEmpty);
                    }
                    snapshot.directories.remove(&id);
                }
            }
            snapshot.entries.remove(index);
            Self::touch_directory(snapshot, parent);
            Ok(())
        })
    }

    fn retry_mutation<T>(
        &self,
        operation: impl Fn(&mut MetadataSnapshot) -> Result<T, FilesystemError>,
    ) -> Result<T, FilesystemError> {
        for _ in 0..8 {
            let mut snapshot = self.store.load()?;
            let result = operation(&mut snapshot)?;
            match self.store.compare_and_swap(snapshot) {
                Ok(()) => return Ok(result),
                Err(FilesystemError::Conflict) => continue,
                Err(error) => return Err(error),
            }
        }
        Err(FilesystemError::Conflict)
    }

    pub(crate) fn resolve_components(
        snapshot: &MetadataSnapshot,
        components: &[String],
    ) -> Result<NamespaceTarget, FilesystemError> {
        let mut current = snapshot.root;
        if components.is_empty() {
            return Ok(NamespaceTarget::Directory(current));
        }
        for (index, component) in components.iter().enumerate() {
            let entry = snapshot
                .entries
                .iter()
                .find(|entry| entry.parent == current && entry.name == *component)
                .ok_or(FilesystemError::NotFound)?;
            match entry.target {
                NamespaceTarget::Directory(id) => current = id,
                NamespaceTarget::File(id) if index + 1 == components.len() => {
                    return Ok(NamespaceTarget::File(id));
                }
                NamespaceTarget::File(_) => return Err(FilesystemError::NotDirectory),
            }
        }
        Ok(NamespaceTarget::Directory(current))
    }

    fn resolve_directory(
        snapshot: &MetadataSnapshot,
        components: &[String],
    ) -> Result<DirectoryId, FilesystemError> {
        match Self::resolve_components(snapshot, components)? {
            NamespaceTarget::Directory(id) => Ok(id),
            NamespaceTarget::File(_) => Err(FilesystemError::NotDirectory),
        }
    }

    fn metadata_for(
        snapshot: &MetadataSnapshot,
        target: NamespaceTarget,
    ) -> Result<EntryMetadata, FilesystemError> {
        match target {
            NamespaceTarget::File(id) => snapshot.files.get(&id).cloned().map(EntryMetadata::File),
            NamespaceTarget::Directory(id) => snapshot
                .directories
                .get(&id)
                .cloned()
                .map(EntryMetadata::Directory),
        }
        .ok_or_else(|| {
            FilesystemError::MetadataUnavailable("namespace target has no metadata".into())
        })
    }

    fn ensure_absent(
        snapshot: &MetadataSnapshot,
        parent: DirectoryId,
        name: &str,
    ) -> Result<(), FilesystemError> {
        if snapshot
            .entries
            .iter()
            .any(|entry| entry.parent == parent && entry.name == name)
        {
            Err(FilesystemError::AlreadyExists)
        } else {
            Ok(())
        }
    }

    fn touch_directory(snapshot: &mut MetadataSnapshot, id: DirectoryId) {
        if let Some(directory) = snapshot.directories.get_mut(&id) {
            directory.generation += 1;
            directory.modified_at_millis = now_millis();
        }
    }

    fn is_descendant(
        snapshot: &MetadataSnapshot,
        mut candidate: DirectoryId,
        ancestor: DirectoryId,
    ) -> bool {
        loop {
            if candidate == ancestor {
                return true;
            }
            if candidate == snapshot.root {
                return false;
            }
            let Some(parent) = snapshot.entries.iter().find_map(|entry| {
                (entry.target == NamespaceTarget::Directory(candidate)).then_some(entry.parent)
            }) else {
                return false;
            };
            candidate = parent;
        }
    }
}

pub(crate) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
