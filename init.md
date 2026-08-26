Initialize a new open-source monorepo named **kubeweft**.

The project is a personal distributed operating layer over existing devices. Long-term it will connect Linux, Android, Windows, macOS, servers, Raspberry Pi, and other user-owned devices into one trusted capability-based cluster.

Do **not** attempt to implement the full product now.

The goal of this task is to create a clean, minimal, buildable repository foundation with strong architectural boundaries.

## Core architectural principles

The system revolves around these concepts:

- `Device` — physical device participating in the cluster.
- `Capability` — something a device can do, e.g.:
  - `core.files.read`
  - `core.files.write`
  - `core.process.execute`
  - `core.application.list`
  - `core.application.launch`
  - `core.screen.capture`
  - `core.screen.present`
  - `core.input.keyboard`
  - `core.input.pointer`
  - `core.audio.source`
  - `core.audio.sink`
  - `core.compute.cpu`
  - `core.compute.gpu`
- `Resource` — CPU, RAM, GPU, storage, display, microphone, etc.
- `Application` / `Service` — logical entities that may have implementations on different devices.
- `Policy` — authorization and placement constraints.
- `Job` — work owned by the cluster rather than by a specific device.

The most important architecture rule is:

> Platform-specific code implements capabilities. Core code reasons about capabilities.

The core must never depend directly on Linux, Android, Qt, systemd, Sunshine, PipeWire, Docker, Jellyfin, etc.

Examples of dependency directions that must NOT exist:

- scheduler → systemd
- scheduler → Android APIs
- core → Qt
- core → Sunshine
- core → Jellyfin

Platform integrations must live behind adapters/interfaces.

## Technology decisions

Use these technologies:

### Shared/core
- Rust
- Cargo workspace
- Rust edition 2024 where supported by the current stable toolchain

### Linux
- Rust
- Linux agent will eventually run as a daemon/systemd service
- Do not implement systemd integration yet beyond directory/module placeholders if useful

### Desktop UI
- Rust backend
- Qt 6 / Qt Quick / QML
- Keep all Qt-specific code isolated inside the desktop application
- Core crates must not depend on Qt

Do not build a real UI yet. Create only the minimal directory/project skeleton necessary to make the intended boundary clear.

### Android
- Kotlin
- Gradle Kotlin DSL
- Jetpack Compose
- Native Android APIs
- Do not use Flutter, React Native, Electron, Capacitor, or other cross-platform UI frameworks

For now create the Android project structure, but do not implement capabilities or complicated UI.

### Protocol
- Protocol Buffers
- Protocol definitions are language-neutral and shared between Rust and Kotlin

Do not couple the semantic protocol to a particular transport yet.

gRPC may be used later, but this initialization task should focus primarily on schemas and boundaries.

### Manifests
- Human-facing declarative manifests may eventually be YAML
- Schemas should use JSON Schema
- Runtime code must eventually deserialize manifests into typed internal models rather than operate directly on arbitrary YAML values

### Repository tooling
Use:
- `just` as the top-level developer command runner
- Nix flake for reproducible development environment
- `.editorconfig`
- GitHub Actions skeleton
- Markdown documentation

Do not introduce Bazel, Buck, Pants, Nx, Turborepo, or other monorepo orchestration frameworks.

Cargo + Gradle + Nix + just are sufficient.

---

# Repository structure

Create approximately this structure.

Minor adjustments are allowed where required by Cargo, Gradle, Android, Qt, or Nix conventions, but preserve the architectural intent.

```text
kubeweft/
├── .github/
│   └── workflows/
│       └── ci.yml
│
├── docs/
│   ├── architecture.md
│   ├── glossary.md
│   └── adr/
│       └── README.md
│
├── protocol/
│   ├── proto/
│   │   ├── kubeweft/
│   │   │   └── v1/
│   │   │       ├── common.proto
│   │   │       ├── device.proto
│   │   │       ├── capability.proto
│   │   │       ├── resource.proto
│   │   │       ├── application.proto
│   │   │       ├── job.proto
│   │   │       ├── event.proto
│   │   │       └── control.proto
│   │   └── README.md
│   │
│   └── schemas/
│       ├── application.schema.json
│       ├── service.schema.json
│       ├── plugin.schema.json
│       ├── policy.schema.json
│       └── README.md
│
├── crates/
│   ├── kubeweft-model/
│   ├── kubeweft-protocol/
│   ├── kubeweft-capability/
│   ├── kubeweft-identity/
│   ├── kubeweft-policy/
│   ├── kubeweft-scheduler/
│   ├── kubeweft-planner/
│   ├── kubeweft-agent-core/
│   └── kubeweft-plugin-sdk/
│
├── agents/
│   └── linux/
│       ├── src/
│       └── adapters/
│           ├── filesystem/
│           ├── process/
│           ├── systemd/
│           ├── docker/
│           ├── podman/
│           ├── pipewire/
│           └── sunshine/
│
├── apps/
│   ├── cli/
│   │
│   ├── desktop/
│   │   ├── src/
│   │   ├── qml/
│   │   └── README.md
│   │
│   └── android/
│       ├── app/
│       ├── build.gradle.kts
│       ├── settings.gradle.kts
│       └── gradle.properties
│
├── plugins/
│   ├── builtin/
│   └── examples/
│
├── nix/
│   ├── modules/
│   └── README.md
│
├── examples/
│   └── manifests/
│
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── flake.nix
├── flake.lock
├── justfile
├── .gitignore
├── .editorconfig
├── LICENSE
├── README.md
├── CONTRIBUTING.md
├── SECURITY.md
└── CODE_OF_CONDUCT.md
```

Do not create empty directories that Git cannot track without either a meaningful README or an intentional `.gitkeep`.

Prefer small README files explaining intended boundaries rather than meaningless `.gitkeep` files.

---

# Rust workspace

Create a root Cargo workspace.

Each crate should compile.

Keep implementations minimal.

Prefer domain crates to contain only enough code to establish their purpose and dependency direction.

Suggested responsibilities:

## `kubeweft-model`

Pure domain types.

May contain preliminary types such as:

```rust
pub struct DeviceId(...);
pub struct CapabilityId(...);
pub struct JobId(...);
```

Do not over-model the domain yet.

Avoid premature database/storage assumptions.

## `kubeweft-protocol`

Protocol-related Rust types/generated protobuf integration.

Keep generated code separated from handwritten domain code.

## `kubeweft-capability`

Generic capability abstraction and registry concepts.

Do not encode platform-specific capabilities as a giant Rust enum.

Capability identifiers must be extensible strings/namespaced identifiers.

Bad:

```rust
enum Capability {
    Filesystem,
    Sunshine,
    Jellyfin,
    Minecraft,
}
```

Preferred conceptual model:

```rust
CapabilityId("core.application.launch")
CapabilityId("org.jellyfin.library")
```

Core-defined capabilities use the `core.*` namespace.

External integrations may use their own namespaces.

## `kubeweft-scheduler`

Create only a minimal library boundary.

The scheduler must be a pure planning component and must not execute OS actions.

Architecturally:

```text
ClusterState + JobRequest
        ↓
     Scheduler
        ↓
       Plan
```

The plan may eventually contain operations such as:

- stop allocation
- suspend allocation
- reserve resource
- start allocation
- restore allocation

Do not implement real scheduling/preemption yet.

## `kubeweft-planner`

Reserved for higher-level capability/application placement and route planning.

Keep minimal.

## `kubeweft-agent-core`

Platform-independent logic shared by agents where appropriate.

It must not depend on Linux-specific APIs.

## `kubeweft-identity`

Placeholder boundary for device identity, trust, certificates, pairing, etc.

Do not design the entire PKI yet.

## `kubeweft-policy`

Placeholder boundary for authorization/policy evaluation.

Do not build a policy language yet.

## `kubeweft-plugin-sdk`

Define the intended plugin protocol boundary.

Plugins should eventually run out-of-process.

Do NOT design Rust ABI plugins using `dlopen`.

The architecture should assume something like:

```text
agent
  ↕ IPC
plugin process
```

The actual IPC implementation may be added later.

---

# Linux agent

Create a minimal Rust binary for:

```text
agents/linux
```

Name the resulting binary:

```text
kubeweft-agent
```

It should start, log a small message, and exit cleanly or stay alive only if that can be done without adding unnecessary architecture.

Prefer a minimal implementation.

The Linux agent must depend on abstractions from the core crates rather than the opposite direction.

Adapters under:

```text
agents/linux/adapters/
```

represent future platform implementations.

Do not implement real:

- systemd control
- Docker
- Podman
- PipeWire
- Sunshine
- filesystem abstraction

yet.

Each adapter directory may contain a README describing its intended capability mapping.

---

# CLI

Create a Rust CLI binary named:

```text
kubeweft
```

For now implement only something equivalent to:

```text
kubeweft --version
```

and optionally:

```text
kubeweft doctor
```

`doctor` may simply print which parts are placeholders.

Do not add a large CLI framework unless justified.

If using one, keep dependencies minimal.

---

# Android

Create a minimal native Android application.

Use:

- Kotlin
- Gradle Kotlin DSL
- Jetpack Compose
- Material 3

Package namespace:

```text
dev.kubeweft.android
```

Suggested package layout:

```text
dev.kubeweft.android
├── app
├── agent
├── capability
├── cluster
├── platform
└── ui
```

The future conceptual structure should support:

```text
Android process
├── AgentService
├── CapabilityRegistry
├── ClusterClient
├── Android platform adapters
└── Compose UI
```

Do not implement these in detail.

The app should build and display a minimal screen containing:

```text
Kubeweft
No cluster connected
```

Do not add JNI or Rust integration.

For the MVP architecture, Android is an independent Kotlin implementation of the shared protocol.

---

# Desktop

Create the intended desktop application boundary.

Target future stack:

```text
Qt 6
Qt Quick
QML
Rust
```

Do not introduce Electron, Tauri, Flutter, React Native, or a browser-based UI.

The desktop architecture must assume:

```text
QML UI
   ↓
thin Qt/Rust bridge
   ↓
desktop client logic
   ↓
local kubeweft-agent IPC
```

The desktop application must NOT contain the cluster daemon.

Closing the UI must eventually have no effect on the running node/cluster agent.

Do not invest significant effort into CXX-Qt integration yet unless required to create a trivial compiling skeleton.

If Qt tooling complicates the initial repository bootstrap excessively, create a documented placeholder desktop project with clear future structure rather than introducing fragile hacks.

---

# Protocol Buffers

Create minimal valid protobuf definitions.

Use package namespace:

```protobuf
package kubeweft.v1;
```

Start with intentionally small definitions.

For example:

```protobuf
message DeviceId {
  string value = 1;
}
```

and concepts for:

- Device
- Capability
- Resource
- Application
- Job
- Event

Do not attempt to define the final protocol.

The schemas exist primarily to establish ownership and versioning.

Protocol evolution must assume backward compatibility.

Never reuse removed protobuf field numbers.

Add comments where useful.

---

# Control plane vs data plane

Document this explicitly in `docs/architecture.md`.

Kubeweft control plane handles things such as:

- discovering devices
- advertising capabilities
- authorization
- selecting execution location
- starting jobs
- negotiating routes
- tracking state
- explaining decisions

Large application data should normally bypass the controller.

Examples:

```text
Android
  → control request
Linux PC
  → Sunshine/Moonlight stream
Android
```

or:

```text
Phone
  → control request
Server
  → DLNA stream
TV
```

The phone/controller must not unnecessarily proxy the media stream.

Kubeweft coordinates existing mature technologies instead of reimplementing them.

---

# Trust model

Document only the fundamental principle:

> Membership in the network does not imply authorization to every capability.

Do NOT design the entire security architecture yet.

Assume:

- device identity exists
- individual capabilities can be authorized
- compromised membership must not automatically imply SSH/filesystem/admin access across the cluster
- capability access follows least privilege
- VPN/network reachability is not equivalent to trust

ADB, Shizuku, root, sudo, etc. are capability implementation details and must not leak into scheduler semantics.

For example, scheduler sees:

```text
core.application.launch = available
```

not:

```text
available_via_shizuku = true
```

unless backend metadata is specifically relevant to policy.

---

# Manifests

Create minimal JSON Schema placeholders for:

- Application
- Service
- Plugin
- Policy

Also add one example manifest under:

```text
examples/manifests/
```

Use YAML for the example human-facing manifest.

Keep it simple.

Something conceptually similar to:

```yaml
apiVersion: kubeweft.dev/v1alpha1
kind: Application

metadata:
  id: dev.zed.Zed
  name: Zed

spec:
  requires:
    - core.application.launch
```

Do not try to finalize the manifest format.

Mark it clearly as `v1alpha1`.

---

# Naming

Project name:

```text
Kubeweft
```

Repository/binary namespace:

```text
kubeweft
```

Rust crates:

```text
kubeweft-*
```

Protocol namespace:

```text
kubeweft.v1
```

Manifest API namespace:

```text
kubeweft.dev
```

Avoid Kubernetes-specific terminology unless it naturally fits.

Despite the name `Kubeweft`, this project is NOT Kubernetes-based and should not pretend to be a Kubernetes distribution.

---

# `.gitignore`

Create a comprehensive but reasonable `.gitignore` covering at least:

- Rust `/target`
- Gradle
- Android build directories
- Android local.properties
- IntelliJ / Android Studio
- VS Code workspace-local state where appropriate
- Qt generated/build files
- CMake build outputs if Qt skeleton requires them
- Nix result symlinks
- OS junk files
- temporary files
- logs
- environment/secrets files

Do NOT ignore useful project-level configuration files.

Do not ignore Gradle wrapper files.

Do not ignore Cargo.lock for this repository.

---

# `.editorconfig`

Create sane defaults:

- UTF-8
- LF
- final newline
- trim trailing whitespace
- 4 spaces by default
- 2 spaces for YAML/JSON where appropriate
- Markdown may preserve trailing whitespace where needed

---

# Nix

Create a `flake.nix` providing a development shell for Linux development.

It should contain the tools reasonably necessary for this repository, such as:

- stable Rust toolchain support or integration with `rust-toolchain.toml`
- protobuf compiler
- just
- pkg-config
- basic Qt 6 development dependencies if practical
- JDK suitable for Android/Gradle development if practical

Do not attempt to fully package Kubeweft as a NixOS module yet.

Keep:

```text
nix/modules/
```

as a future integration boundary and document it.

The flake should be understandable and not excessively abstract.

---

# `justfile`

Provide convenient top-level commands.

At minimum consider:

```text
just check
just test
just fmt
just lint
just rust-check
just rust-test
just android-check
just proto-check
```

Commands that cannot reasonably work without Android SDK or optional Qt dependencies should fail gracefully or be clearly documented.

`just check` should run the checks that are expected to work in the normal development environment.

---

# CI

Create a GitHub Actions workflow.

Initially prioritize:

- Rust formatting
- Rust compilation/check
- Rust tests
- protobuf validation where practical

Android checks may be included if they can be done cleanly and reproducibly without making the bootstrap unnecessarily complex.

Do not create deployment/release pipelines.

---

# Documentation

Create a useful root `README.md`.

It should contain:

1. What Kubeweft is.
2. What Kubeweft is not.
3. Current project status: very early experimental stage.
4. Core architecture idea.
5. Repository layout.
6. Basic developer commands.
7. Explicit warning that APIs/protocols are unstable.

Use concise technical prose.

Do not oversell the project.

Create `docs/architecture.md` explaining at least:

- device/capability/resource/job model
- control plane vs data plane
- agent model
- adapter model
- scheduler/executor separation
- plugin process isolation
- protocol boundary
- platform independence
- Android and Linux differences
- dependency direction

Create `docs/glossary.md` defining:

- Device
- Capability
- Resource
- Application
- Service
- Job
- Agent
- Controller
- Planner
- Scheduler
- Adapter
- Plugin
- Manifest
- Control plane
- Data plane

Create an ADR directory with a README explaining how ADRs will be added later.

---

# Open-source project files

Create:

- `LICENSE`
- `CONTRIBUTING.md`
- `SECURITY.md`
- `CODE_OF_CONDUCT.md`

Use the Apache License 2.0 unless there is a strong technical reason not to.

Do not invent company contact details or email addresses.

Where a security email would normally be required, refer users to GitHub private vulnerability reporting or leave a clearly marked repository-maintainer placeholder.

---

# Dependency policy

Keep dependencies minimal.

Do not install libraries merely because they might become useful later.

Avoid speculative dependencies for:

- databases
- CRDTs
- QUIC
- service meshes
- Kubernetes
- WebRTC
- distributed consensus
- policy languages
- WASM runtimes
- observability stacks

They can be chosen when a real requirement appears.

---

# Coding expectations

Use:

- clear module boundaries
- idiomatic Rust
- idiomatic Kotlin
- documentation on architectural APIs
- tests for any non-trivial domain behavior you introduce

Avoid:

- premature abstractions
- giant enums for extensible concepts
- global mutable state
- platform-specific dependencies in core
- unsafe Rust unless absolutely unavoidable
- unnecessary macros
- elaborate dependency injection frameworks
- generated boilerplate that does not provide value

---

# Validation

After creating the repository:

1. Inspect the complete resulting directory structure.
2. Run formatting.
3. Run Rust checks.
4. Run Rust tests.
5. Validate protobuf files if tooling is available.
6. Run Android Gradle checks if the environment has the required Android SDK.
7. Run Nix flake checks if Nix is available.
8. Fix issues found during validation.

Do not merely generate files and stop.

At the end, provide a concise report containing:

- resulting repository tree
- important architectural decisions encoded by the structure
- commands successfully validated
- anything that could not be validated because tooling was unavailable
- any intentional deviations from this prompt and why

Do not start implementing networking, pairing, scheduler algorithms, real capabilities, distributed storage, remote application streaming, or plugins yet.

The finished result of this task should be a **small, clean, buildable architectural skeleton of Kubeweft**, not an unfinished implementation of the entire vision.
