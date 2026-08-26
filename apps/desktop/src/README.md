# Desktop Rust source boundary

Future desktop Rust code owns client logic and a thin bridge to QML. It may use desktop-local Qt bridge code and platform-independent Kubeweft abstractions. It must not turn core crates into Qt consumers, host the cluster daemon, or embed Linux/Android adapter implementations.
