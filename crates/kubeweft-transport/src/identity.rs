use std::{fs, io::Write, path::Path};

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use curve25519_dalek::montgomery::MontgomeryPoint;
use kubeweft_identity::DevicePublicKey;
use kubeweft_model::DeviceId;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::TransportError;

const IDENTITY_VERSION: u8 = 1;

pub(crate) struct LocalDeviceIdentity {
    private_key: Zeroizing<[u8; 32]>,
    public_key: DevicePublicKey,
}

impl LocalDeviceIdentity {
    pub(crate) fn load_or_create(data_directory: &Path) -> Result<Self, TransportError> {
        let path = data_directory.join("identity.json");
        if path.exists() {
            return Self::load(&path);
        }

        let parameters = super::secure::noise_parameters()?;
        let keypair = snow::Builder::new(parameters)
            .generate_keypair()
            .map_err(secure_channel_error)?;
        let private_key: [u8; 32] =
            keypair.private.as_slice().try_into().map_err(|_| {
                TransportError::InvalidIdentity("invalid generated private key".into())
            })?;
        let public_key: [u8; 32] =
            keypair.public.as_slice().try_into().map_err(|_| {
                TransportError::InvalidIdentity("invalid generated public key".into())
            })?;
        let stored = StoredIdentity {
            version: IDENTITY_VERSION,
            suite: "Noise_XX_25519_ChaChaPoly_BLAKE2s".into(),
            private_key: STANDARD_NO_PAD.encode(private_key),
            public_key: STANDARD_NO_PAD.encode(public_key),
        };
        let temporary = path.with_extension("tmp");
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec_pretty(&stored)?)?;
        file.sync_all()?;
        set_private_permissions(&temporary)?;
        fs::rename(temporary, &path)?;
        sync_parent_directory(&path)?;
        Self::from_parts(private_key, public_key)
    }

    fn load(path: &Path) -> Result<Self, TransportError> {
        set_private_permissions(path)?;
        let stored: StoredIdentity = serde_json::from_slice(&fs::read(path)?)?;
        if stored.version != IDENTITY_VERSION || stored.suite != "Noise_XX_25519_ChaChaPoly_BLAKE2s"
        {
            return Err(TransportError::InvalidIdentity(
                "unsupported device identity version or suite".into(),
            ));
        }
        let private_key = decode_key(&stored.private_key, "private")?;
        let public_key = decode_key(&stored.public_key, "public")?;
        Self::from_parts(private_key, public_key)
    }

    fn from_parts(private_key: [u8; 32], public_key: [u8; 32]) -> Result<Self, TransportError> {
        if private_key.iter().all(|byte| *byte == 0) || public_key.iter().all(|byte| *byte == 0) {
            return Err(TransportError::InvalidIdentity(
                "device identity contains an invalid zero key".into(),
            ));
        }
        let derived_public_key = MontgomeryPoint::mul_base_clamped(private_key).to_bytes();
        if derived_public_key != public_key {
            return Err(TransportError::InvalidIdentity(
                "stored public and private keys do not match".into(),
            ));
        }
        Ok(Self {
            private_key: Zeroizing::new(private_key),
            public_key: DevicePublicKey::from_bytes(public_key),
        })
    }

    pub(crate) fn private_key(&self) -> &[u8; 32] {
        &self.private_key
    }

    pub(crate) fn device_id(&self) -> DeviceId {
        self.public_key.device_id()
    }
}

#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    version: u8,
    suite: String,
    private_key: String,
    public_key: String,
}

fn decode_key(encoded: &str, name: &str) -> Result<[u8; 32], TransportError> {
    let decoded = STANDARD_NO_PAD.decode(encoded).map_err(|_| {
        TransportError::InvalidIdentity(format!("invalid base64 {name} identity key"))
    })?;
    decoded.try_into().map_err(|_| {
        TransportError::InvalidIdentity(format!("{name} identity key must contain 32 bytes"))
    })
}

fn secure_channel_error(error: snow::Error) -> TransportError {
    TransportError::SecureChannel(error.to_string())
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<(), TransportError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> Result<(), TransportError> {
    Ok(())
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> Result<(), TransportError> {
    let parent = path.parent().ok_or_else(|| {
        TransportError::InvalidIdentity("identity path has no parent directory".into())
    })?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> Result<(), TransportError> {
    Ok(())
}
