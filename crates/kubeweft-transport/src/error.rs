use std::fmt;

#[derive(Debug)]
pub enum TransportError {
    Io(std::io::Error),
    InvalidMessage(String),
    Remote { code: String, message: String },
    AgentUnavailable,
    NotCoordinator,
    AlreadyInCluster,
    NotInCluster,
    InvalidInvite,
    Unauthorized,
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
            Self::NotCoordinator => {
                formatter.write_str("this device is not the cluster coordinator")
            }
            Self::AlreadyInCluster => formatter.write_str("device already belongs to a cluster"),
            Self::NotInCluster => formatter.write_str("device does not belong to a cluster"),
            Self::InvalidInvite => formatter.write_str("invalid or expired cluster invite"),
            Self::Unauthorized => formatter.write_str("cluster member authentication failed"),
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
