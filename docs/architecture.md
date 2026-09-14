# Architecture

Kubeweft is a personal distributed operating layer over user-owned devices. It is **not Kubernetes** and is not Kubernetes-based. This repository is an early experimental foundation: all APIs, manifests, and protocols are unstable.

## Domain model

A **Device** offers **Capabilities** and contains **Resources** such as CPU, storage, displays, or microphones. **Applications** and **Services** are logical entities that may be implemented on different devices. A **Job** is cluster-owned work, constrained by **Policy**, rather than work owned by one machine.

Capabilities are extensible namespaced identifiers, for example `core.application.launch` or `org.example.library`; they are not a closed enum of products or platform mechanisms.

## Dependency direction

Platform applications and adapters depend inward on agent/client orchestration, capability, policy, scheduler, and planner boundaries, which depend on the pure domain model. The protocol is a sibling wire-representation boundary, not the owner of domain semantics.

Core code reasons about capabilities; platform-specific code implements them. The following dependency directions are forbidden:

- `scheduler -> systemd`
- `scheduler -> Android APIs`
- `core -> Qt`
- `core -> Sunshine`
- `core -> Jellyfin`

The same rule excludes direct core dependencies on Linux, Docker, Podman, PipeWire, ADB, Shizuku, root, sudo, or any other platform mechanism. Adapters hide those mechanisms behind capability-oriented interfaces.

## Planning and execution

The scheduler is a pure planning component:

```text
ClusterState + JobRequest -> Scheduler -> Plan
```

It may decide that a plan should reserve a resource, start an allocation, or restore one, but it never executes operating-system actions. Agents and their adapters are the executor side: after authorization and policy decisions, they perform local work. This scheduler/executor separation keeps placement decisions testable and prevents platform authority from leaking into the core model.

## Control plane and data plane

The control plane discovers devices, advertises capabilities, authorizes access, selects execution locations, starts jobs, negotiates routes, tracks state, and explains decisions. The data plane carries application data.

Large data should use a direct path whenever practical. For example, an Android device can issue a control request to a Linux PC, while a Sunshine/Moonlight stream flows directly from the Linux PC to Android. Likewise, a phone can submit a control request while a server streams DLNA media directly to a TV. Controllers coordinate existing mature technologies; they should not become unnecessary media or file proxies.

UDP discovery is an untrusted endpoint hint and network reachability grants no authority. Linux privileged control uses a user-only Unix socket. Peer TCP uses Noise XX with a persistent key per data directory; `DeviceId` fingerprints that key, pairing pins the coordinator, and accepted membership pins the joiner. Membership credentials remain separately revocable authorization. Cluster creation makes the creating agent a temporary membership coordinator; a persistent join transaction makes invite consumption retry-safe. Presence is an ephemeral observation made only after Noise identity and roster credential validation; offline never means removed. Protobuf files remain the cross-language semantic source of truth; the encrypted JSON application payload is still private Rust scaffolding (see [ADR 0001](adr/0001-development-transport-boundary.md), [ADR 0002](adr/0002-device-identity-and-secure-channel.md), and [ADR 0003](adr/0003-presence-and-filesystem-data-plane.md)).

## Logical filesystem

Kubeweft exposes one cluster-owned namespace: paths and stable file identities do not contain placement information. Devices contribute content storage, while metadata and files belong to the cluster.

Content is immutable and SHA-256-addressed. Writes advance metadata generations with compare-and-swap. Stored blobs are distinct from retained content: live file references (and future snapshot retention) receive replication, while garbage candidates remain stored until future GC without gaining replicas. Replica state is merged into the current placement during CAS retries, and an unavailable replica never removes the file from the namespace. The first data plane moves complete immutable blobs directly between authorized peers over Noise and checks `ContentId` at both ends. It never transports namespace or metadata. Filesystem metadata remains independent of the temporary membership coordinator; it will need its own consensus-backed model.

## Agents, adapters, and trust

An agent hosts platform-independent coordination plus local adapters. Linux adapters may eventually map filesystem, process, systemd, container, PipeWire, or Sunshine mechanisms to capabilities. Android is a separate native Kotlin and Jetpack Compose client for the shared protocol; it does not consume Rust through JNI. Linux uses Rust agents and adapters. As Android integration grows, its process, service, registry, client, and Compose boundaries will remain native Kotlin and use Android APIs behind Android adapters.

Network membership or VPN reachability is not trust and never grants capability authorization. Each capability is authorized separately and follows least privilege. A scheduler sees `core.application.launch = available`, not `available_via_shizuku = true`, unless policy deliberately needs backend metadata. Compromised membership must not automatically grant SSH, filesystem, or administrator access.

## Plugins and desktop

Plugins are future out-of-process processes communicating with an agent through IPC. Kubeweft will not use in-process Rust ABI plugins or `dlopen` as the plugin architecture.

The desktop currently uses an in-process bridge for local filesystem development. Its intended networked flow is:

```text
QML UI -> thin Qt/Rust bridge -> desktop client logic -> local kubeweft-agent IPC
```

Qt 6/Qt Quick/QML code stays inside the desktop application. The desktop process does not host the cluster daemon, so closing its UI must not stop the agent.

## Protocol and manifests

Protocol Buffers in `protocol/proto/` are language-neutral and transport-neutral; they do not commit Kubeweft to gRPC. Transport framing is not domain protocol. The current Rust JSON envelopes are private scaffolding and must migrate to generated protobuf payloads before Android networking. Schema-defined, human-facing manifests are eventually deserialized into typed internal models, never treated as arbitrary YAML values at runtime. Protocol evolution must preserve compatibility, including never reusing removed protobuf field numbers.
