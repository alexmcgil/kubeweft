use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{
    ClusterId, ClusterMember, ClusterRole, ClusterStore, TransportError,
    discovery::{DEFAULT_DISCOVERY_PORT, discovery_sender_socket, run_discovery},
    domain::{StoredCluster, StoredMember, new_secret, now_millis},
    wire::{
        ControlRequest, ControlResponse, PROTOCOL_VERSION, RequestEnvelope, ResponseEnvelope,
        read_frame, send_request, write_frame,
    },
};

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub data_directory: PathBuf,
    pub listen: SocketAddr,
    pub advertise: Option<SocketAddr>,
    pub discovery_port: Option<u16>,
    pub device_name: Option<String>,
}

impl AgentConfig {
    pub fn new(data_directory: impl Into<PathBuf>) -> Self {
        Self {
            data_directory: data_directory.into(),
            listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 37_846),
            advertise: None,
            discovery_port: Some(DEFAULT_DISCOVERY_PORT),
            device_name: None,
        }
    }
}

pub struct AgentHandle {
    endpoint: SocketAddr,
    advertised_endpoint: SocketAddr,
    shutdown: Arc<AtomicBool>,
    threads: Vec<JoinHandle<Result<(), TransportError>>>,
}

impl AgentHandle {
    pub fn start(config: AgentConfig) -> Result<Self, TransportError> {
        let store = ClusterStore::open(&config.data_directory, config.device_name.as_deref())?;
        let listener = TcpListener::bind(config.listen)?;
        listener.set_nonblocking(true)?;
        let endpoint = listener.local_addr()?;
        let local_endpoint = loopback_endpoint(endpoint);
        store.write_runtime_endpoint(local_endpoint)?;
        let advertised_endpoint = advertised_endpoint(config.advertise, endpoint)?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let discovery_socket = config
            .discovery_port
            .map(|_| discovery_sender_socket())
            .transpose()?;

        let tcp_store = store.clone();
        let tcp_shutdown = Arc::clone(&shutdown);
        let tcp_thread = thread::spawn(move || {
            run_control_listener(listener, tcp_store, advertised_endpoint, tcp_shutdown)
        });
        let mut threads = vec![tcp_thread];
        if let (Some(discovery_port), Some(discovery_socket)) =
            (config.discovery_port, discovery_socket)
        {
            let discovery_store = store;
            let discovery_shutdown = Arc::clone(&shutdown);
            threads.push(thread::spawn(move || {
                run_discovery(
                    discovery_socket,
                    discovery_store,
                    advertised_endpoint,
                    discovery_port,
                    discovery_shutdown,
                )
            }));
        }

        Ok(Self {
            endpoint: local_endpoint,
            advertised_endpoint,
            shutdown,
            threads,
        })
    }

    pub fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    pub fn advertised_endpoint(&self) -> SocketAddr {
        self.advertised_endpoint
    }

    pub fn wait(mut self) -> Result<(), TransportError> {
        join_threads(&mut self.threads)
    }

    pub fn shutdown(mut self) -> Result<(), TransportError> {
        self.shutdown.store(true, Ordering::Relaxed);
        join_threads(&mut self.threads)
    }
}

impl Drop for AgentHandle {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

fn run_control_listener(
    listener: TcpListener,
    store: ClusterStore,
    advertised_endpoint: SocketAddr,
    shutdown: Arc<AtomicBool>,
) -> Result<(), TransportError> {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, peer)) => {
                let _ = handle_connection(stream, peer, &store, advertised_endpoint);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn handle_connection(
    mut stream: TcpStream,
    peer: SocketAddr,
    store: &ClusterStore,
    advertised_endpoint: SocketAddr,
) -> Result<(), TransportError> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let response = match read_frame::<RequestEnvelope>(&mut stream) {
        Ok(envelope) if envelope.version == PROTOCOL_VERSION => {
            dispatch(envelope.request, peer, store, advertised_endpoint)
                .unwrap_or_else(error_response)
        }
        Ok(envelope) => ControlResponse::Error {
            code: "unsupported_version".into(),
            message: format!("unsupported protocol version {}", envelope.version),
        },
        Err(error) => error_response(error),
    };
    write_frame(
        &mut stream,
        &ResponseEnvelope {
            version: PROTOCOL_VERSION,
            response,
        },
    )
}

fn dispatch(
    request: ControlRequest,
    peer: SocketAddr,
    store: &ClusterStore,
    advertised_endpoint: SocketAddr,
) -> Result<ControlResponse, TransportError> {
    match request {
        ControlRequest::Ping => Ok(ControlResponse::Pong {
            status: store.load()?.status(),
        }),
        ControlRequest::LocalStatus => {
            require_loopback(peer)?;
            Ok(ControlResponse::Status {
                status: store.load()?.status(),
            })
        }
        ControlRequest::CreateCluster { name } => {
            require_loopback(peer)?;
            create_cluster(store, advertised_endpoint, name)
        }
        ControlRequest::CreateInvite => {
            require_loopback(peer)?;
            create_invite(store)
        }
        ControlRequest::JoinCluster {
            coordinator,
            invite_token,
        } => {
            require_loopback(peer)?;
            join_cluster(store, advertised_endpoint, coordinator, invite_token)
        }
        ControlRequest::LocalMembers => {
            require_loopback(peer)?;
            local_members(store)
        }
        ControlRequest::RemoteJoin {
            invite_token,
            member,
        } => accept_member(store, invite_token, member),
        ControlRequest::RemoteMembers {
            cluster_id,
            device_id,
            credential,
        } => remote_members(store, cluster_id, &device_id, &credential),
    }
}

fn create_cluster(
    store: &ClusterStore,
    endpoint: SocketAddr,
    name: String,
) -> Result<ControlResponse, TransportError> {
    let name = validate_cluster_name(name)?;
    let invite_token = new_secret();
    let own_credential = new_secret();
    let (cluster, invite_token) = store.update(|state| {
        if state.cluster.is_some() {
            return Err(TransportError::AlreadyInCluster);
        }
        let cluster = StoredCluster {
            id: ClusterId::new(),
            name: name.clone(),
            coordinator: endpoint,
            role: ClusterRole::Coordinator,
            credential: own_credential.clone(),
            invite_token: Some(invite_token.clone()),
            members: vec![StoredMember {
                member: ClusterMember {
                    device_id: state.device_id.clone(),
                    device_name: state.device_name.clone(),
                    endpoint,
                    joined_at_millis: now_millis(),
                },
                credential: own_credential.clone(),
            }],
        };
        let summary = cluster.summary();
        state.cluster = Some(cluster);
        Ok((summary, invite_token.clone()))
    })?;
    Ok(ControlResponse::ClusterCreated {
        cluster,
        invite_token,
    })
}

fn create_invite(store: &ClusterStore) -> Result<ControlResponse, TransportError> {
    let token = new_secret();
    let invite_token = store.update(|state| {
        let cluster = state.cluster.as_mut().ok_or(TransportError::NotInCluster)?;
        if cluster.role != ClusterRole::Coordinator {
            return Err(TransportError::NotCoordinator);
        }
        cluster.invite_token = Some(token.clone());
        Ok(token.clone())
    })?;
    Ok(ControlResponse::InviteCreated { invite_token })
}

fn join_cluster(
    store: &ClusterStore,
    endpoint: SocketAddr,
    coordinator: SocketAddr,
    invite_token: String,
) -> Result<ControlResponse, TransportError> {
    let state = store.load()?;
    if state.cluster.is_some() {
        return Err(TransportError::AlreadyInCluster);
    }
    let member = ClusterMember {
        device_id: state.device_id.clone(),
        device_name: state.device_name.clone(),
        endpoint,
        joined_at_millis: now_millis(),
    };
    let response = send_request(
        coordinator,
        ControlRequest::RemoteJoin {
            invite_token,
            member,
        },
    )?;
    let ControlResponse::JoinAccepted {
        cluster,
        credential,
        members,
    } = response
    else {
        return Err(TransportError::InvalidMessage(
            "coordinator returned an unexpected join response".into(),
        ));
    };
    let saved_cluster = cluster.clone();
    store.update(|state| {
        if state.cluster.is_some() {
            return Err(TransportError::AlreadyInCluster);
        }
        state.cluster = Some(StoredCluster {
            id: saved_cluster.id,
            name: saved_cluster.name.clone(),
            coordinator: saved_cluster.coordinator,
            role: ClusterRole::Member,
            credential: credential.clone(),
            invite_token: None,
            members: members
                .iter()
                .cloned()
                .map(|member| StoredMember {
                    member,
                    credential: String::new(),
                })
                .collect(),
        });
        Ok(())
    })?;
    Ok(ControlResponse::Joined { cluster })
}

fn accept_member(
    store: &ClusterStore,
    invite_token: String,
    member: ClusterMember,
) -> Result<ControlResponse, TransportError> {
    validate_member(&member)?;
    let credential = new_secret();
    let (cluster, members) = store.update(|state| {
        let cluster = state.cluster.as_mut().ok_or(TransportError::NotInCluster)?;
        if cluster.role != ClusterRole::Coordinator {
            return Err(TransportError::NotCoordinator);
        }
        if cluster.invite_token.as_deref() != Some(invite_token.as_str()) {
            return Err(TransportError::InvalidInvite);
        }
        if cluster
            .members
            .iter()
            .any(|existing| existing.member.device_id == member.device_id)
        {
            return Err(TransportError::AlreadyInCluster);
        }
        cluster.members.push(StoredMember {
            member: member.clone(),
            credential: credential.clone(),
        });
        cluster.invite_token = None;
        Ok((cluster.summary(), cluster.public_members()))
    })?;
    Ok(ControlResponse::JoinAccepted {
        cluster,
        credential,
        members,
    })
}

fn local_members(store: &ClusterStore) -> Result<ControlResponse, TransportError> {
    let state = store.load()?;
    let cluster = state.cluster.ok_or(TransportError::NotInCluster)?;
    if cluster.role == ClusterRole::Coordinator {
        return Ok(ControlResponse::Members {
            cluster: cluster.summary(),
            members: cluster.public_members(),
        });
    }
    send_request(
        cluster.coordinator,
        ControlRequest::RemoteMembers {
            cluster_id: cluster.id,
            device_id: state.device_id,
            credential: cluster.credential,
        },
    )
}

fn remote_members(
    store: &ClusterStore,
    cluster_id: ClusterId,
    device_id: &kubeweft_model::DeviceId,
    credential: &str,
) -> Result<ControlResponse, TransportError> {
    let state = store.load()?;
    let cluster = state.cluster.ok_or(TransportError::NotInCluster)?;
    if cluster.role != ClusterRole::Coordinator {
        return Err(TransportError::NotCoordinator);
    }
    if cluster.id != cluster_id
        || !cluster
            .members
            .iter()
            .any(|member| member.member.device_id == *device_id && member.credential == credential)
    {
        return Err(TransportError::Unauthorized);
    }
    Ok(ControlResponse::Members {
        cluster: cluster.summary(),
        members: cluster.public_members(),
    })
}

fn require_loopback(peer: SocketAddr) -> Result<(), TransportError> {
    if peer.ip().is_loopback() {
        Ok(())
    } else {
        Err(TransportError::Unauthorized)
    }
}

fn validate_cluster_name(name: String) -> Result<String, TransportError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err(TransportError::InvalidMessage(
            "cluster name must be 1-128 printable characters".into(),
        ));
    }
    Ok(name.to_owned())
}

fn validate_member(member: &ClusterMember) -> Result<(), TransportError> {
    if member.device_name.trim().is_empty()
        || member.device_name.len() > 128
        || member.device_name.chars().any(char::is_control)
    {
        return Err(TransportError::InvalidMessage(
            "device name must be 1-128 printable characters".into(),
        ));
    }
    if member.endpoint.ip().is_unspecified() || member.endpoint.port() == 0 {
        return Err(TransportError::InvalidMessage(
            "member endpoint must be reachable".into(),
        ));
    }
    Ok(())
}

fn error_response(error: TransportError) -> ControlResponse {
    if let TransportError::Remote { code, message } = error {
        return ControlResponse::Error { code, message };
    }
    let message = error.to_string();
    let code = match error {
        TransportError::AgentUnavailable => "agent_unavailable",
        TransportError::NotCoordinator => "not_coordinator",
        TransportError::AlreadyInCluster => "already_in_cluster",
        TransportError::NotInCluster => "not_in_cluster",
        TransportError::InvalidInvite => "invalid_invite",
        TransportError::Unauthorized => "unauthorized",
        TransportError::Conflict => "conflict",
        TransportError::Io(_) => "io",
        TransportError::InvalidMessage(_) => "invalid_message",
        TransportError::Remote { .. } => unreachable!(),
    };
    ControlResponse::Error {
        code: code.into(),
        message,
    }
}

fn advertised_endpoint(
    configured: Option<SocketAddr>,
    bound: SocketAddr,
) -> Result<SocketAddr, TransportError> {
    let mut endpoint = configured.unwrap_or_else(|| loopback_endpoint(bound));
    if endpoint.ip().is_unspecified() {
        return Err(TransportError::InvalidMessage(
            "advertised endpoint cannot use an unspecified address".into(),
        ));
    }
    if endpoint.port() == 0 {
        endpoint.set_port(bound.port());
    }
    Ok(endpoint)
}

fn loopback_endpoint(endpoint: SocketAddr) -> SocketAddr {
    if endpoint.ip().is_unspecified() {
        match endpoint {
            SocketAddr::V4(address) => SocketAddr::new(Ipv4Addr::LOCALHOST.into(), address.port()),
            SocketAddr::V6(address) => {
                SocketAddr::new(std::net::Ipv6Addr::LOCALHOST.into(), address.port())
            }
        }
    } else {
        endpoint
    }
}

fn join_threads(
    threads: &mut Vec<JoinHandle<Result<(), TransportError>>>,
) -> Result<(), TransportError> {
    for thread in threads.drain(..) {
        thread
            .join()
            .map_err(|_| TransportError::InvalidMessage("agent thread panicked".into()))??;
    }
    Ok(())
}
