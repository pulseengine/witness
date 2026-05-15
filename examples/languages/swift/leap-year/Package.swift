// swift-tools-version: 6.0
//
// Minimal SwiftPM manifest so `swift build --swift-sdk ...` can
// cross-compile the leap-year predicate to wasm32-wasip1.

import PackageDescription

let package = Package(
    name: "leap",
    targets: [
        .executableTarget(
            name: "leap",
            path: ".",
            exclude: ["build.sh", "README.md", ".gitignore"],
            sources: ["leap.swift"]
        )
    ]
)
