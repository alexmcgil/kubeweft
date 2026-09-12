//! Transport-independent device identity concepts.
//!
//! Private-key persistence and secure-channel handshakes belong to platform and
//! transport adapters. The stable identity is the fingerprint of a public key,
//! so a device identifier can be verified from proof of key possession.

use std::fmt::Write as _;

use kubeweft_model::DeviceId;
use sha2::{Digest, Sha256};

const DEVICE_ID_DOMAIN: &[u8] = b"kubeweft-device-identity-v1\0x25519\0";

/// Public key used by the current device-identity suite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DevicePublicKey([u8; 32]);

impl DevicePublicKey {
    pub const LENGTH: usize = 32;

    pub fn from_bytes(bytes: [u8; Self::LENGTH]) -> Self {
        Self(bytes)
    }

    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, InvalidPublicKey> {
        let bytes = bytes.try_into().map_err(|_| InvalidPublicKey)?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; Self::LENGTH] {
        &self.0
    }

    /// Derives a crypto-agile, namespaced identifier from the public key.
    pub fn device_id(&self) -> DeviceId {
        let digest = Sha256::new()
            .chain_update(DEVICE_ID_DOMAIN)
            .chain_update(self.0)
            .finalize();
        let mut encoded = String::with_capacity(7 + digest.len() * 2);
        encoded.push_str("device.");
        for byte in digest {
            write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
        }
        DeviceId::new(encoded).expect("hexadecimal device fingerprint is a valid identifier")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidPublicKey;

impl std::fmt::Display for InvalidPublicKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("device public key must contain exactly 32 bytes")
    }
}

impl std::error::Error for InvalidPublicKey {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_ids_are_stable_and_key_bound() {
        let first = DevicePublicKey::from_bytes([1; 32]).device_id();
        assert_eq!(first, DevicePublicKey::from_bytes([1; 32]).device_id());
        assert_ne!(first, DevicePublicKey::from_bytes([2; 32]).device_id());
        assert!(first.as_str().starts_with("device."));
    }
}
