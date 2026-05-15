// Minimal Kotlin/Wasm leap-year project.
//
// Uses the Kotlin Multiplatform plugin with the wasmJs() target —
// the standard path for Kotlin → wasm-gc. Output lands in
// build/js/packages/leap-year/kotlin/.

import org.jetbrains.kotlin.gradle.ExperimentalWasmDsl

plugins {
    kotlin("multiplatform") version "2.2.0"
}

repositories {
    mavenCentral()
}

kotlin {
    @OptIn(ExperimentalWasmDsl::class)
    wasmJs {
        binaries.executable()
        nodejs()
    }
}
