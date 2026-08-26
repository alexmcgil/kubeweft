# Plugins

This directory owns future plugin packaging and discovery conventions. Plugins extend agents as separate processes over IPC, using the `kubeweft-plugin-sdk` contract when it becomes concrete. Plugins may depend on the SDK and their own platform libraries; core crates must not depend on plugin implementations.

In-process Rust ABI plugins, `dlopen`-based extension loading, and speculative runtime implementations are excluded.
