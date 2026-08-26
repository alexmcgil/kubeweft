set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

check: fmt-check lint rust-check rust-test proto-check

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

android-check:
    if [[ -z "${ANDROID_HOME:-}" && -z "${ANDROID_SDK_ROOT:-}" ]]; then \
        echo "Android SDK not configured; set ANDROID_HOME or ANDROID_SDK_ROOT to run android-check." >&2; \
        exit 1; \
    fi
    ./apps/android/gradlew -p apps/android testDebugUnitTest lintDebug assembleDebug
