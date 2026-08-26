# Kubeweft Repository Initialization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a small, buildable Kubeweft monorepo whose Rust, protocol, Android, desktop-placeholder, documentation, and tooling boundaries match `init.md`.

**Architecture:** Pure Rust domain crates sit at the center; capability, policy, scheduling, planning, and agent abstractions depend inward on them; binaries and platform adapters sit at the edge. Protocol schemas are language-neutral and transport-neutral, Android is an independent Kotlin client, and the desktop remains a documented Qt/QML boundary.

**Tech Stack:** Rust stable (edition 2024), Cargo, Protocol Buffers 3, JSON Schema 2020-12, Kotlin 2.1.0, Android Gradle Plugin 8.7.3, Gradle 8.9, Jetpack Compose/Material 3, Nix flakes, just, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-08-26-kubeweft-initialization-design.md`

## Global Constraints

- The root `init.md` and the approved design spec are authoritative.
- Core code must not depend on Linux, Android, Qt, systemd, Sunshine, PipeWire, Docker, Podman, Jellyfin, or other platform integrations.
- Capability identifiers are extensible namespaced strings; never model integrations as a closed enum.
- The scheduler produces plans and never executes operating-system actions.
- Plugins are future out-of-process programs; do not define a Rust dynamic-library ABI.
- Protocol Buffer package names are exactly `kubeweft.v1` and are not coupled to a transport.
- Android namespace is exactly `dev.kubeweft.android`; no JNI or Rust integration.
- Desktop is documentation-only in this iteration and is excluded from required builds.
- Keep `Cargo.lock` and Gradle Wrapper files tracked.
- Do not introduce speculative networking, storage, database, consensus, policy-language, streaming, or observability dependencies.
- Prefix every shell command with `rtk`, as required by the repository instructions.

---

### Task 1: Rust workspace and domain identifiers

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `crates/kubeweft-model/Cargo.toml`
- Create: `crates/kubeweft-model/src/lib.rs`
- Create: `crates/kubeweft-model/tests/identifiers.rs`

**Interfaces:**
- Consumes: no project interfaces.
- Produces: `DeviceId`, `CapabilityId`, `JobId`, and `InvalidIdentifier`; each ID supports `new`, `as_str`, `Display`, and `FromStr`.

- [ ] **Step 1: Create the workspace manifests and failing identifier tests**

Start the virtual workspace with only `crates/kubeweft-model` as a member so the
first focused RED/GREEN cycle is runnable. Configure resolver `2`, workspace
package version `0.1.0`, edition `2024`, rust-version `1.85`, Apache-2.0 license,
and no external dependencies. Pin the stable toolchain with `rustfmt` and
`clippy` components. Tasks 2 and 3 append their members; the final workspace has
all eleven Rust members (nine crates plus `agents/linux` and `apps/cli`).

Write integration tests that require:

```rust
use kubeweft_model::{CapabilityId, DeviceId, JobId};

#[test]
fn accepts_namespaced_capability_identifier() {
    let id = CapabilityId::new("core.application.launch").unwrap();
    assert_eq!(id.as_str(), "core.application.launch");
    assert_eq!(id.to_string(), "core.application.launch");
}

#[test]
fn rejects_identifier_without_namespace() {
    let error = CapabilityId::new("launch").unwrap_err();
    assert_eq!(error.to_string(), "identifier must contain at least one dot-separated namespace");
}

#[test]
fn rejects_empty_identifier_segment() {
    assert!(DeviceId::new("device..phone").is_err());
    assert!(JobId::new("jobs.").is_err());
}
```

- [ ] **Step 2: Run the test and verify RED**

Run: `rtk nix develop --command cargo test -p kubeweft-model --test identifiers`

Expected: FAIL because `crates/kubeweft-model/src/lib.rs` and its exported identifier types do not yet exist.

- [ ] **Step 3: Implement the minimal identifier model**

Implement a private validation function accepting non-empty ASCII dot-separated segments containing lowercase letters, digits, `-`, or `_`, requiring at least two segments. Use a small macro only to remove identical newtype boilerplate for the three public types. Store `String`; derive `Debug`, `Clone`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, and `Hash`. Implement a typed error enum without an external error crate.

- [ ] **Step 4: Verify GREEN and workspace formatting**

Run:

```bash
rtk nix develop --command cargo test -p kubeweft-model --test identifiers
rtk nix develop --command cargo fmt --all -- --check
```

Expected: all three tests PASS and formatting exits 0.

- [ ] **Step 5: Commit**

```bash
rtk git add Cargo.toml rust-toolchain.toml crates/kubeweft-model
rtk git commit -m "feat: establish Rust domain model"
```

### Task 2: Core abstraction crates and pure scheduler

**Files:**
- Create: `crates/kubeweft-capability/{Cargo.toml,src/lib.rs}`
- Create: `crates/kubeweft-identity/{Cargo.toml,src/lib.rs}`
- Create: `crates/kubeweft-policy/{Cargo.toml,src/lib.rs}`
- Create: `crates/kubeweft-scheduler/{Cargo.toml,src/lib.rs,tests/scheduler.rs}`
- Create: `crates/kubeweft-planner/{Cargo.toml,src/lib.rs}`
- Create: `crates/kubeweft-agent-core/{Cargo.toml,src/lib.rs}`
- Create: `crates/kubeweft-plugin-sdk/{Cargo.toml,src/lib.rs}`
- Create: `crates/kubeweft-protocol/{Cargo.toml,src/lib.rs}`

**Interfaces:**
- Consumes: `kubeweft_model::{CapabilityId, DeviceId, JobId}`.
- Produces: `CapabilityDescriptor`, `CapabilityProvider`, `AuthorizationDecision`, `ClusterState`, `JobRequest`, `Plan`, `PlanStep`, `Scheduler`, `RouteRequest`, `AgentDescriptor`, `PluginDescriptor`, and a protocol-boundary module marker.

- [ ] **Step 1: Add crate manifests, documented public boundaries, and a failing scheduler test**

Append these eight crates to the root workspace member list. Keep dependencies
one-way: capability, policy, scheduler, planner, agent-core, and plugin-sdk may
depend on model; protocol has no generated dependency yet; model depends on none
of them. Define only data required to state each boundary.

The scheduler test must construct a cluster containing one device advertising `core.compute.cpu`, request that capability, call `Scheduler::plan`, and assert the literal result:

```rust
assert_eq!(
    plan.steps(),
    &[PlanStep::StartAllocation {
        job_id: JobId::new("job.example").unwrap(),
        device_id: DeviceId::new("device.workstation").unwrap(),
    }]
);
```

Add a second test where no device advertises the requested capability and assert an empty plan. Do not add resource scoring, preemption, execution, async code, or platform metadata.

- [ ] **Step 2: Run the scheduler test and verify RED**

Run: `rtk nix develop --command cargo test -p kubeweft-scheduler --test scheduler`

Expected: FAIL because `Scheduler::plan` and/or the scheduler types are missing.

- [ ] **Step 3: Implement the minimum pure planning function**

Implement deterministic first-match selection over the supplied device order. `Plan` owns `Vec<PlanStep>` and exposes `steps(&self) -> &[PlanStep]`. Use domain IDs from `kubeweft-model`. Every crate root must document what it owns and, crucially, what it must not own.

- [ ] **Step 4: Verify all core crates**

Run:

```bash
rtk nix develop --command cargo test --workspace
rtk nix develop --command cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS with no warnings.

- [ ] **Step 5: Commit**

```bash
rtk git add crates
rtk git commit -m "feat: define core architectural boundaries"
```

### Task 3: CLI and Linux agent edge binaries

**Files:**
- Create: `apps/cli/Cargo.toml`
- Create: `apps/cli/src/main.rs`
- Create: `apps/cli/tests/cli.rs`
- Create: `agents/linux/Cargo.toml`
- Create: `agents/linux/src/main.rs`
- Create: `agents/linux/tests/startup.rs`
- Create: `agents/linux/adapters/{README.md,filesystem/README.md,process/README.md,systemd/README.md,docker/README.md,podman/README.md,pipewire/README.md,sunshine/README.md}`

**Interfaces:**
- Consumes: workspace package metadata and `kubeweft-agent-core` for the Linux binary boundary.
- Produces: binaries `kubeweft` and `kubeweft-agent`.

- [ ] **Step 1: Write failing process-level tests**

Append `apps/cli` and `agents/linux` to the root workspace member list. Use
Cargo-provided `CARGO_BIN_EXE_*` paths and `std::process::Command`, without a test
dependency. Assert:

- `kubeweft --version` succeeds and stdout is exactly `kubeweft 0.1.0\n`;
- `kubeweft doctor` succeeds and mentions that cluster connectivity and platform capabilities are placeholders;
- `kubeweft unknown` fails and stderr is exactly `unknown command: unknown\n`;
- `kubeweft-agent` succeeds and stdout is exactly `kubeweft-agent: platform adapters are not configured\n`.

- [ ] **Step 2: Run the binary tests and verify RED**

Run:

```bash
rtk nix develop --command cargo test -p kubeweft-cli --test cli
rtk nix develop --command cargo test -p kubeweft-agent-linux --test startup
```

Expected: FAIL because the binaries are not implemented.

- [ ] **Step 3: Implement minimal argument handling and startup output**

Use `std::env::args_os`; add no CLI or logging dependency. Return exit code 2 for unknown commands. The adapter READMEs map future platform integrations to generic capabilities and explicitly state that core crates must never depend on adapters.

- [ ] **Step 4: Verify GREEN**

Run: `rtk nix develop --command cargo test --workspace`

Expected: every workspace test PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add apps/cli agents/linux
rtk git commit -m "feat: add CLI and Linux agent skeletons"
```

### Task 4: Language-neutral protocol and manifest schemas

**Files:**
- Create: `protocol/proto/README.md`
- Create: `protocol/proto/kubeweft/v1/{common,device,capability,resource,application,job,event,control}.proto`
- Create: `protocol/schemas/{README.md,application.schema.json,service.schema.json,plugin.schema.json,policy.schema.json}`
- Create: `examples/manifests/zed.application.yaml`

**Interfaces:**
- Consumes: vocabulary from the approved design; no Rust or Kotlin types.
- Produces: transport-neutral `kubeweft.v1` messages and `kubeweft.dev/v1alpha1` draft manifest contracts.

- [ ] **Step 1: Create intentionally invalid minimal schema fixtures and verify validators catch them**

Create the eight `.proto` files initially with message shells but put the invalid
field declaration `string value = ;` in `common.proto`; create
`application.schema.json` with a trailing comma. Run:

```bash
rtk nix develop --command protoc -I protocol/proto --descriptor_set_out=/tmp/kubeweft.pb protocol/proto/kubeweft/v1/*.proto
rtk nix develop --command check-jsonschema --check-metaschema protocol/schemas/*.json
```

Expected: both commands FAIL for the intended syntax errors. This establishes that the actual format validators detect the breaks being guarded.

- [ ] **Step 2: Write valid minimal protobuf contracts**

Every file uses `syntax = "proto3";` and `package kubeweft.v1;`. Define wrapper IDs in `common.proto`; use repeated capabilities/resources where needed; keep `control.proto` to request/response envelopes for advertising a device and submitting a job. Import other files by repository-relative proto path. Add comments warning that removed field numbers must be reserved and never reused. Do not add gRPC services.

- [ ] **Step 3: Write valid JSON Schemas and example manifest**

Use JSON Schema draft 2020-12 with stable `$id` values below `https://kubeweft.dev/schemas/v1alpha1/`. Each schema requires `apiVersion`, `kind`, `metadata`, and `spec`, rejects unknown top-level properties, and constrains the exact kind. The application example is:

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

- [ ] **Step 4: Verify protocol and schemas GREEN**

Run:

```bash
rtk nix develop --command protoc -I protocol/proto --descriptor_set_out=/tmp/kubeweft.pb protocol/proto/kubeweft/v1/*.proto
rtk nix develop --command check-jsonschema --check-metaschema protocol/schemas/*.json
```

Expected: both commands exit 0 and `/tmp/kubeweft.pb` is non-empty.

- [ ] **Step 5: Commit**

```bash
rtk git add protocol examples/manifests
rtk git commit -m "feat: add protocol and manifest schemas"
```

### Task 5: Native Android application and Gradle Wrapper

**Files:**
- Create: `apps/android/settings.gradle.kts`
- Create: `apps/android/build.gradle.kts`
- Create: `apps/android/gradle.properties`
- Create: `apps/android/app/build.gradle.kts`
- Create: `apps/android/app/src/main/AndroidManifest.xml`
- Create: `apps/android/app/src/main/java/dev/kubeweft/android/app/MainActivity.kt`
- Create: `apps/android/app/src/main/java/dev/kubeweft/android/ui/KubeweftApp.kt`
- Create: `apps/android/app/src/test/java/dev/kubeweft/android/ui/StatusCopyTest.kt`
- Create: `apps/android/gradle/wrapper/{gradle-wrapper.jar,gradle-wrapper.properties}`
- Create: `apps/android/{gradlew,gradlew.bat}`
- Create: `apps/android/README.md`

**Interfaces:**
- Consumes: the conceptual shared protocol only; no Rust artifacts.
- Produces: Android application namespace `dev.kubeweft.android` and a Compose screen with the approved copy.

- [ ] **Step 1: Create Gradle configuration and a failing copy-model test**

Pin Kotlin and Compose plugins to `2.1.0`, AGP to `8.7.3`, Gradle distribution
to `8.9`, compile/target SDK to `35`, minimum SDK to `26`, and Java/Kotlin
bytecode to 17. Use the Compose BOM `2024.12.01`, `activity-compose`, and
Material 3. Generate the checked-in wrapper before running Gradle:

```bash
rtk nix shell nixpkgs#gradle --command gradle -p apps/android wrapper --gradle-version 8.9
```

Define a `StatusCopy` value returned by `statusCopy()` and test literal
title/status values before implementing it.

- [ ] **Step 2: Run the focused test and verify RED**

Run: `rtk apps/android/gradlew -p apps/android testDebugUnitTest`

Expected: FAIL because `StatusCopy`/`statusCopy()` is absent. If the Android SDK is absent, run the same command inside an environment with `ANDROID_HOME` configured; do not misreport missing SDK as the intended RED failure.

- [ ] **Step 3: Implement the minimal Compose application**

Implement `statusCopy()` returning `StatusCopy("Kubeweft", "No cluster connected")`; render those two values in a centered Material 3 `Column`. `MainActivity` only hosts `KubeweftApp`. The README documents future `agent`, `capability`, `cluster`, `platform`, and `ui` boundaries without creating empty package directories.

- [ ] **Step 4: Verify Android GREEN where SDK is available**

Run: `rtk apps/android/gradlew -p apps/android testDebugUnitTest lintDebug assembleDebug`

Expected: PASS. If no SDK is installed, verify `rtk apps/android/gradlew -p apps/android --version` and record the SDK limitation for final validation.

- [ ] **Step 5: Commit**

```bash
rtk git add apps/android
rtk git commit -m "feat: add native Android application skeleton"
```

### Task 6: Architecture and open-source documentation

**Files:**
- Create: `README.md`
- Create: `docs/architecture.md`
- Create: `docs/glossary.md`
- Create: `docs/adr/README.md`
- Create: `apps/desktop/{README.md,src/README.md,qml/README.md}`
- Create: `plugins/{README.md,builtin/README.md,examples/README.md}`
- Create: `nix/{README.md,modules/README.md}`
- Create: `CONTRIBUTING.md`
- Create: `SECURITY.md`
- Create: `CODE_OF_CONDUCT.md`
- Modify: `LICENSE`

**Interfaces:**
- Consumes: all approved architectural decisions and repository commands.
- Produces: the human-facing architecture contract and project governance baseline.

- [ ] **Step 1: Write the root and architecture documentation**

Cover every documentation item listed in `init.md`. Include explicit forbidden dependency examples, scheduler/executor separation, control-plane/data-plane examples, direct data paths, least privilege, VPN-not-trust, Android/Linux implementation differences, out-of-process plugins, and the Qt/QML desktop flow. State prominently that Kubeweft is not Kubernetes and that all APIs/protocols are unstable.

- [ ] **Step 2: Write glossary and boundary READMEs**

Define every glossary term required by `init.md`. Replace every otherwise-empty future directory with a concise README explaining ownership, allowed dependencies, and exclusions.

- [ ] **Step 3: Add project policy files**

Use the unmodified Apache License 2.0 text with copyright notice
`Copyright 2026 Kubeweft contributors`. Base the code of conduct on Contributor Covenant 2.1 without inventing contact details. Direct security reports to GitHub private vulnerability reporting and explicitly ask users not to open public issues for undisclosed vulnerabilities.

- [ ] **Step 4: Check prose and tracked directory coverage**

Run:

```bash
rtk rg -n "Kubernetes|control plane|data plane|least privilege|unstable" README.md docs
rtk rg --files apps/desktop plugins nix/modules docs/adr agents/linux/adapters
rtk git diff --check
```

Expected: required concepts are present, every intended boundary has a tracked file, and whitespace check passes. This is a review aid, not a substitute for human document review.

- [ ] **Step 5: Commit**

```bash
rtk git add README.md docs apps/desktop plugins nix CONTRIBUTING.md SECURITY.md CODE_OF_CONDUCT.md LICENSE
rtk git commit -m "docs: establish project architecture and policies"
```

### Task 7: Repository tooling, Nix shell, and CI

**Files:**
- Create: `.gitignore`
- Create: `.editorconfig`
- Create: `flake.nix`
- Create: `flake.lock`
- Create: `justfile`
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: Cargo workspace, protocol paths, Android Gradle project.
- Produces: reproducible development shell and commands `check`, `test`, `fmt`, `lint`, `rust-check`, `rust-test`, `android-check`, and `proto-check`.

- [ ] **Step 1: Add EditorConfig and ignore policy**

Configure UTF-8, LF, final newlines, trailing whitespace removal, four-space defaults, two spaces for YAML/JSON, and preserved Markdown trailing whitespace. Ignore Rust, Gradle/Android, IDE-local, Qt/CMake, Nix result, OS junk, logs, temp, and secret/environment artifacts while keeping Cargo.lock, Gradle Wrapper, project IDE settings where useful, and all source configuration tracked.

- [ ] **Step 2: Add the Nix development shell**

Use a pinned `nixos-unstable` input with `flake-utils`; provide `cargo`, `rustc`,
`rustfmt`, `clippy`, `protobuf`, `just`, `pkg-config`, `jq`, JDK 17, and practical
Qt 6 packages and `python3Packages.check-jsonschema` on Linux. The lock file makes the Nixpkgs stable Rust package
selection reproducible, while `rust-toolchain.toml` gives rustup-based
environments the same edition-compatible stable-toolchain intent. Add
`packages`/`checks` only where they genuinely validate existing artifacts; avoid
packaging the unfinished applications.

- [ ] **Step 3: Add just recipes and verify missing-tool behavior**

Implement:

```text
check: fmt-check lint rust-check rust-test proto-check
test: rust-test
fmt: cargo fmt --all
lint: cargo clippy --workspace --all-targets -- -D warnings
rust-check: cargo check --workspace --all-targets
rust-test: cargo test --workspace
proto-check: protoc descriptor generation plus JSON Schema metaschema validation
android-check: clear SDK preflight, then Gradle test/lint/assemble
```

Before finalizing the Android recipe, run it once with `ANDROID_HOME` and `ANDROID_SDK_ROOT` unset and verify it exits non-zero with a concise message explaining how to enable the check.

- [ ] **Step 4: Add CI and lock the flake**

CI checks out the repository, installs the pinned Rust toolchain and `protoc`, caches Cargo conservatively, and runs `cargo fmt --check`, Clippy with warnings denied, workspace tests/checks, and protobuf/schema validation. Do not add Android, desktop, release, or deployment jobs. Generate `flake.lock` with `rtk nix flake lock`.

- [ ] **Step 5: Validate tooling**

Run:

```bash
rtk nix flake check
rtk nix develop --command just check
rtk git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 6: Commit**

```bash
rtk git add .gitignore .editorconfig flake.nix flake.lock justfile .github/workflows/ci.yml
rtk git commit -m "ci: add reproducible repository tooling"
```

### Task 8: Complete validation and architectural audit

**Files:**
- Modify: only files implicated by validation failures.
- Create: `Cargo.lock` if Cargo has not already generated it.

**Interfaces:**
- Consumes: the complete repository.
- Produces: fresh verification evidence and a clean buildable tree.

- [ ] **Step 1: Inspect the full tracked structure**

Run:

```bash
rtk git status --short
rtk tree -a -I '.git|target|.gradle|build|result'
rtk cargo metadata --no-deps --format-version 1
```

Expected: only intentional changes exist, required directories contain meaningful tracked files, and all eleven Cargo members are listed.

- [ ] **Step 2: Audit forbidden dependency directions**

Run `rtk cargo tree --workspace` and inspect every internal edge. Confirm core crates contain no platform dependencies and no Qt, Android, systemd, container, media, database, networking, or async runtime crate appears.

- [ ] **Step 3: Run the full verification matrix**

Run:

```bash
rtk nix develop --command cargo fmt --all -- --check
rtk nix develop --command cargo check --workspace --all-targets
rtk nix develop --command cargo test --workspace
rtk nix develop --command cargo clippy --workspace --all-targets -- -D warnings
rtk nix develop --command just proto-check
rtk nix flake check
rtk git diff --check
```

If an Android SDK is configured, also run:

```bash
rtk apps/android/gradlew -p apps/android testDebugUnitTest lintDebug assembleDebug
```

Expected: every available check exits 0. Record unavailable Android SDK validation explicitly rather than weakening the project or installing an unrequested SDK.

- [ ] **Step 4: Fix failures with test-first evidence**

For behavioral failures, add or refine a focused failing test, observe the intended failure, apply the smallest fix, and rerun the focused and full suites. For declarative syntax/tooling failures, run the actual validator to observe the failure, make the minimal correction, and rerun it.

- [ ] **Step 5: Commit final generated lockfile or validation fixes**

```bash
rtk git add Cargo.lock
rtk git add -u
rtk git commit -m "chore: finalize repository bootstrap"
```

Skip this commit only if there are no remaining changes.

- [ ] **Step 6: Prepare the handoff report**

Report the resulting tree at a readable depth, architectural boundaries encoded,
exact successful commands, unavailable checks and reasons, and any intentional
deviation from `init.md`. Do not claim success for a command not run in this final
validation pass.
