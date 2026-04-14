// swift-tools-version: 6.0
import Foundation
import PackageDescription

func ghosttyKitCandidates(in root: String, fileManager: FileManager) -> [String] {
    guard fileManager.fileExists(atPath: root),
          let enumerator = fileManager.enumerator(atPath: root)
    else {
        return []
    }

    var candidates: [String] = []
    for case let relativePath as String in enumerator {
        guard relativePath.hasSuffix("/GhosttyKit.xcframework") || relativePath == "GhosttyKit.xcframework" else {
            continue
        }
        candidates.append((root as NSString).appendingPathComponent(relativePath))
    }
    return candidates.sorted()
}

func resolveGhosttyKitSourcePath() -> String? {
    let fileManager = FileManager.default
    let explicitPath = ProcessInfo.processInfo.environment["THE_EDITOR_GHOSTTYKIT_XCFRAMEWORK_PATH"]
        .map { NSString(string: $0).expandingTildeInPath }
    if let explicitPath, fileManager.fileExists(atPath: explicitPath) {
        return explicitPath
    }

    let packageRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().path
    let localPaths = [
        (packageRoot as NSString).appendingPathComponent("GhosttyKit.xcframework"),
        ((packageRoot as NSString).deletingLastPathComponent as NSString).appendingPathComponent("GhosttyKit.xcframework")
    ]
    for path in localPaths where fileManager.fileExists(atPath: path) {
        return path
    }

    let cacheRoots = [
        NSString(string: "~/.cache/cmux/ghosttykit").expandingTildeInPath,
        NSString(string: "~/.cache/the-editor/ghosttykit").expandingTildeInPath,
    ]
    for cacheRoot in cacheRoots {
        if let match = ghosttyKitCandidates(in: cacheRoot, fileManager: fileManager).last {
            return match
        }
    }

    return nil
}

func relativePath(from basePath: String, to destinationPath: String) -> String {
    let baseComponents = URL(fileURLWithPath: basePath).standardized.pathComponents
    let destinationComponents = URL(fileURLWithPath: destinationPath).standardized.pathComponents

    var commonCount = 0
    while commonCount < baseComponents.count,
          commonCount < destinationComponents.count,
          baseComponents[commonCount] == destinationComponents[commonCount] {
        commonCount += 1
    }

    let upward = Array(repeating: "..", count: max(baseComponents.count - commonCount, 0))
    let downward = destinationComponents.dropFirst(commonCount)
    let relativeComponents = upward + downward
    return relativeComponents.isEmpty ? "." : NSString.path(withComponents: relativeComponents)
}

func resolveGhosttyKitBinaryTargetPath() -> String? {
    guard let sourcePath = resolveGhosttyKitSourcePath() else {
        return nil
    }
    let packageRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().path
    return relativePath(from: packageRoot, to: sourcePath)
}

let ghosttyKitPath = resolveGhosttyKitBinaryTargetPath()
let hasGhosttyKit = ghosttyKitPath != nil

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

var linkerSettings: [LinkerSetting] = [
    .linkedLibrary("iconv")
]
if hasGhosttyKit {
    linkerSettings.append(.linkedLibrary("c++"))
    linkerSettings.append(.linkedFramework("Carbon"))
}

targets.append(
    .executableTarget(
        name: "TheSwiftPOC",
        dependencies: executableDependencies,
        path: "Sources/TheSwiftPOC",
        linkerSettings: linkerSettings
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
