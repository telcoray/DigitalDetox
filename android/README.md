# Digital Detox for Android (preview)

Android 11+ / ARM64. First test device: HONOR 400 Lite, Android 16.

This is a test APK, not a published F-Droid release. It implements Detox, the Windows-compatible
25/5/15 Pomodoro schedule, pause/resume, a blocking accessibility overlay, an intentional local
emergency exit, optional device-administrator uninstall friction and local-network Sync.
The UI is currently English. No Google Play Services, advertising, analytics or cloud backend.

## Build from source

Install JDK 17, Rust (with `aarch64-linux-android`), Android SDK platform 36 and NDK 28.2.13676358.
Set `ANDROID_HOME` and `JAVA_HOME`. From this directory:

```
rustup target add aarch64-linux-android
./gradlew assembleDebug testDebugUnitTest lintDebug
cargo test --manifest-path native/Cargo.toml --locked
```

On Windows use `gradlew.bat`. Gradle builds the native library from source before packaging the
APK. Do not commit `jniLibs`, keystores or build outputs. The wrapper pins Gradle 8.14 and the
Android plugin is pinned to 8.13.1. Cargo.lock pins native dependencies.

The native build compiles the existing Windows `src/sync/mod.rs` and `protocol.rs` with Android
adapters for configuration, broadcast addresses and event delivery. The pairing cryptography
and wire format are not reimplemented: SPAKE2, HMAC-SHA256, UDP 47821 and unsigned device IDs
remain compatible. Group keys are encrypted using Android Keystore before persistence.

## Setup and consent

The first screen explains the accessibility permission before opening Android settings.
The service observes foreground package names only; window-content retrieval is disabled.
The blocking overlay covers other apps while a session is active. Phone/dialer screens are
exempt, and the overlay provides a delayed, confirmed emergency exit. Exiting does not unlock
other paired devices. Optional device administration requests no wipe/password policies.
To uninstall, finish or exit the session and deactivate Digital Detox in Device admin apps.

Enable notifications. On HONOR, allow Auto-launch, Secondary launch and Run in background
under App launch/manual management and review battery optimization. Menu labels vary by OS.
Android may require allowing restricted settings before enabling accessibility for a sideloaded APK.

Pair only devices you trust: after explicitly consenting to pairing and Sync, a paired device
can start sessions on this phone. Both devices must be on a LAN that permits peer traffic;
Windows Firewall must allow Digital Detox on private networks. Disable Sync to opt out.
Messages received during a lock cannot shorten that lock; newer state waits until it ends.

## Validation

Automated: four Java schedule tests and the three existing Rust protocol tests. APK signature,
native-library alignment, build and Android Lint are checked separately. Lint warnings remain
for English-only UI strings and intentional synchronous persistence/service lifetime references.

Before relying on this build, manually test on the HONOR:

1. One-minute Detox: Home, Back, Recents, notification shade, screen off/on; ensure expiry unlocks.
2. Emergency exit: wait ten seconds, confirm; verify calls are still reachable.
3. Pair with Windows in both directions; verify the phone appears and sessions follow each other.
4. Pomodoro: reminder/status, break transition, pause/resume, long break and stop.
5. Disable Sync, restart the app/phone, reconnect Wi-Fi, leave the screen off; check persistence.

On the HONOR 400 Lite (Android 16), the user verified automatic unlock after a one-minute Detox,
Windows pairing, bidirectional Detox start, and synchronized Pomodoro lock/unlock across a full
25-minute focus period and 5-minute break. Android/MagicOS can stop
services or permit system-level escapes; this is not an unbreakable device lock. Screen-off
network delivery can be delayed by Doze. Broadcast addresses are captured when the service
starts, so restart it after changing networks if discovery stops working. Unlike the desktop
app, this preview does not yet include a breathing animation or hardware screen dimming.

## F-Droid

License: MIT, inherited from the repository root. All native and Java dependencies are free
software. A source-only build is provided; no proprietary SDKs or bundled native binaries are
needed in Git. Publication remains pending remaining device checks, a release signing strategy,
final privacy/store metadata and an F-Droid build recipe/review. Debug signatures are for testing
only; moving to F-Droid signing may require uninstalling this preview and pairing again.
