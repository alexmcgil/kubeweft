# ADR 0003: Presence and filesystem data plane

Status: accepted for the current prototype.

Presence is local, ephemeral state derived from authenticated probes to known cluster members. Discovery does not establish presence, and presence does not change membership.

The peer filesystem data plane transfers whole immutable content objects over the existing Noise channel. A request must match the Noise-proven `DeviceId`, cluster roster, and membership credential digest. Receivers verify the requested `ContentId`; filesystem metadata and placement remain outside the membership coordinator and outside this protocol.

The current 256 MiB limit and whole-object buffering are prototype constraints. Chunked manifests, resumable transfer, bandwidth control, replicated metadata, endpoint updates, and garbage collection remain separate work.
