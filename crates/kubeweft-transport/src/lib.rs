//! Development control-plane transport for Kubeweft agents.
//!
//! The crate owns LAN discovery, framed TCP requests, and persistent cluster
//! membership. Domain and filesystem crates remain independent from networking.

mod agent;
mod client;
mod discovery;
mod domain;
mod error;
mod identity;
mod local_ipc;
mod secure;
mod store;
mod wire;

pub use agent::{AgentConfig, AgentHandle};
pub use client::LocalControlClient;
pub use discovery::{DEFAULT_DISCOVERY_PORT, DiscoveryAnnouncement, discover};
pub use domain::{
    AgentState, ClusterId, ClusterMember, ClusterRole, ClusterSummary, JoinRequestId,
    MembershipStatus,
};
pub use error::TransportError;
pub use store::ClusterStore;
