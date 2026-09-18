use std::{path::Path, sync::Arc};

use kubeweft_model::DeviceId;

use crate::{
    ClusterStore, TransportError,
    identity::LocalDeviceIdentity,
    secure::{MAX_CONTENT_TRANSFER_SIZE, SecurePeerStream},
    wire::{
        ControlResponse, PROTOCOL_VERSION, PeerAuthorization, RemoteRequest, RequestEnvelope,
        ResponseEnvelope,
    },
};

/// Storage adapter served to authenticated members over the peer channel.
pub trait PeerContentStore: Send + Sync {
    fn get(&self, content_id: &str) -> Result<Vec<u8>, TransportError>;
    fn put(&self, content_id: &str, data: &[u8]) -> Result<(), TransportError>;
    fn exists(&self, content_id: &str) -> Result<bool, TransportError>;
}

#[derive(Clone)]
pub struct ClusterContentClient {
    store: ClusterStore,
    identity: Arc<LocalDeviceIdentity>,
}

impl ClusterContentClient {
    pub fn local(data_directory: impl AsRef<Path>) -> Result<Self, TransportError> {
        let data_directory = data_directory.as_ref();
        Ok(Self {
            store: ClusterStore::open(data_directory, None)?,
            identity: Arc::new(LocalDeviceIdentity::load_or_create(data_directory)?),
        })
    }

    pub fn exists(&self, target: &DeviceId, content_id: &str) -> Result<bool, TransportError> {
        let (mut stream, authorization) = self.connect(target)?;
        stream.write(&RequestEnvelope {
            version: PROTOCOL_VERSION,
            request: RemoteRequest::ContentExists {
                authorization,
                content_id: content_id.to_owned(),
            },
        })?;
        match read_response(&mut stream)? {
            ControlResponse::ContentExists { exists } => Ok(exists),
            response => unexpected(response),
        }
    }

    pub fn get(&self, target: &DeviceId, content_id: &str) -> Result<Vec<u8>, TransportError> {
        let (mut stream, authorization) = self.connect(target)?;
        stream.write(&RequestEnvelope {
            version: PROTOCOL_VERSION,
            request: RemoteRequest::ContentGet {
                authorization,
                content_id: content_id.to_owned(),
            },
        })?;
        match read_response(&mut stream)? {
            ControlResponse::ContentAvailable { size } => stream.read_payload(size),
            response => unexpected(response),
        }
    }

    pub fn put(
        &self,
        target: &DeviceId,
        content_id: &str,
        data: &[u8],
    ) -> Result<(), TransportError> {
        let size = u64::try_from(data.len()).map_err(|_| TransportError::ContentTooLarge)?;
        if size > MAX_CONTENT_TRANSFER_SIZE {
            return Err(TransportError::ContentTooLarge);
        }
        let (mut stream, authorization) = self.connect(target)?;
        stream.write(&RequestEnvelope {
            version: PROTOCOL_VERSION,
            request: RemoteRequest::ContentPut {
                authorization,
                content_id: content_id.to_owned(),
                size,
            },
        })?;
        match read_response(&mut stream)? {
            ControlResponse::ContentReady => {}
            response => return unexpected(response),
        }
        stream.write_payload(data)?;
        match read_response(&mut stream)? {
            ControlResponse::ContentStored => Ok(()),
            response => unexpected(response),
        }
    }

    fn connect(
        &self,
        target: &DeviceId,
    ) -> Result<(SecurePeerStream, PeerAuthorization), TransportError> {
        let state = self.store.load()?;
        let cluster = state.cluster.ok_or(TransportError::NotInCluster)?;
        let member = cluster
            .members
            .iter()
            .find(|member| member.member.device_id == *target)
            .ok_or(TransportError::Unauthorized)?;
        let stream = SecurePeerStream::connect(
            member.member.endpoint,
            &self.identity,
            &member.member.device_id,
        )?;
        Ok((
            stream,
            PeerAuthorization {
                cluster_id: cluster.id,
                device_id: state.device_id,
                credential: cluster.credential,
            },
        ))
    }
}

fn read_response(stream: &mut SecurePeerStream) -> Result<ControlResponse, TransportError> {
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

fn unexpected<T>(response: ControlResponse) -> Result<T, TransportError> {
    Err(TransportError::InvalidMessage(format!(
        "unexpected peer content response: {response:?}"
    )))
}
