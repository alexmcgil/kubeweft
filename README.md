# Kubeweft

> **Very early experimental project. All Kubeweft APIs, manifests, and protocols are unstable and may change without compatibility guarantees.**

Kubeweft is a personal distributed operating layer for coordinating capabilities across user-owned devices. It describes devices, resources, applications, services, policies, and jobs without making a particular operating system or integration part of the core model.

Kubeweft is **not Kubernetes**, a Kubernetes distribution, or a replacement for existing media, container, remote-desktop, or networking technologies. It does not yet implement consensus, coordinator failover, or production platform capabilities.

## Architecture in brief

Platform-specific adapters implement capabilities; Rust core code reasons about extensible capability identifiers and plans. The scheduler is pure: `ClusterState + JobRequest -> Plan`. Agents and adapters execute approved plans; the scheduler never performs operating-system actions.

The control plane discovers devices, authorizes capabilities, chooses placement, and coordinates routes. Data such as media and large files should travel directly between appropriate endpoints rather than through a controller proxy. Read the [architecture](docs/architecture.md) and [glossary](docs/glossary.md) before depending on a boundary.

## Layout

- `crates/` — platform-independent Rust domain and orchestration boundaries.
- `crates/kubeweft-filesystem/` — logical namespace, immutable content, placement, and replication core.
- `agents/linux/` — Linux agent and future platform adapters.
- `apps/cli/`, `apps/android/`, `apps/desktop/` — CLI, native Android, and Qt/QML clients.
- `protocol/` — transport-neutral protobuf and manifest schemas.
- `plugins/` — future out-of-process plugin boundary.
- `nix/` — reproducible development and future Nix integration boundaries.
- `docs/` — architecture, terminology, and future ADRs.

## Development

The top-level `just` recipes provide the repository checks. In the Nix development shell, use:

```sh
just check
just build
just test
just fmt
just lint
just rust-check
just rust-test
just proto-check
just android-check
```

`just build` produces release CLI/agent binaries, the Qt desktop application, and an Android debug APK. Android builds require SDK platform 35 via `ANDROID_HOME` or `ANDROID_SDK_ROOT`. CLI and desktop share local state through `KUBEWEFT_DATA_DIR` (default: the platform application-data directory).

For a local two-agent cluster, start agents with separate data directories, run `kubeweft --data-dir <first> cluster create home`, then use the returned endpoint and pairing code with `kubeweft --data-dir <second> cluster join <endpoint> <code>`. `cluster discover` returns untrusted LAN hints, `cluster members` shows membership, and `cluster presence` shows current authenticated reachability observations. Across hosts, bind with `kubeweft-agent --listen 0.0.0.0:37846 --advertise <LAN-IP>:37846`. Peer TCP traffic uses Noise XX with persistent device keys; the pairing code pins the coordinator identity during the first join. The filesystem data plane can transfer immutable blobs directly between members, while namespace metadata is still local prototype state.

## Contributing and security

See [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Kubeweft is licensed under [Apache-2.0](LICENSE).
