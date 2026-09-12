# ADR 0001: Development transport boundary

Status: accepted for the bootstrap transport; migration required before Android networking.

`protocol/proto/kubeweft/v1` is the source of truth for cross-language message semantics. TCP length framing does not imply gRPC. The current serde/JSON envelopes are private Rust development scaffolding and are not a stable protocol.

Before an Android client connects, generate Rust and Kotlin protobuf types, keep explicit conversions to domain types, and replace JSON payloads inside the existing frames. Do not move domain semantics into generated types.

Peer framing now runs inside the standard Noise channel described in [ADR 0002](0002-device-identity-and-secure-channel.md). JSON remains a private application-message encoding and still must migrate to generated protobuf payloads before Android networking. Discovery remains an untrusted endpoint hint.
