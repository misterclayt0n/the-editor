// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "TheSwiftPOC",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(name: "TheSwiftPOC", targets: ["TheSwiftPOC"])
    ],
    targets: [
        .binaryTarget(
            name: "TheEditorFFI",
            path: "RustBridge/TheEditorFFI.xcframework"
        ),
        .executableTarget(
            name: "TheSwiftPOC",
            dependencies: ["TheEditorFFI"],
            path: "Sources/TheSwiftPOC"
        )
    ]
)
