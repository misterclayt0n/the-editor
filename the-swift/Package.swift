// swift-tools-version: 6.0
import Foundation
import PackageDescription

func resolveGhosttyKitSourcePath() -> String? {
    let fileManager = FileManager.default
    let explicitPath = ProcessInfo.processInfo.environment["THE_EDITOR_GHOSTTYKIT_XCFRAMEWORK_PATH"]
        .map { NSString(string: $0).expandingTildeInPath }
    if let explicitPath, fileManager.fileExists(atPath: explicitPath) {
        return explicitPath
    }

    let cacheRoot = NSString(string: "~/.cache/the-editor/ghosttykit").expandingTildeInPath
    guard fileManager.fileExists(atPath: cacheRoot),
          let enumerator = fileManager.enumerator(atPath: cacheRoot)
    else {
        return nil
    }

    var candidates: [String] = []
    for case let relativePath as String in enumerator {
        guard relativePath.hasSuffix("/GhosttyKit.xcframework") || relativePath == "GhosttyKit.xcframework" else {
            continue
        }
        candidates.append((cacheRoot as NSString).appendingPathComponent(relativePath))
    }
    return candidates.sorted().last
}

func prepareGhosttyKitBinaryTargetPath() -> String? {
    guard let sourcePath = resolveGhosttyKitSourcePath() else {
        return nil
    }

    let fileManager = FileManager.default
    let packageRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    let buildSupportDirectory = packageRoot.appendingPathComponent(".build/ghostty", isDirectory: true)
    let linkPath = buildSupportDirectory.appendingPathComponent("GhosttyKit.xcframework", isDirectory: true)

    try? fileManager.createDirectory(at: buildSupportDirectory, withIntermediateDirectories: true)
    if fileManager.fileExists(atPath: linkPath.path) {
        try? fileManager.removeItem(at: linkPath)
    }
    do {
        try fileManager.createSymbolicLink(atPath: linkPath.path, withDestinationPath: sourcePath)
        return ".build/ghostty/GhosttyKit.xcframework"
    } catch {
        return nil
    }
}

let ghosttyKitPath = prepareGhosttyKitBinaryTargetPath()

var executableDependencies: [Target.Dependency] = ["TheEditorFFI"]
var targets: [Target] = [
    .binaryTarget(
        name: "TheEditorFFI",
        path: "RustBridge/TheEditorFFI.xcframework"
    )
]

if let ghosttyKitPath {
    targets.append(
        .binaryTarget(
            name: "GhosttyKit",
            path: ghosttyKitPath
        )
    )
    executableDependencies.append("GhosttyKit")
}

targets.append(
    .executableTarget(
        name: "TheSwiftPOC",
        dependencies: executableDependencies,
        path: "Sources/TheSwiftPOC"
    )
)

let package = Package(
    name: "TheSwiftPOC",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(name: "TheSwiftPOC", targets: ["TheSwiftPOC"])
    ],
    targets: targets
)
