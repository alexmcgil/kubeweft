# Kubeweft desktop

The desktop application is a Qt 6/Qt Quick filesystem browser backed by the Rust filesystem core through a thin C ABI bridge. Build it from the repository development shell with:

```sh
just desktop-build
./target/desktop/kubeweft-desktop
```

It currently opens persistent local development state. Agent IPC replaces this in a later networking iteration; the core crates remain independent from Qt.
