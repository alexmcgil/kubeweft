use std::{net::SocketAddr, path::Path};

use crate::{
    ClusterMember, ClusterStore, ClusterSummary, MembershipStatus, TransportError,
    wire::{ControlRequest, ControlResponse, send_request},
};

#[derive(Debug, Clone, Copy)]
pub struct ControlClient {
    endpoint: SocketAddr,
}

impl ControlClient {
    pub fn new(endpoint: SocketAddr) -> Self {
        Self { endpoint }
    }

    pub fn local(data_directory: impl AsRef<Path>) -> Result<Self, TransportError> {
        let store = ClusterStore::open(data_directory, None)?;
        Ok(Self::new(store.read_runtime_endpoint()?))
    }

    pub fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    pub fn ping(&self) -> Result<MembershipStatus, TransportError> {
        match send_request(self.endpoint, ControlRequest::Ping)? {
            ControlResponse::Pong { status } => Ok(status),
            response => unexpected(response),
        }
    }

    pub fn status(&self) -> Result<MembershipStatus, TransportError> {
        match send_request(self.endpoint, ControlRequest::LocalStatus)? {
            ControlResponse::Status { status } => Ok(status),
            response => unexpected(response),
        }
    }

    pub fn create_cluster(
        &self,
        name: impl Into<String>,
    ) -> Result<(ClusterSummary, String), TransportError> {
        match send_request(
            self.endpoint,
            ControlRequest::CreateCluster { name: name.into() },
        )? {
            ControlResponse::ClusterCreated {
                cluster,
                invite_token,
            } => Ok((cluster, invite_token)),
            response => unexpected(response),
        }
    }

    pub fn create_invite(&self) -> Result<String, TransportError> {
        match send_request(self.endpoint, ControlRequest::CreateInvite)? {
            ControlResponse::InviteCreated { invite_token } => Ok(invite_token),
            response => unexpected(response),
        }
    }

    pub fn join_cluster(
        &self,
        coordinator: SocketAddr,
        invite_token: impl Into<String>,
    ) -> Result<ClusterSummary, TransportError> {
        match send_request(
            self.endpoint,
            ControlRequest::JoinCluster {
                coordinator,
                invite_token: invite_token.into(),
            },
        )? {
            ControlResponse::Joined { cluster } => Ok(cluster),
            response => unexpected(response),
        }
    }

    pub fn members(&self) -> Result<(ClusterSummary, Vec<ClusterMember>), TransportError> {
        match send_request(self.endpoint, ControlRequest::LocalMembers)? {
            ControlResponse::Members { cluster, members } => Ok((cluster, members)),
            response => unexpected(response),
        }
    }
}

fn unexpected<T>(response: ControlResponse) -> Result<T, TransportError> {
    Err(TransportError::InvalidMessage(format!(
        "unexpected response: {response:?}"
    )))
}
