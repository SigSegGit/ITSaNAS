// A deliberately standalone Gradle project (its own settings.gradle.kts,
// no dependency at all on ../app or the Android Gradle Plugin) — this is
// the whole point: it must be runnable with nothing but a JVM and Maven
// Central access, so the API contract layer gets tested every time even
// in an environment that can't resolve Google's Maven repo (see
// android/README.md). If this module lived inside the same Gradle build
// as :app, Gradle would try to configure :app's Android plugin too and
// fail before a single test ran.
//
// Compiles the *actual* production source files from ../app (not copies —
// pointed at directly via the sourceSets block below) so this can never
// silently drift from what really ships. Only the files that don't touch
// any `android.*` API are included: UriRequestBody.kt and the Compose/
// Activity code need the real Android SDK and aren't part of this.
plugins {
    kotlin("jvm") version "1.9.24"
    kotlin("plugin.serialization") version "1.9.24"
}

// Repositories are declared once, in settings.gradle.kts's
// dependencyResolutionManagement — declaring them here too conflicts with
// that block's FAIL_ON_PROJECT_REPOS mode.

sourceSets {
    main {
        kotlin {
            srcDir("../app/src/main/java")
            include(
                "com/itsanas/client/network/Models.kt",
                "com/itsanas/client/network/DaemonApi.kt",
                "com/itsanas/client/network/RetrofitClient.kt",
            )
        }
    }
}

dependencies {
    implementation("com.squareup.retrofit2:retrofit:2.11.0")
    implementation("com.jakewharton.retrofit:retrofit2-kotlinx-serialization-converter:1.0.0")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.6.3")

    testImplementation(kotlin("test"))
    testImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.8.1")
}

tasks.test {
    useJUnitPlatform()
}
