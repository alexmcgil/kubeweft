# Desktop bridge

Qt-facing C++ code maps QML actions to the small Rust C ABI in `../bridge`. The bridge owns local client wiring; it does not turn core crates into Qt consumers or host the Linux agent.
