// swift-tools-version: 5.9
import Foundation
import PackageDescription

let ghosttyKitPath = "Frameworks/GhosttyKit.xcframework"
let ghosttyEnabled = FileManager.default.fileExists(atPath: ghosttyKitPath)

var bridgeTargetDependencies: [Target.Dependency] = ["TheEditorFFI"]
if ghosttyEnabled {
  bridgeTargetDependencies.append("GhosttyKit")
}

var executableDependencies: [Target.Dependency] = ["TheEditorFFIBridge"]
if ghosttyEnabled {
  executableDependencies.append("GhosttyKit")
}

var packageTargets: [Target] = [
  .binaryTarget(
    name: "TheEditorFFI",
    path: "Frameworks/TheEditorFFI.xcframework"
  ),
]

if ghosttyEnabled {
  packageTargets.append(
    .binaryTarget(
      name: "GhosttyKit",
      path: ghosttyKitPath
    )
  )
}

packageTargets.append(
  .target(
    name: "TheEditorFFIBridge",
    dependencies: bridgeTargetDependencies,
    path: "Sources/TheEditorFFIBridge",
    linkerSettings: [
      .linkedLibrary("z"),
      .linkedLibrary("iconv"),
    ]
  )
)

packageTargets.append(
  .executableTarget(
    name: "TheSwift",
    dependencies: executableDependencies,
    path: "Sources/TheSwift",
    resources: [
      .process("Resources")
    ],
    linkerSettings: [
      .linkedFramework("SwiftUI"),
      .linkedFramework("AppKit"),
      .linkedFramework("UserNotifications"),
      .linkedFramework("Carbon"),
      .linkedLibrary("c++"),
    ]
  )
)

let package = Package(
  name: "the-swift",
  platforms: [.macOS(.v13)],
  products: [
    .library(name: "TheEditorFFIBridge", targets: ["TheEditorFFI", "TheEditorFFIBridge"]),
    .executable(name: "the-swift", targets: ["TheSwift"]),
  ],
  targets: packageTargets
)
