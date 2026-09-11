use std::{fmt, io};

/// Errors with stable semantic categories for clients and future protocols.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesystemError {
    NotFound,
    AlreadyExists,
    NotDirectory,
    IsDirectory,
    DirectoryNotEmpty,
    Conflict,
    ContentUnavailable,
    NoSpace,
    InvalidPath,
    NodeNotFound,
    MetadataUnavailable(String),
    Storage(String),
}

impl fmt::Display for FilesystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("path not found"),
            Self::AlreadyExists => formatter.write_str("path already exists"),
            Self::NotDirectory => formatter.write_str("path component is not a directory"),
            Self::IsDirectory => formatter.write_str("operation requires a file"),
            Self::DirectoryNotEmpty => formatter.write_str("directory is not empty"),
            Self::Conflict => formatter.write_str("file generation conflict"),
            Self::ContentUnavailable => {
                formatter.write_str("file exists, but its content is currently unavailable")
            }
            Self::NoSpace => formatter.write_str("storage node has insufficient free space"),
            Self::InvalidPath => formatter.write_str("invalid absolute filesystem path"),
            Self::NodeNotFound => formatter.write_str("storage node not found"),
            Self::MetadataUnavailable(message) => {
                write!(formatter, "metadata unavailable: {message}")
            }
            Self::Storage(message) => write!(formatter, "content storage error: {message}"),
        }
    }
}

impl std::error::Error for FilesystemError {}

impl From<io::Error> for FilesystemError {
    fn from(error: io::Error) -> Self {
        Self::Storage(error.to_string())
    }
}
