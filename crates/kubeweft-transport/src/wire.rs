use std::{
    io::{Read, Write},
    net::SocketAddr,
};

use kubeweft_model::DeviceId;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    ClusterId, ClusterMember, ClusterSummary, JoinRequestId, MembershipStatus, TransportError,
    identity::LocalDeviceIdentity, secure::SecurePeerStream,
};

pub(crate) const PROTOCOL_VERSION: u16 = 3;
const MAX_FRAME_SIZE: usize = 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RequestEnvelope<T> {
    pub version: u16,
    pub request: T,
}

/// Private development-only local IPC messages. These are not a public wire contract.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum LocalRequest {
    LocalStatus,
    CreateCluster {
        name: String,
    },
    CreateInvite,
    JoinCluster {
        coordinator: SocketAddr,
        invite_token: String,
    },
    LocalMembers,
}

/// Private development-only peer messages. Cross-language semantics live in proto.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum RemoteRequest {
    RemoteJoin {
        invite_token: String,
        member: ClusterMember,
        join_request_id: JoinRequestId,
    },
    RemoteMembers {
        cluster_id: ClusterId,
        device_id: DeviceId,
        credential: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ResponseEnvelope {
    pub version: u16,
    pub response: ControlResponse,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ControlResponse {
    Status {
        status: MembershipStatus,
    },
    ClusterCreated {
        cluster: ClusterSummary,
        invite_token: String,
    },
    InviteCreated {
        invite_token: String,
    },
    Joined {
        cluster: ClusterSummary,
    },
    JoinAccepted {
        cluster: ClusterSummary,
        credential: String,
        members: Vec<ClusterMember>,
    },
    Members {
        cluster: ClusterSummary,
        members: Vec<ClusterMember>,
    },
    Error {
        code: String,
        message: String,
    },
}

pub(crate) fn send_request(
    endpoint: SocketAddr,
    identity: &LocalDeviceIdentity,
    expected_remote: &DeviceId,
    request: RemoteRequest,
) -> Result<ControlResponse, TransportError> {
    let mut stream = SecurePeerStream::connect(endpoint, identity, expected_remote)?;
    stream.write(&RequestEnvelope {
        version: PROTOCOL_VERSION,
        request,
    })?;
    let response: ResponseEnvelope = stream.read()?;
    if response.version != PROTOCOL_VERSION {
        return Err(TransportError::InvalidMessage(format!(
            "unsupported response protocol version {}",
            response.version
        )));
    }
    match response.response {
        ControlResponse::Error { code, message } => Err(TransportError::Remote { code, message }),
        response => Ok(response),
    }
}

pub(crate) fn read_frame<T: DeserializeOwned>(stream: &mut impl Read) -> Result<T, TransportError> {
    let mut size = [0_u8; 4];
    stream.read_exact(&mut size)?;
    let size = usize::try_from(u32::from_be_bytes(size))
        .map_err(|_| TransportError::InvalidMessage("invalid frame size".into()))?;
    if size == 0 || size > MAX_FRAME_SIZE {
        return Err(TransportError::InvalidMessage(format!(
            "frame size {size} is outside the accepted range"
        )));
    }
    let mut bytes = vec![0; size];
    stream.read_exact(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub(crate) fn write_frame<T: Serialize>(
    stream: &mut impl Write,
    value: &T,
) -> Result<(), TransportError> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_FRAME_SIZE {
        return Err(TransportError::InvalidMessage("frame is too large".into()));
    }
    let size = u32::try_from(bytes.len())
        .map_err(|_| TransportError::InvalidMessage("frame is too large".into()))?;
    stream.write_all(&size.to_be_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}
