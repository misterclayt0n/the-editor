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
    .binaryTarget(
      name: "TheEditorFFI",
      path: "Frameworks/TheEditorFFI.xcframework"
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
      resources: [
        .process("Resources")
      ],
      linkerSettings: [
        .linkedFramework("SwiftUI"),
        .linkedFramework("AppKit"),
      ]
    ),
  ]
)
