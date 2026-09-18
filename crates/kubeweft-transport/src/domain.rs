use std::{
    fmt,
    net::SocketAddr,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use kubeweft_model::DeviceId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ClusterId(Uuid);

impl ClusterId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct JoinRequestId(Uuid);

impl JoinRequestId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for JoinRequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for JoinRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Default for ClusterId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ClusterId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for ClusterId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterRole {
    Coordinator,
    Member,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterMember {
    pub device_id: DeviceId,
    pub device_name: String,
    pub endpoint: SocketAddr,
    pub joined_at_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceState {
    Unknown,
    Online,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresenceRecord {
    pub member: ClusterMember,
    pub state: PresenceState,
    pub observed_at_millis: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterSummary {
    pub id: ClusterId,
    pub name: String,
    pub coordinator: SocketAddr,
    pub coordinator_device_id: DeviceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipStatus {
    pub device_id: DeviceId,
    pub device_name: String,
    pub cluster: Option<ClusterSummary>,
    pub role: Option<ClusterRole>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub(crate) revision: u64,
    pub(crate) device_id: DeviceId,
    pub(crate) device_name: String,
    pub(crate) cluster: Option<StoredCluster>,
    #[serde(default)]
    pub(crate) pending_join: Option<PendingJoin>,
}

impl AgentState {
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn status(&self) -> MembershipStatus {
        MembershipStatus {
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            cluster: self.cluster.as_ref().map(StoredCluster::summary),
            role: self.cluster.as_ref().map(|cluster| cluster.role),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredCluster {
    pub id: ClusterId,
    pub name: String,
    pub coordinator: SocketAddr,
    pub coordinator_device_id: DeviceId,
    pub role: ClusterRole,
    pub credential: String,
    pub invite_token: Option<String>,
    pub members: Vec<StoredMember>,
    #[serde(default)]
    pub accepted_joins: Vec<AcceptedJoin>,
}

impl StoredCluster {
    pub fn summary(&self) -> ClusterSummary {
        ClusterSummary {
            id: self.id,
            name: self.name.clone(),
            coordinator: self.coordinator,
            coordinator_device_id: self.coordinator_device_id.clone(),
        }
    }

    pub fn public_members(&self) -> Vec<ClusterMember> {
        self.members
            .iter()
            .map(|member| member.member.clone())
            .collect()
    }

    pub fn authorized_members(&self) -> Vec<AuthorizedMember> {
        self.members
            .iter()
            .map(|member| AuthorizedMember {
                member: member.member.clone(),
                credential_digest: if member.credential_digest.is_empty() {
                    credential_digest(&member.credential)
                } else {
                    member.credential_digest.clone()
                },
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredMember {
    pub member: ClusterMember,
    pub credential: String,
    #[serde(default)]
    pub credential_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AuthorizedMember {
    pub member: ClusterMember,
    pub credential_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PendingJoin {
    pub coordinator: SocketAddr,
    pub invite_token: String,
    pub request_id: JoinRequestId,
    pub cluster_id: ClusterId,
    pub coordinator_device_id: DeviceId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AcceptedJoin {
    pub request_id: JoinRequestId,
    pub device_id: DeviceId,
    pub credential: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PairingInvitation {
    version: u8,
    pub cluster_id: ClusterId,
    pub coordinator_device_id: DeviceId,
    secret: String,
}

impl PairingInvitation {
    const PREFIX: &'static str = "kwpair1.";

    pub fn new(cluster_id: ClusterId, coordinator_device_id: DeviceId) -> Self {
        Self {
            version: 1,
            cluster_id,
            coordinator_device_id,
            secret: new_secret(),
        }
    }

    pub fn encode(&self) -> Result<String, crate::TransportError> {
        let payload = serde_json::to_vec(self)?;
        Ok(format!(
            "{}{}",
            Self::PREFIX,
            URL_SAFE_NO_PAD.encode(payload)
        ))
    }

    pub fn decode(encoded: &str) -> Result<Self, crate::TransportError> {
        let payload = encoded
            .strip_prefix(Self::PREFIX)
            .ok_or(crate::TransportError::InvalidInvite)?;
        let bytes = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| crate::TransportError::InvalidInvite)?;
        let invitation: Self =
            serde_json::from_slice(&bytes).map_err(|_| crate::TransportError::InvalidInvite)?;
        if invitation.version != 1 || invitation.secret.len() < 32 {
            return Err(crate::TransportError::InvalidInvite);
        }
        Ok(invitation)
    }
}

pub(crate) fn new_secret() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub(crate) fn credential_digest(credential: &str) -> String {
    let digest = Sha256::new()
        .chain_update(b"kubeweft-membership-credential-v1\0")
        .chain_update(credential.as_bytes())
        .finalize();
    format!("sha256:{digest:x}")
}

pub(crate) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
