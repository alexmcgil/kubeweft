set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

check: fmt-check lint rust-check rust-test proto-check desktop-check

build: x86-build desktop-build android-build

x86-build:
    cargo build --release -p kubeweft-cli -p kubeweft-agent-linux

desktop-build:
    cargo build --release -p kubeweft-desktop-bridge
    mkdir -p target/desktop-build target/desktop
    qmake6 apps/desktop/kubeweft-desktop.pro -o target/desktop-build/Makefile
    make -C target/desktop-build

desktop-check: desktop-build
    status=0; QT_QPA_PLATFORM=offscreen timeout 2 target/desktop/kubeweft-desktop >/dev/null 2>&1 || status=$?; if [[ "$status" != 124 ]]; then echo "desktop smoke test failed with status $status" >&2; exit "$status"; fi

test: rust-test

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

rust-check:
    cargo check --workspace --all-targets

rust-test:
    cargo test --workspace

proto-check:
    mkdir -p target/proto
    protoc --proto_path=protocol/proto --include_imports --descriptor_set_out=target/proto/kubeweft-v1.pb protocol/proto/kubeweft/v1/*.proto
    check-jsonschema --check-metaschema protocol/schemas/*.schema.json
    check-jsonschema --schemafile protocol/schemas/application.schema.json examples/manifests/zed.application.yaml

android-check:
    if [[ -z "${ANDROID_HOME:-}" && -z "${ANDROID_SDK_ROOT:-}" ]]; then \
        echo "Android SDK not configured; set ANDROID_HOME or ANDROID_SDK_ROOT to run android-check." >&2; \
        exit 1; \
    fi
    ./apps/android/gradlew -p apps/android testDebugUnitTest lintDebug assembleDebug

android-build: android-check
