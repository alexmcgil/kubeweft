use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use fs2::FileExt;
use serde::Serialize;

use crate::{AgentState, TransportError, domain::new_device_id};

pub(crate) struct AgentDirectoryLock {
    _file: fs::File,
}

#[derive(Clone)]
pub struct ClusterStore {
    state_path: PathBuf,
    lock_path: PathBuf,
    process_lock: Arc<Mutex<()>>,
}

impl ClusterStore {
    pub fn open(
        data_directory: impl AsRef<Path>,
        device_name: Option<&str>,
    ) -> Result<Self, TransportError> {
        let data_directory = data_directory.as_ref();
        fs::create_dir_all(data_directory)?;
        set_private_directory_permissions(data_directory)?;
        let store = Self {
            state_path: data_directory.join("cluster.json"),
            lock_path: data_directory.join("cluster.lock"),
            process_lock: Arc::new(Mutex::new(())),
        };
        {
            let _guard = store
                .process_lock
                .lock()
                .map_err(|_| TransportError::Conflict)?;
            let lock = store.lock_file()?;
            FileExt::lock_exclusive(&lock)?;
            if !store.state_path.exists() {
                let name = normalize_device_name(
                    device_name
                        .map(ToOwned::to_owned)
                        .or_else(|| std::env::var("HOSTNAME").ok())
                        .unwrap_or_else(|| "kubeweft-device".to_owned()),
                )?;
                store.write_state(&AgentState {
                    revision: 0,
                    device_id: new_device_id(),
                    device_name: name,
                    cluster: None,
                    pending_join: None,
                })?;
            } else {
                store.read_state()?;
            }
        }
        Ok(store)
    }

    pub(crate) fn acquire_agent_lock(
        data_directory: impl AsRef<Path>,
    ) -> Result<AgentDirectoryLock, TransportError> {
        let data_directory = data_directory.as_ref();
        fs::create_dir_all(data_directory)?;
        set_private_directory_permissions(data_directory)?;
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(data_directory.join("agent.lock"))?;
        FileExt::try_lock_exclusive(&file).map_err(|error| {
            if error.kind() == std::io::ErrorKind::WouldBlock {
                TransportError::AgentAlreadyRunning
            } else {
                error.into()
            }
        })?;
        Ok(AgentDirectoryLock { _file: file })
    }

    pub fn load(&self) -> Result<AgentState, TransportError> {
        let _guard = self
            .process_lock
            .lock()
            .map_err(|_| TransportError::Conflict)?;
        let lock = self.lock_file()?;
        FileExt::lock_shared(&lock)?;
        self.read_state()
    }

    pub(crate) fn update<T>(
        &self,
        operation: impl FnOnce(&mut AgentState) -> Result<T, TransportError>,
    ) -> Result<T, TransportError> {
        let _guard = self
            .process_lock
            .lock()
            .map_err(|_| TransportError::Conflict)?;
        let lock = self.lock_file()?;
        FileExt::lock_exclusive(&lock)?;
        let mut state = self.read_state()?;
        let result = operation(&mut state)?;
        state.revision = state.revision.saturating_add(1);
        self.write_state(&state)?;
        Ok(result)
    }

    fn read_state(&self) -> Result<AgentState, TransportError> {
        let bytes = fs::read(&self.state_path)?;
        serde_json::from_slice(&bytes)
            .map_err(|error| TransportError::InvalidMessage(error.to_string()))
    }

    fn write_state(&self, state: &AgentState) -> Result<(), TransportError> {
        self.write_json(&self.state_path, state)
    }

    fn write_json(&self, path: &Path, value: &impl Serialize) -> Result<(), TransportError> {
        let bytes = serde_json::to_vec_pretty(value)
            .map_err(|error| TransportError::InvalidMessage(error.to_string()))?;
        let temporary = path.with_extension("tmp");
        let mut file = fs::File::create(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        set_private_permissions(&temporary)?;
        fs::rename(temporary, path)?;
        Ok(())
    }

    fn lock_file(&self) -> Result<fs::File, TransportError> {
        Ok(fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&self.lock_path)?)
    }
}

fn normalize_device_name(name: String) -> Result<String, TransportError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err(TransportError::InvalidMessage(
            "device name must be 1-128 printable characters".into(),
        ));
    }
    Ok(name.to_owned())
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<(), TransportError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), TransportError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> Result<(), TransportError> {
    Ok(())
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<(), TransportError> {
    Ok(())
}
