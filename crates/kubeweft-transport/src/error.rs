use std::fmt;

use kubeweft_model::DeviceId;

#[derive(Debug)]
pub enum TransportError {
    Io(std::io::Error),
    InvalidMessage(String),
    Remote {
        code: String,
        message: String,
    },
    AgentUnavailable,
    AgentAlreadyRunning,
    InvalidIdentity(String),
    SecureChannel(String),
    PeerIdentityMismatch {
        expected: DeviceId,
        actual: DeviceId,
    },
    NotCoordinator,
    AlreadyInCluster,
    NotInCluster,
    InvalidInvite,
    Unauthorized,
    ContentUnavailable,
    ContentIntegrity,
    ContentTooLarge,
    NoSpace,
    PeerContentDisabled,
    Conflict,
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::InvalidMessage(message) => {
                write!(formatter, "invalid transport message: {message}")
            }
            Self::Remote { code, message } => write!(formatter, "remote error {code}: {message}"),
            Self::AgentUnavailable => formatter.write_str("local agent is not running"),
            Self::AgentAlreadyRunning => {
                formatter.write_str("another agent is already using this data directory")
            }
            Self::InvalidIdentity(message) => {
                write!(formatter, "invalid device identity: {message}")
            }
            Self::SecureChannel(message) => {
                write!(formatter, "secure peer channel failed: {message}")
            }
            Self::PeerIdentityMismatch { expected, actual } => write!(
                formatter,
                "peer identity mismatch: expected {expected}, received {actual}"
            ),
            Self::NotCoordinator => {
                formatter.write_str("this device is not the cluster coordinator")
            }
            Self::AlreadyInCluster => formatter.write_str("device already belongs to a cluster"),
            Self::NotInCluster => formatter.write_str("device does not belong to a cluster"),
            Self::InvalidInvite => formatter.write_str("invalid or expired cluster invite"),
            Self::Unauthorized => {
                formatter.write_str("peer identity or cluster membership credential is invalid")
            }
            Self::ContentUnavailable => formatter.write_str("peer content is unavailable"),
            Self::ContentIntegrity => {
                formatter.write_str("peer content failed integrity validation")
            }
            Self::ContentTooLarge => formatter.write_str("peer content exceeds the transfer limit"),
            Self::NoSpace => formatter.write_str("peer content store has insufficient space"),
            Self::PeerContentDisabled => formatter.write_str("peer content service is disabled"),
            Self::Conflict => formatter.write_str("cluster state changed concurrently"),
        }
    }
}

impl std::error::Error for TransportError {}

impl From<std::io::Error> for TransportError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for TransportError {
    fn from(error: serde_json::Error) -> Self {
        Self::InvalidMessage(error.to_string())
    }
}
