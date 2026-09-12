# ADR 0002: Device identity and peer channel

Status: accepted for the current peer control transport.

Each agent data directory owns a persistent X25519 private key. `DeviceId` is a domain-separated SHA-256 fingerprint of its public key. Peer TCP uses the standard `Noise_XX_25519_ChaChaPoly_BLAKE2s` handshake and encrypts every application frame. The pairing code binds the first join to the coordinator's fingerprint; the coordinator binds membership to the joining key proven by the handshake. Later requests require both that pinned identity and the revocable membership credential.

Discovery remains an untrusted reachability hint. Possession of an unexpired pairing code authorizes one new identity. Private keys are protected by user-only file permissions; OS keystores, key rotation, and recovery are not implemented, and copying a data directory copies its identity. Endpoint freshness is not presence. Filesystem metadata remains independent of membership coordination. Presence and filesystem data protocols may reuse this channel only through separate application-message boundaries.
