# ADR 0001: Development transport boundary

Status: accepted for the bootstrap transport; migration required before Android networking.

`protocol/proto/kubeweft/v1` is the source of truth for cross-language message semantics. TCP length framing does not imply gRPC. The current serde/JSON envelopes are private Rust development scaffolding and are not a stable protocol.

Before an Android client connects, generate Rust and Kotlin protobuf types, keep explicit conversions to domain types, and replace JSON payloads inside the existing frames. Do not move domain semantics into generated types.

The current LAN channel is plaintext. Its invite and membership bearer credentials provide validation only, without confidentiality, integrity against an active network attacker, or cryptographic device identity. The next security design must combine cryptographic device identity, authenticated encrypted channels, and invite/pairing bootstrap using standard reviewed protocols and libraries. Discovery remains an untrusted endpoint hint.
