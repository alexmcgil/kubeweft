# Kubeweft Android

This is Kubeweft's native Android application. It is an independent Kotlin and
Jetpack Compose client: it does not consume Rust artifacts or JNI bindings.

The current Compose screen is a mobile namespace preview. It remains an
independent client until cluster transport is implemented; it does not embed the
Rust filesystem through JNI.

## Build

With Android SDK platform 35 installed and `ANDROID_HOME` configured:

```sh
./gradlew testDebugUnitTest lintDebug assembleDebug
```
