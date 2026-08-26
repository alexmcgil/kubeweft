# Contributing to Kubeweft

Kubeweft is early and experimental. Please discuss substantial architectural changes before implementing them, and keep changes small and focused.

Respect the dependency direction: platform-specific code implements capabilities, while core code reasons about capabilities. Do not add direct core dependencies on Linux, Android, Qt, systemd, container engines, media systems, or speculative infrastructure.

Before submitting a change, run the relevant `just` commands: `check`, `test`, `fmt`, `lint`, `rust-check`, `rust-test`, `proto-check`, and (with an Android SDK) `android-check`. Update architecture and glossary documentation when shared terminology or a boundary changes. By contributing, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
