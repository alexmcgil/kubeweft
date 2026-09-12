use std::path::{Path, PathBuf};

use crate::{
    ClusterMember, ClusterSummary, MembershipStatus, TransportError,
    local_ipc::{connect, socket_path},
    wire::{
        ControlResponse, LocalRequest, PROTOCOL_VERSION, RequestEnvelope, ResponseEnvelope,
        read_frame, write_frame,
    },
};

/// Client for the user-scoped local agent control channel.
#[derive(Debug, Clone)]
pub struct LocalControlClient {
    socket_path: PathBuf,
}

impl LocalControlClient {
    pub fn local(data_directory: impl AsRef<Path>) -> Result<Self, TransportError> {
        Ok(Self {
            socket_path: socket_path(data_directory.as_ref()),
        })
    }

    pub fn status(&self) -> Result<MembershipStatus, TransportError> {
        match self.request(LocalRequest::LocalStatus)? {
            ControlResponse::Status { status } => Ok(status),
            response => unexpected(response),
        }
    }

    pub fn create_cluster(
        &self,
        name: impl Into<String>,
    ) -> Result<(ClusterSummary, String), TransportError> {
        match self.request(LocalRequest::CreateCluster { name: name.into() })? {
            ControlResponse::ClusterCreated {
                cluster,
                invite_token,
            } => Ok((cluster, invite_token)),
            response => unexpected(response),
        }
    }

    pub fn create_invite(&self) -> Result<String, TransportError> {
        match self.request(LocalRequest::CreateInvite)? {
            ControlResponse::InviteCreated { invite_token } => Ok(invite_token),
            response => unexpected(response),
        }
    }

    pub fn join_cluster(
        &self,
        coordinator: std::net::SocketAddr,
        invite_token: impl Into<String>,
    ) -> Result<ClusterSummary, TransportError> {
        match self.request(LocalRequest::JoinCluster {
            coordinator,
            invite_token: invite_token.into(),
        })? {
            ControlResponse::Joined { cluster } => Ok(cluster),
            response => unexpected(response),
        }
    }

    pub fn members(&self) -> Result<(ClusterSummary, Vec<ClusterMember>), TransportError> {
        match self.request(LocalRequest::LocalMembers)? {
            ControlResponse::Members { cluster, members } => Ok((cluster, members)),
            response => unexpected(response),
        }
    }

    fn request(&self, request: LocalRequest) -> Result<ControlResponse, TransportError> {
        let mut stream = connect(&self.socket_path)?;
        write_frame(
            &mut stream,
            &RequestEnvelope {
                version: PROTOCOL_VERSION,
                request,
            },
        )?;
        let response: ResponseEnvelope = read_frame(&mut stream)?;
        if response.version != PROTOCOL_VERSION {
            return Err(TransportError::InvalidMessage(format!(
                "unsupported response protocol version {}",
                response.version
            )));
        }
        match response.response {
            ControlResponse::Error { code, message } => {
                Err(TransportError::Remote { code, message })
            }
            response => Ok(response),
        }
    }
}

fn unexpected<T>(response: ControlResponse) -> Result<T, TransportError> {
    Err(TransportError::InvalidMessage(format!(
        "unexpected response: {response:?}"
    )))
}
