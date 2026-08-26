# Kubeweft

> **Very early experimental project. All Kubeweft APIs, manifests, and protocols are unstable and may change without compatibility guarantees.**

Kubeweft is a personal distributed operating layer for coordinating capabilities across user-owned devices. It describes devices, resources, applications, services, policies, and jobs without making a particular operating system or integration part of the core model.

Kubeweft is **not Kubernetes**, a Kubernetes distribution, or a replacement for existing media, container, remote-desktop, or networking technologies. It does not yet implement networking, pairing, persistence, or platform capabilities.

## Architecture in brief

Platform-specific adapters implement capabilities; Rust core code reasons about extensible capability identifiers and plans. The scheduler is pure: `ClusterState + JobRequest -> Plan`. Agents and adapters execute approved plans; the scheduler never performs operating-system actions.

The control plane discovers devices, authorizes capabilities, chooses placement, and coordinates routes. Data such as media and large files should travel directly between appropriate endpoints rather than through a controller proxy. Read the [architecture](docs/architecture.md) and [glossary](docs/glossary.md) before depending on a boundary.

## Layout

- `crates/` — platform-independent Rust domain and orchestration boundaries.
- `agents/linux/` — Linux agent and future platform adapters.
- `apps/cli/`, `apps/android/`, `apps/desktop/` — native client boundaries; desktop is documentation-only for now.
- `protocol/` — transport-neutral protobuf and manifest schemas.
- `plugins/` — future out-of-process plugin boundary.
- `nix/` — reproducible development and future Nix integration boundaries.
- `docs/` — architecture, terminology, and future ADRs.

## Development

The top-level `just` recipes provide the repository checks. In the Nix development shell, use:

```sh
just check
just test
just fmt
just lint
just rust-check
just rust-test
just proto-check
just android-check
```

`android-check` requires an Android SDK. The desktop directory intentionally has no build command yet: it documents the future Qt 6/Qt Quick/QML boundary only.

## Contributing and security

See [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Kubeweft is licensed under [Apache-2.0](LICENSE).
