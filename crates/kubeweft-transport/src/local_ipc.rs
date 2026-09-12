//! User-scoped local control transport boundary.
//!
//! Unix domain sockets are the current backend. Other platforms can provide a
//! backend here without changing cluster-domain or peer-transport messages.

#[cfg(unix)]
mod platform {
    use std::{
        fs,
        os::unix::{
            fs::PermissionsExt,
            net::{UnixListener, UnixStream},
        },
        path::{Path, PathBuf},
    };

    use crate::TransportError;

    pub(crate) type LocalListener = UnixListener;
    pub(crate) type LocalStream = UnixStream;

    pub(crate) fn socket_path(data_directory: &Path) -> PathBuf {
        data_directory.join("agent.sock")
    }

    pub(crate) fn bind(path: &Path) -> Result<LocalListener, TransportError> {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let listener = UnixListener::bind(path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        Ok(listener)
    }

    pub(crate) fn connect(path: &Path) -> Result<LocalStream, TransportError> {
        UnixStream::connect(path).map_err(|error| match error.kind() {
            std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound => {
                TransportError::AgentUnavailable
            }
            _ => error.into(),
        })
    }
}

#[cfg(not(unix))]
mod platform {
    use std::{
        io::{Read, Write},
        path::{Path, PathBuf},
    };

    use crate::TransportError;

    pub(crate) struct LocalListener;
    pub(crate) struct LocalStream;

    impl LocalListener {
        pub(crate) fn accept(&self) -> std::io::Result<(LocalStream, ())> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "local IPC backend is not implemented for this platform",
            ))
        }
    }

    impl Read for LocalStream {
        fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::Unsupported.into())
        }
    }

    impl Write for LocalStream {
        fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::Unsupported.into())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::ErrorKind::Unsupported.into())
        }
    }

    pub(crate) fn socket_path(data_directory: &Path) -> PathBuf {
        data_directory.join("agent.local-ipc")
    }

    pub(crate) fn bind(_path: &Path) -> Result<LocalListener, TransportError> {
        Err(TransportError::InvalidMessage(
            "local IPC backend is not implemented for this platform".into(),
        ))
    }

    pub(crate) fn connect(_path: &Path) -> Result<LocalStream, TransportError> {
        Err(TransportError::AgentUnavailable)
    }
}

pub(crate) use platform::{LocalListener, LocalStream, bind, connect, socket_path};
