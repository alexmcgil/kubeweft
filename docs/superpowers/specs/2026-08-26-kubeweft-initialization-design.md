# Kubeweft Repository Initialization Design

## Objective

Initialize Kubeweft as a small, buildable open-source monorepo that makes its
long-term architectural boundaries explicit without implementing the distributed
product. The repository will provide working Rust foundations, a minimal native
Android application, language-neutral protocol and manifest schemas, documented
platform integration boundaries, and reproducible developer tooling.

The requirements in the repository root `init.md` are authoritative. This design
resolves the few choices that document leaves open.

## Scope

This iteration includes:

- a Cargo workspace using Rust edition 2024;
- nine focused Rust library crates, a Linux agent binary, and a CLI binary;
- minimal Protocol Buffer definitions in package `kubeweft.v1`;
- versioned JSON Schemas and one `v1alpha1` YAML manifest example;
- a native Kotlin/Jetpack Compose Android application with a checked-in Gradle
  Wrapper;
- a documented Qt 6/Qt Quick/QML desktop boundary, not a compiled Qt bridge;
- Nix, `just`, EditorConfig, GitHub Actions, and repository policy files;
- architecture, glossary, adapter, plugin, and future-integration documentation.

This iteration excludes networking, pairing, persistence, real capability
implementations, scheduling algorithms, process plugins, media streaming, JNI,
Rust/Qt bridging, packaging, deployment, and release automation.

## Architectural Direction

Kubeweft core code reasons about devices, extensible capability identifiers,
resources, applications, policies, jobs, and plans. It never reasons about the
platform mechanism used to implement a capability. Linux, Android, Qt, systemd,
container engines, PipeWire, Sunshine, and other integrations remain outside the
core dependency graph.

The intended dependency direction is:

```text
platform applications and adapters
              ↓
agent/client orchestration boundaries
              ↓
capability, policy, scheduler, and planner abstractions
              ↓
       pure domain model
```

`kubeweft-protocol` is a sibling boundary for wire representations rather than
the owner of domain semantics. Generated protobuf code, when introduced, will be
kept separate from handwritten domain code. Protocol schemas are transport
neutral and evolve compatibly; removed protobuf field numbers are never reused.

## Rust Components

- `kubeweft-model` owns small domain identifiers and preliminary value types.
  Namespaced identifiers are strings, not closed enums.
- `kubeweft-capability` owns generic provider and registry contracts.
- `kubeweft-identity` reserves the identity, trust, and pairing boundary without
  designing a PKI.
- `kubeweft-policy` reserves capability authorization and placement policy
  evaluation without defining a policy language.
- `kubeweft-scheduler` is a pure planning boundary. Its shape is
  `ClusterState + JobRequest -> Plan`; it performs no operating-system actions.
- `kubeweft-planner` reserves higher-level placement and route planning.
- `kubeweft-agent-core` contains only platform-independent agent coordination.
- `kubeweft-plugin-sdk` describes an eventual out-of-process plugin contract and
  must not expose an in-process Rust ABI.
- `kubeweft-protocol` owns Rust-side protocol integration boundaries. Initial
  Cargo builds do not require code generation or a locally installed `protoc`.

Non-trivial behavior is deliberately scarce. Extensible identifier validation
and any pure scheduler transformation receive behavior-focused unit tests. Empty
or declarative boundaries are documented rather than padded with fake logic.

The Linux binary is named `kubeweft-agent`. It logs a short startup message and
exits successfully. Future Linux adapter directories each explain the
capabilities they may implement; they contain no placeholder platform code.

The CLI binary is named `kubeweft`. It supports `--version` and a small `doctor`
command without adopting a large command-line framework. Unknown commands fail
with a non-zero exit status and concise diagnostic output.

## Platform Applications

The Android application is an independent Kotlin implementation of the shared
protocol. It uses Gradle Kotlin DSL, Jetpack Compose, Material 3, namespace
`dev.kubeweft.android`, and a checked-in Gradle Wrapper. Its only user-facing
behavior is a screen containing “Kubeweft” and “No cluster connected.” Suggested
agent, capability, cluster, platform, and UI package boundaries are represented
only where meaningful documentation or code is needed; no JNI or Rust integration
is introduced.

The desktop directory documents the future path:

```text
QML UI -> thin Qt/Rust bridge -> desktop client logic -> local agent IPC
```

It is intentionally excluded from required builds in this iteration. The desktop
process does not host the cluster daemon, and closing it must not stop the agent.

## Protocol and Manifests

Protocol Buffer files define small, valid messages for common identifiers,
devices, capabilities, resources, applications, jobs, events, and control-plane
requests. All use `package kubeweft.v1`. Semantic messages do not select gRPC or
another transport.

Draft JSON Schemas establish ownership and versioning for Application, Service,
Plugin, and Policy manifests. A single YAML example uses
`apiVersion: kubeweft.dev/v1alpha1`. Runtime code will eventually deserialize
manifests into typed models; arbitrary YAML values are not a domain API.

## Control, Data, and Trust

The control plane discovers devices, advertises capabilities, authorizes access,
selects placement, starts jobs, negotiates routes, tracks state, and explains
decisions. Large application data flows directly between suitable endpoints via
mature technologies whenever practical; the controller is not an unnecessary
media or file proxy.

Network membership and reachability do not grant capability authorization.
Capability access follows least privilege. Details such as root, sudo, ADB, or
Shizuku implement capabilities behind adapters and do not leak into scheduler
semantics unless policy explicitly needs backend metadata.

## Tooling and Validation

The Nix flake exposes an understandable Linux development shell with stable Rust,
`protoc`, `just`, `pkg-config`, a suitable JDK, and practical Qt 6 development
dependencies. It does not package Kubeweft or define a NixOS service.

The `justfile` is the top-level interface for formatting, checks, tests, linting,
Rust validation, protobuf validation, and Android validation. The normal `check`
path covers checks expected in the development shell. Android checks detect and
clearly report a missing Android SDK; the optional desktop placeholder has no
build check.

GitHub Actions initially validates Rust formatting, compilation, tests, linting,
and protobuf syntax. It contains no release or deployment workflow.

Completion requires fresh evidence from:

- repository tree inspection;
- formatting checks;
- Cargo checks, tests, and Clippy;
- protobuf syntax validation when `protoc` is available;
- JSON Schema parsing/validation with an available standard tool;
- Android Gradle checks when an Android SDK is available;
- Nix flake evaluation/checks when Nix is available.

Unavailable host tooling is reported explicitly. The Nix development shell may
be used to provide missing Rust, Cargo, `just`, or `protoc` tools.

## Error Handling and Testing

Identifier parsing returns typed errors for malformed values. The CLI gives
predictable exit codes and diagnostics. Optional tool checks distinguish a missing
toolchain from a failed validation.

Behavior is developed test-first. Tests assert observable contracts rather than
file contents or implementation details. Human-facing documentation,
configuration, generated wrapper files, and declarative schemas are validated by
their consumers or format validators rather than artificial unit tests.

## Repository Documentation and Policy

The root README states what Kubeweft is and is not, identifies the project as
early and experimental, explains the architecture and layout, lists basic
commands, and warns that APIs and protocols are unstable. Architecture and
glossary documents define the dependency rules and shared vocabulary. ADR,
adapter, plugin, Nix module, protocol, and schema directories contain concise
boundary READMEs instead of meaningless `.gitkeep` files.

The repository uses Apache License 2.0, a contributor guide, a code of conduct,
and a security policy that directs reports to GitHub private vulnerability
reporting without inventing contact details.

## Success Criteria

The result is a clean tracked tree matching the intent of `init.md`; every Rust
workspace member builds and tests; the CLI and Linux agent exhibit only their
specified minimal behavior; protobuf and schemas are valid; Android is a genuine
native Compose project with a reproducible wrapper; optional checks are either
successful or explicitly documented as unavailable; and no forbidden
platform-to-core dependency or speculative product implementation is introduced.
