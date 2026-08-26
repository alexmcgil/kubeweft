# Kubeweft Android

This is Kubeweft's native Android application. It is an independent Kotlin and
Jetpack Compose client: it does not consume Rust artifacts or JNI bindings.

The current screen reports that no cluster is connected. It only depends on the
conceptual shared protocol, leaving protocol transport and native integration
for later work.

## Future package boundaries

The following packages are documented boundaries for future implementation;
they are intentionally not created until they contain real code.

- `dev.kubeweft.android.agent`: agent discovery, connection, and lifecycle.
- `dev.kubeweft.android.capability`: capability presentation and invocation.
- `dev.kubeweft.android.cluster`: cluster identity, selection, and state.
- `dev.kubeweft.android.platform`: Android platform services and integrations.
- `dev.kubeweft.android.ui`: Compose screens and presentation state.

## Build

With Android SDK platform 35 installed and `ANDROID_HOME` configured:

```sh
./gradlew testDebugUnitTest lintDebug assembleDebug
```
