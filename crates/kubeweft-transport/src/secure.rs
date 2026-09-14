use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

use kubeweft_identity::DevicePublicKey;
use kubeweft_model::DeviceId;
use serde::{Serialize, de::DeserializeOwned};

use crate::{TransportError, identity::LocalDeviceIdentity};

const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
const PROLOGUE: &[u8] = b"kubeweft-peer-channel-v1";
const MAX_HANDSHAKE_FRAME: usize = 512;
const MAX_CIPHERTEXT_FRAME: usize = 65_535;
const MAX_PLAINTEXT_FRAME: usize = MAX_CIPHERTEXT_FRAME - 16;
pub(crate) const MAX_CONTENT_TRANSFER_SIZE: u64 = 256 * 1024 * 1024;

pub(crate) fn noise_parameters() -> Result<snow::params::NoiseParams, TransportError> {
    NOISE_PATTERN.parse().map_err(|_| {
        TransportError::SecureChannel("invalid built-in Noise protocol parameters".into())
    })
}

pub(crate) struct SecurePeerStream {
    stream: TcpStream,
    transport: snow::TransportState,
    remote_device_id: DeviceId,
}

impl SecurePeerStream {
    pub(crate) fn connect(
        endpoint: SocketAddr,
        identity: &LocalDeviceIdentity,
        expected_remote: &DeviceId,
    ) -> Result<Self, TransportError> {
        let stream =
            TcpStream::connect_timeout(&endpoint, Duration::from_secs(3)).map_err(|error| {
                match error.kind() {
                    std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::NotFound => TransportError::AgentUnavailable,
                    _ => error.into(),
                }
            })?;
        Self::configure(&stream)?;
        let builder = snow::Builder::new(noise_parameters()?)
            .local_private_key(identity.private_key())
            .map_err(secure_channel_error)?;
        let mut handshake = builder
            .prologue(PROLOGUE)
            .map_err(secure_channel_error)?
            .build_initiator()
            .map_err(secure_channel_error)?;
        let mut stream = stream;
        let mut outgoing = [0_u8; MAX_HANDSHAKE_FRAME];
        let written = handshake
            .write_message(&[], &mut outgoing)
            .map_err(secure_channel_error)?;
        write_binary_frame(&mut stream, &outgoing[..written])?;

        let incoming = read_binary_frame(&mut stream, MAX_HANDSHAKE_FRAME)?;
        handshake
            .read_message(&incoming, &mut [])
            .map_err(secure_channel_error)?;
        let remote_device_id = remote_device_id(&handshake)?;
        if &remote_device_id != expected_remote {
            return Err(TransportError::PeerIdentityMismatch {
                expected: expected_remote.clone(),
                actual: remote_device_id,
            });
        }

        let written = handshake
            .write_message(&[], &mut outgoing)
            .map_err(secure_channel_error)?;
        write_binary_frame(&mut stream, &outgoing[..written])?;
        let transport = handshake
            .into_transport_mode()
            .map_err(secure_channel_error)?;
        Ok(Self {
            stream,
            transport,
            remote_device_id: expected_remote.clone(),
        })
    }

    pub(crate) fn accept(
        stream: TcpStream,
        identity: &LocalDeviceIdentity,
    ) -> Result<Self, TransportError> {
        Self::configure(&stream)?;
        let builder = snow::Builder::new(noise_parameters()?)
            .local_private_key(identity.private_key())
            .map_err(secure_channel_error)?;
        let mut handshake = builder
            .prologue(PROLOGUE)
            .map_err(secure_channel_error)?
            .build_responder()
            .map_err(secure_channel_error)?;
        let mut stream = stream;
        let incoming = read_binary_frame(&mut stream, MAX_HANDSHAKE_FRAME)?;
        handshake
            .read_message(&incoming, &mut [])
            .map_err(secure_channel_error)?;

        let mut outgoing = [0_u8; MAX_HANDSHAKE_FRAME];
        let written = handshake
            .write_message(&[], &mut outgoing)
            .map_err(secure_channel_error)?;
        write_binary_frame(&mut stream, &outgoing[..written])?;

        let incoming = read_binary_frame(&mut stream, MAX_HANDSHAKE_FRAME)?;
        handshake
            .read_message(&incoming, &mut [])
            .map_err(secure_channel_error)?;
        let remote_device_id = remote_device_id(&handshake)?;
        let transport = handshake
            .into_transport_mode()
            .map_err(secure_channel_error)?;
        Ok(Self {
            stream,
            transport,
            remote_device_id,
        })
    }

    pub(crate) fn remote_device_id(&self) -> &DeviceId {
        &self.remote_device_id
    }

    pub(crate) fn write<T: Serialize>(&mut self, value: &T) -> Result<(), TransportError> {
        let plaintext = serde_json::to_vec(value)?;
        if plaintext.is_empty() || plaintext.len() > MAX_PLAINTEXT_FRAME {
            return Err(TransportError::InvalidMessage(
                "secure peer message is outside the accepted size range".into(),
            ));
        }
        let mut ciphertext = vec![0_u8; plaintext.len() + 16];
        let written = self
            .transport
            .write_message(&plaintext, &mut ciphertext)
            .map_err(secure_channel_error)?;
        write_binary_frame(&mut self.stream, &ciphertext[..written])
    }

    pub(crate) fn read<T: DeserializeOwned>(&mut self) -> Result<T, TransportError> {
        let ciphertext = read_binary_frame(&mut self.stream, MAX_CIPHERTEXT_FRAME)?;
        let mut plaintext = vec![0_u8; ciphertext.len()];
        let read = self
            .transport
            .read_message(&ciphertext, &mut plaintext)
            .map_err(secure_channel_error)?;
        if read == 0 || read > MAX_PLAINTEXT_FRAME {
            return Err(TransportError::InvalidMessage(
                "secure peer message is outside the accepted size range".into(),
            ));
        }
        Ok(serde_json::from_slice(&plaintext[..read])?)
    }

    pub(crate) fn write_payload(&mut self, payload: &[u8]) -> Result<(), TransportError> {
        for chunk in payload.chunks(MAX_PLAINTEXT_FRAME) {
            let mut ciphertext = vec![0_u8; chunk.len() + 16];
            let written = self
                .transport
                .write_message(chunk, &mut ciphertext)
                .map_err(secure_channel_error)?;
            write_binary_frame(&mut self.stream, &ciphertext[..written])?;
        }
        Ok(())
    }

    pub(crate) fn read_payload(&mut self, size: u64) -> Result<Vec<u8>, TransportError> {
        if size > MAX_CONTENT_TRANSFER_SIZE {
            return Err(TransportError::ContentTooLarge);
        }
        let size = usize::try_from(size).map_err(|_| TransportError::ContentTooLarge)?;
        let mut payload = Vec::with_capacity(size);
        while payload.len() < size {
            let ciphertext = read_binary_frame(&mut self.stream, MAX_CIPHERTEXT_FRAME)?;
            let mut plaintext = vec![0_u8; ciphertext.len()];
            let read = self
                .transport
                .read_message(&ciphertext, &mut plaintext)
                .map_err(secure_channel_error)?;
            if read == 0 || payload.len().saturating_add(read) > size {
                return Err(TransportError::InvalidMessage(
                    "content payload length does not match its header".into(),
                ));
            }
            payload.extend_from_slice(&plaintext[..read]);
        }
        Ok(payload)
    }

    fn configure(stream: &TcpStream) -> Result<(), TransportError> {
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        Ok(())
    }
}

fn remote_device_id(handshake: &snow::HandshakeState) -> Result<DeviceId, TransportError> {
    let public_key = handshake.get_remote_static().ok_or_else(|| {
        TransportError::SecureChannel("Noise handshake did not authenticate a static key".into())
    })?;
    Ok(DevicePublicKey::try_from_slice(public_key)
        .map_err(|error| TransportError::InvalidIdentity(error.to_string()))?
        .device_id())
}

fn write_binary_frame(stream: &mut impl Write, bytes: &[u8]) -> Result<(), TransportError> {
    let size = u32::try_from(bytes.len())
        .map_err(|_| TransportError::InvalidMessage("peer frame is too large".into()))?;
    stream.write_all(&size.to_be_bytes())?;
    stream.write_all(bytes)?;
    stream.flush()?;
    Ok(())
}

fn read_binary_frame(stream: &mut impl Read, maximum: usize) -> Result<Vec<u8>, TransportError> {
    let mut size = [0_u8; 4];
    stream.read_exact(&mut size)?;
    let size = usize::try_from(u32::from_be_bytes(size))
        .map_err(|_| TransportError::InvalidMessage("invalid peer frame size".into()))?;
    if size == 0 || size > maximum {
        return Err(TransportError::InvalidMessage(format!(
            "peer frame size {size} is outside the accepted range"
        )));
    }
    let mut bytes = vec![0_u8; size];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn secure_channel_error(error: snow::Error) -> TransportError {
    TransportError::SecureChannel(error.to_string())
}
