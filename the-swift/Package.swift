// swift-tools-version: 5.9
import Foundation
import PackageDescription

let packageRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().path
let ghosttyKitRelativePath = "Frameworks/GhosttyKit.xcframework"
let ghosttyKitPath = URL(fileURLWithPath: ghosttyKitRelativePath, relativeTo: URL(fileURLWithPath: packageRoot)).path
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
      path: ghosttyKitRelativePath
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
  .target(
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

packageTargets.append(
  .executableTarget(
    name: "TheSwiftExecutable",
    dependencies: ["TheSwift"],
    path: "Sources/TheSwiftExecutable"
  )
)

let package = Package(
  name: "the-swift",
  platforms: [.macOS(.v13)],
  products: [
    .library(name: "TheEditorFFIBridge", targets: ["TheEditorFFI", "TheEditorFFIBridge"]),
    .library(name: "TheSwiftAppCore", targets: ["TheSwift"]),
    .executable(name: "the-swift", targets: ["TheSwiftExecutable"]),
  ],
  targets: packageTargets
)
