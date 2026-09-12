use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{
    ClusterId, ClusterMember, ClusterRole, ClusterStore, TransportError,
    discovery::{DEFAULT_DISCOVERY_PORT, discovery_sender_socket, run_discovery},
    domain::{
        AcceptedJoin, PairingInvitation, PendingJoin, StoredCluster, StoredMember, new_secret,
        now_millis,
    },
    identity::LocalDeviceIdentity,
    local_ipc::{LocalListener, LocalStream, bind as bind_local, socket_path},
    secure::SecurePeerStream,
    store::AgentDirectoryLock,
    wire::{
        ControlResponse, LocalRequest, PROTOCOL_VERSION, RemoteRequest, RequestEnvelope,
        ResponseEnvelope, read_frame, send_request, write_frame,
    },
};

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub data_directory: PathBuf,
    pub listen: SocketAddr,
    pub advertise: Option<SocketAddr>,
    pub discovery_port: Option<u16>,
    pub device_name: Option<String>,
    /// Development fault injection used to prove idempotent join retries.
    #[doc(hidden)]
    pub drop_remote_join_responses: usize,
    /// Development fault injection for a crash after remote acceptance.
    #[doc(hidden)]
    pub fail_join_after_remote_accepts: usize,
}

impl AgentConfig {
    pub fn new(data_directory: impl Into<PathBuf>) -> Self {
        Self {
            data_directory: data_directory.into(),
            listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 37_846),
            advertise: None,
            discovery_port: Some(DEFAULT_DISCOVERY_PORT),
            device_name: None,
            drop_remote_join_responses: 0,
            fail_join_after_remote_accepts: 0,
        }
    }
}

pub struct AgentHandle {
    endpoint: SocketAddr,
    advertised_endpoint: SocketAddr,
    local_socket_path: PathBuf,
    shutdown: Arc<AtomicBool>,
    threads: Vec<JoinHandle<Result<(), TransportError>>>,
    _directory_lock: AgentDirectoryLock,
}

impl AgentHandle {
    pub fn start(config: AgentConfig) -> Result<Self, TransportError> {
        let directory_lock = ClusterStore::acquire_agent_lock(&config.data_directory)?;
        let store = ClusterStore::open(&config.data_directory, config.device_name.as_deref())?;
        let identity = Arc::new(LocalDeviceIdentity::load_or_create(&config.data_directory)?);

        let local_socket_path = socket_path(&config.data_directory);
        let local_listener = bind_local(&local_socket_path)?;
        let remote_listener = TcpListener::bind(config.listen)?;
        remote_listener.set_nonblocking(true)?;
        let endpoint = remote_listener.local_addr()?;
        let advertised_endpoint = advertised_endpoint(config.advertise, endpoint)?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let discovery_socket = config
            .discovery_port
            .map(|_| discovery_sender_socket())
            .transpose()?;
        let dropped_join_responses = Arc::new(AtomicUsize::new(config.drop_remote_join_responses));
        let failed_local_join_commits =
            Arc::new(AtomicUsize::new(config.fail_join_after_remote_accepts));

        let local_store = store.clone();
        let local_identity = Arc::clone(&identity);
        let local_shutdown = Arc::clone(&shutdown);
        let local_thread = thread::spawn(move || {
            run_local_listener(
                local_listener,
                local_store,
                local_identity,
                advertised_endpoint,
                local_shutdown,
                failed_local_join_commits,
            )
        });

        let remote_store = store.clone();
        let remote_identity = identity;
        let remote_shutdown = Arc::clone(&shutdown);
        let remote_thread = thread::spawn(move || {
            run_remote_listener(
                remote_listener,
                remote_store,
                remote_identity,
                remote_shutdown,
                dropped_join_responses,
            )
        });
        let mut threads = vec![local_thread, remote_thread];
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
            endpoint,
            advertised_endpoint,
            local_socket_path,
            shutdown,
            threads,
            _directory_lock: directory_lock,
        })
    }

    pub fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    pub fn advertised_endpoint(&self) -> SocketAddr {
        self.advertised_endpoint
    }

    pub fn wait(mut self) -> Result<(), TransportError> {
        let result = join_threads(&mut self.threads);
        remove_local_socket(&self.local_socket_path);
        result
    }

    pub fn shutdown(mut self) -> Result<(), TransportError> {
        self.shutdown.store(true, Ordering::Relaxed);
        let result = join_threads(&mut self.threads);
        remove_local_socket(&self.local_socket_path);
        result
    }
}

impl Drop for AgentHandle {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = join_threads(&mut self.threads);
        remove_local_socket(&self.local_socket_path);
    }
}

fn run_local_listener(
    listener: LocalListener,
    store: ClusterStore,
    identity: Arc<LocalDeviceIdentity>,
    advertised_endpoint: SocketAddr,
    shutdown: Arc<AtomicBool>,
    failed_local_join_commits: Arc<AtomicUsize>,
) -> Result<(), TransportError> {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = handle_local_connection(
                    stream,
                    &store,
                    &identity,
                    advertised_endpoint,
                    &failed_local_join_commits,
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn handle_local_connection(
    mut stream: LocalStream,
    store: &ClusterStore,
    identity: &LocalDeviceIdentity,
    advertised_endpoint: SocketAddr,
    failed_local_join_commits: &AtomicUsize,
) -> Result<(), TransportError> {
    let response = match read_frame::<RequestEnvelope<LocalRequest>>(&mut stream) {
        Ok(envelope) if envelope.version == PROTOCOL_VERSION => dispatch_local(
            envelope.request,
            store,
            identity,
            advertised_endpoint,
            failed_local_join_commits,
        )
        .unwrap_or_else(error_response),
        Ok(envelope) => unsupported_version(envelope.version),
        Err(error) => error_response(error),
    };
    write_response(&mut stream, response)
}

fn run_remote_listener(
    listener: TcpListener,
    store: ClusterStore,
    identity: Arc<LocalDeviceIdentity>,
    shutdown: Arc<AtomicBool>,
    dropped_join_responses: Arc<AtomicUsize>,
) -> Result<(), TransportError> {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ =
                    handle_remote_connection(stream, &store, &identity, &dropped_join_responses);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn handle_remote_connection(
    stream: TcpStream,
    store: &ClusterStore,
    identity: &LocalDeviceIdentity,
    dropped_join_responses: &AtomicUsize,
) -> Result<(), TransportError> {
    let mut stream = SecurePeerStream::accept(stream, identity)?;
    let peer_device_id = stream.remote_device_id().clone();
    let envelope = match stream.read::<RequestEnvelope<RemoteRequest>>() {
        Ok(envelope) if envelope.version == PROTOCOL_VERSION => envelope,
        Ok(envelope) => {
            return stream.write(&ResponseEnvelope {
                version: PROTOCOL_VERSION,
                response: unsupported_version(envelope.version),
            });
        }
        Err(error) => {
            return stream.write(&ResponseEnvelope {
                version: PROTOCOL_VERSION,
                response: error_response(error),
            });
        }
    };
    let is_join = matches!(envelope.request, RemoteRequest::RemoteJoin { .. });
    let response =
        dispatch_remote(envelope.request, store, &peer_device_id).unwrap_or_else(error_response);
    if is_join
        && matches!(response, ControlResponse::JoinAccepted { .. })
        && dropped_join_responses
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                remaining.checked_sub(1)
            })
            .is_ok()
    {
        return Ok(());
    }
    stream.write(&ResponseEnvelope {
        version: PROTOCOL_VERSION,
        response,
    })
}

fn write_response(
    stream: &mut impl std::io::Write,
    response: ControlResponse,
) -> Result<(), TransportError> {
    write_frame(
        stream,
        &ResponseEnvelope {
            version: PROTOCOL_VERSION,
            response,
        },
    )
}

fn dispatch_local(
    request: LocalRequest,
    store: &ClusterStore,
    identity: &LocalDeviceIdentity,
    advertised_endpoint: SocketAddr,
    failed_local_join_commits: &AtomicUsize,
) -> Result<ControlResponse, TransportError> {
    match request {
        LocalRequest::LocalStatus => Ok(ControlResponse::Status {
            status: store.load()?.status(),
        }),
        LocalRequest::CreateCluster { name } => create_cluster(store, advertised_endpoint, name),
        LocalRequest::CreateInvite => create_invite(store),
        LocalRequest::JoinCluster {
            coordinator,
            invite_token,
        } => join_cluster(
            store,
            identity,
            advertised_endpoint,
            coordinator,
            invite_token,
            failed_local_join_commits,
        ),
        LocalRequest::LocalMembers => local_members(store, identity),
    }
}

fn dispatch_remote(
    request: RemoteRequest,
    store: &ClusterStore,
    peer_device_id: &kubeweft_model::DeviceId,
) -> Result<ControlResponse, TransportError> {
    match request {
        RemoteRequest::RemoteJoin {
            invite_token,
            member,
            join_request_id,
        } => accept_member(store, invite_token, member, join_request_id, peer_device_id),
        RemoteRequest::RemoteMembers {
            cluster_id,
            device_id,
            credential,
        } => remote_members(store, cluster_id, &device_id, &credential, peer_device_id),
    }
}

fn create_cluster(
    store: &ClusterStore,
    endpoint: SocketAddr,
    name: String,
) -> Result<ControlResponse, TransportError> {
    let name = validate_cluster_name(name)?;
    let own_credential = new_secret();
    let (cluster, invite_token) = store.update(|state| {
        if state.cluster.is_some() {
            return Err(TransportError::AlreadyInCluster);
        }
        if state.pending_join.is_some() {
            return Err(TransportError::Conflict);
        }
        let cluster_id = ClusterId::new();
        let invite_token = PairingInvitation::new(cluster_id, state.device_id.clone()).encode()?;
        let cluster = StoredCluster {
            id: cluster_id,
            name: name.clone(),
            coordinator: endpoint,
            coordinator_device_id: state.device_id.clone(),
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
            accepted_joins: Vec::new(),
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
    let invite_token = store.update(|state| {
        let cluster = state.cluster.as_mut().ok_or(TransportError::NotInCluster)?;
        if cluster.role != ClusterRole::Coordinator {
            return Err(TransportError::NotCoordinator);
        }
        let token =
            PairingInvitation::new(cluster.id, cluster.coordinator_device_id.clone()).encode()?;
        cluster.invite_token = Some(token.clone());
        Ok(token)
    })?;
    Ok(ControlResponse::InviteCreated { invite_token })
}

fn join_cluster(
    store: &ClusterStore,
    identity: &LocalDeviceIdentity,
    endpoint: SocketAddr,
    coordinator: SocketAddr,
    invite_token: String,
    fail_after_remote_accepts: &AtomicUsize,
) -> Result<ControlResponse, TransportError> {
    let invitation = PairingInvitation::decode(&invite_token)?;
    let existing = store.load()?;
    if let Some(cluster) = existing.cluster {
        if cluster.coordinator == coordinator
            && cluster.coordinator_device_id == invitation.coordinator_device_id
            && cluster.id == invitation.cluster_id
        {
            return Ok(ControlResponse::Joined {
                cluster: cluster.summary(),
            });
        }
        return Err(TransportError::AlreadyInCluster);
    }

    let pending = store.update(|state| {
        if state.cluster.is_some() {
            return Err(TransportError::AlreadyInCluster);
        }
        match &state.pending_join {
            Some(pending)
                if pending.coordinator == coordinator && pending.invite_token == invite_token =>
            {
                Ok(pending.clone())
            }
            Some(_) => Err(TransportError::Conflict),
            None => {
                let pending = PendingJoin {
                    coordinator,
                    invite_token: invite_token.clone(),
                    request_id: crate::JoinRequestId::new(),
                    cluster_id: invitation.cluster_id,
                    coordinator_device_id: invitation.coordinator_device_id.clone(),
                };
                state.pending_join = Some(pending.clone());
                Ok(pending)
            }
        }
    })?;
    let state = store.load()?;
    let member = ClusterMember {
        device_id: state.device_id.clone(),
        device_name: state.device_name.clone(),
        endpoint,
        joined_at_millis: now_millis(),
    };
    let response = match send_request(
        coordinator,
        identity,
        &pending.coordinator_device_id,
        RemoteRequest::RemoteJoin {
            invite_token: pending.invite_token.clone(),
            member,
            join_request_id: pending.request_id,
        },
    ) {
        Ok(response) => response,
        Err(
            error @ (TransportError::Remote { .. } | TransportError::PeerIdentityMismatch { .. }),
        ) => {
            store.update(|state| {
                if state
                    .pending_join
                    .as_ref()
                    .is_some_and(|current| current.request_id == pending.request_id)
                {
                    state.pending_join = None;
                }
                Ok(())
            })?;
            return Err(error);
        }
        Err(error) => return Err(error),
    };
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
    if cluster.id != pending.cluster_id
        || cluster.coordinator_device_id != pending.coordinator_device_id
    {
        return Err(TransportError::PeerIdentityMismatch {
            expected: pending.coordinator_device_id,
            actual: cluster.coordinator_device_id,
        });
    }
    if fail_after_remote_accepts
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
            remaining.checked_sub(1)
        })
        .is_ok()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionAborted,
            "injected failure after coordinator accepted join",
        )
        .into());
    }
    let saved_cluster = cluster.clone();
    store.update(|state| {
        if let Some(existing) = &state.cluster {
            if existing.id == saved_cluster.id {
                state.pending_join = None;
                return Ok(());
            }
            return Err(TransportError::AlreadyInCluster);
        }
        if state
            .pending_join
            .as_ref()
            .is_none_or(|current| current.request_id != pending.request_id)
        {
            return Err(TransportError::Conflict);
        }
        state.cluster = Some(StoredCluster {
            id: saved_cluster.id,
            name: saved_cluster.name.clone(),
            coordinator: saved_cluster.coordinator,
            coordinator_device_id: saved_cluster.coordinator_device_id.clone(),
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
            accepted_joins: Vec::new(),
        });
        state.pending_join = None;
        Ok(())
    })?;
    Ok(ControlResponse::Joined { cluster })
}

fn accept_member(
    store: &ClusterStore,
    invite_token: String,
    member: ClusterMember,
    join_request_id: crate::JoinRequestId,
    peer_device_id: &kubeweft_model::DeviceId,
) -> Result<ControlResponse, TransportError> {
    validate_member(&member)?;
    if member.device_id != *peer_device_id {
        return Err(TransportError::Unauthorized);
    }
    let credential = new_secret();
    let (cluster, credential, members) = store.update(|state| {
        let cluster = state.cluster.as_mut().ok_or(TransportError::NotInCluster)?;
        if cluster.role != ClusterRole::Coordinator {
            return Err(TransportError::NotCoordinator);
        }
        if let Some(accepted) = cluster
            .accepted_joins
            .iter()
            .find(|accepted| accepted.request_id == join_request_id)
        {
            if accepted.device_id != member.device_id {
                return Err(TransportError::InvalidInvite);
            }
            return Ok((
                cluster.summary(),
                accepted.credential.clone(),
                cluster.public_members(),
            ));
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
        cluster.accepted_joins.push(AcceptedJoin {
            request_id: join_request_id,
            device_id: member.device_id,
            credential: credential.clone(),
        });
        cluster.invite_token = None;
        Ok((
            cluster.summary(),
            credential.clone(),
            cluster.public_members(),
        ))
    })?;
    Ok(ControlResponse::JoinAccepted {
        cluster,
        credential,
        members,
    })
}

fn local_members(
    store: &ClusterStore,
    identity: &LocalDeviceIdentity,
) -> Result<ControlResponse, TransportError> {
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
        identity,
        &cluster.coordinator_device_id,
        RemoteRequest::RemoteMembers {
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
    peer_device_id: &kubeweft_model::DeviceId,
) -> Result<ControlResponse, TransportError> {
    let state = store.load()?;
    let cluster = state.cluster.ok_or(TransportError::NotInCluster)?;
    if cluster.role != ClusterRole::Coordinator {
        return Err(TransportError::NotCoordinator);
    }
    if device_id != peer_device_id
        || cluster.id != cluster_id
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

fn unsupported_version(version: u16) -> ControlResponse {
    ControlResponse::Error {
        code: "unsupported_version".into(),
        message: format!("unsupported protocol version {version}"),
    }
}

fn error_response(error: TransportError) -> ControlResponse {
    if let TransportError::Remote { code, message } = error {
        return ControlResponse::Error { code, message };
    }
    let message = error.to_string();
    let code = match error {
        TransportError::AgentUnavailable => "agent_unavailable",
        TransportError::AgentAlreadyRunning => "agent_already_running",
        TransportError::InvalidIdentity(_) => "invalid_identity",
        TransportError::SecureChannel(_) => "secure_channel",
        TransportError::PeerIdentityMismatch { .. } => "peer_identity_mismatch",
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

fn remove_local_socket(path: &std::path::Path) {
    if let Err(error) = fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!(
            "failed to remove local control socket {}: {error}",
            path.display()
        );
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
