// swift-tools-version: 5.9
import PackageDescription

let package = Package(
  name: "the-swift",
  platforms: [.macOS(.v13)],
  products: [
    .library(name: "TheEditorFFIBridge", targets: ["TheEditorFFI", "TheEditorFFIBridge"]),
    .executable(name: "the-swift", targets: ["TheSwift"]),
  ],
  targets: [
    .target(
      name: "TheEditorFFI",
      path: "Bridge",
      publicHeadersPath: "include",
      cSettings: [
        .headerSearchPath("include"),
      ],
      linkerSettings: [
        .unsafeFlags(["-L", "../target/release", "-lthe_ffi"]),
      ]
    ),
    .target(
      name: "TheEditorFFIBridge",
      dependencies: ["TheEditorFFI"],
      path: "Sources/TheEditorFFIBridge"
    ),
    .executableTarget(
      name: "TheSwift",
      dependencies: ["TheEditorFFIBridge"],
      path: "Sources/TheSwift",
      linkerSettings: [
        .linkedFramework("SwiftUI"),
        .linkedFramework("AppKit"),
      ]
    ),
  ]
)
