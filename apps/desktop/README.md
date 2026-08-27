# Desktop boundary

This is a documentation-only future desktop application boundary; it is not part of the current build. It owns Qt 6, Qt Quick, QML, and the thin Qt/Rust bridge. Its intended flow is `QML UI -> bridge -> desktop client logic -> local kubeweft-agent IPC`.

It may depend on desktop-local code and platform-independent client/core abstractions, but core crates must never depend on Qt. It does not host the cluster daemon, implement the Linux agent, or make closing the UI stop the agent.
