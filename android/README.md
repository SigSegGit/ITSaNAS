# itsanas-android

A thin Android client over `itsanas-daemon`'s local HTTP API (D9, min SDK
29) — Kotlin + Jetpack Compose, Retrofit + OkHttp + kotlinx-serialization.

Unlike `itsanas-gui` on desktop, this app **does not run a daemon itself**
and does not attempt a synced-folder mirror — it's a genuinely thin
client that connects to a daemon running elsewhere (your desktop, or a
dedicated NAS box) at a base URL you configure once, over your LAN or
whatever tunnel/VPN already gets your phone to that machine. The daemon
itself only ever binds to `127.0.0.1` by design, so reaching it from a
phone always requires something in between (Tailscale, WireGuard, SSH
port-forward, etc.) — this app doesn't set that up, it just talks to
whatever address you point it at.

## What's here

- `MainActivity.kt` / `Screens.kt`: Compose UI — connect (base URL) →
  create/unlock account → file list with upload/download/delete.
- `DaemonViewModel.kt`: talks to the API, holds UI state as a `StateFlow`.
- `network/`: `DaemonApi` (Retrofit interface matching `itsanas-daemon`'s
  routes exactly), `Models.kt` (the same JSON shapes as `http.rs`'s
  `StatusResponse`/`FileInfo`/`PasswordRequest`), `RetrofitClient.kt`,
  and `UriRequestBody.kt` (streams an upload straight from a content
  `Uri` instead of buffering it into memory — matters since the daemon's
  own body-size limit is intentionally unbounded).
- `logic-tests/`: a standalone (non-Android) Gradle project testing the
  `network/` contract layer above — see "Testing" below.

## Building and running

Requires Android Studio (or the command-line SDK + `gradlew`) — **this
has not been compiled or run**. Confirmed directly in this sandbox: `gradle
tasks` fails immediately trying to resolve the Android Gradle Plugin
itself —

```
Plugin [id: 'com.android.application', version: '8.5.2', apply: false] was not found
  ...could not resolve plugin artifact 'com.android.application:com.android.application.gradle.plugin:8.5.2'
  Searched in the following repositories: Google, MavenRepo, Gradle Central Plugin Repository
```

— because this environment's network egress policy blocks Google's Maven
repository (the same policy that blocks `dl.google.com` for SDK
downloads). Gradle 8.14.3 and JDK 21 are present here, so everything
*except* reaching Google's infrastructure works; on a machine with normal
internet access, `./gradlew` will fetch the AGP + SDK components it needs
on first run same as any Android project.

**What actually is verified, permanently**: `logic-tests/` — a separate,
standalone Gradle project (own `settings.gradle.kts`, no dependency on
this `app` module or the Android Gradle Plugin) that compiles the *real*
`network/Models.kt`, `DaemonApi.kt`, and `RetrofitClient.kt` — pointed at
directly, not copies, via a custom `sourceSets` block, so it can't
silently drift from what actually ships — and round-trips them against
literal JSON shaped exactly like `itsanas-daemon`'s real responses. Only
needs a JVM and Maven Central (not blocked here, unlike Google's repo),
so run it any time with:

```sh
../../scripts/test-android-logic.sh
```

This is what caught a real bug during development: the
`kotlinx-serialization` Retrofit converter's package is
`com.jakewharton.retrofit2.converter.kotlinx.serialization`, not
`retrofit2.converter.kotlinx.serialization` as first written — and it's
what would catch the next one, e.g. a field renamed on one side of the
API without the other being updated. `UriRequestBody.kt` and everything
under `MainActivity.kt`/`Screens.kt` (Compose, `ContentResolver`,
activity result contracts) does need the real Android SDK and remains
unverified until built on a machine with SDK access — treat that part
accordingly:

```sh
cd android
./gradlew assembleDebug   # first run downloads the Gradle wrapper + Android SDK components
```

Install the resulting APK (`app/build/outputs/apk/debug/app-debug.apk`)
on a device or emulator, open the app, enter the daemon's address (e.g.
`http://192.168.1.20:4279` if it's reachable directly on your LAN), and
proceed the same way you would on desktop: create or unlock the account,
then upload/download/delete files.
