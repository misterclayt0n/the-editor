import AppKit
import Foundation
import SwiftUI

enum PickerIconLoader {
    private static let cache = SvgIconCache()

    static func image(named icon: String) -> Image? {
        guard let image = cache.image(named: icon) else {
            return nil
        }
        return Image(nsImage: image)
    }
}

private final class SvgIconCache {
    private var images: [String: NSImage] = [:]
    private var missing: Set<String> = []
    private let lock = NSLock()
    private let iconDirectories: [URL]

    init() {
        iconDirectories = SvgIconCache.resolveIconDirectories()
    }

    func image(named icon: String) -> NSImage? {
        let key = icon.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !key.isEmpty else { return nil }

        lock.lock()
        if let cached = images[key] {
            lock.unlock()
            return cached
        }
        if missing.contains(key) {
            lock.unlock()
            return nil
        }
        lock.unlock()

        let loaded = loadImage(named: key)

        lock.lock()
        defer { lock.unlock() }
        if let loaded {
            images[key] = loaded
        } else {
            missing.insert(key)
        }
        return loaded
    }

    private func loadImage(named icon: String) -> NSImage? {
        let candidateNames = [icon, "file_generic"]
        for candidate in candidateNames {
            for root in iconDirectories {
                let url = root.appendingPathComponent(candidate).appendingPathExtension("svg")
                guard FileManager.default.fileExists(atPath: url.path) else { continue }
                guard let image = loadSvgImage(from: url) else { continue }
                image.isTemplate = true
                return image
            }
        }
        return nil
    }

    private func loadSvgImage(from url: URL) -> NSImage? {
        guard let data = try? Data(contentsOf: url) else {
            return nil
        }
        if let pngData = createPngDataFromSvgData(data),
           let image = NSImage(data: pngData as Data) {
            return image
        }
        return NSImage(data: data)
    }

    private static func resolveIconDirectories() -> [URL] {
        var dirs: [URL] = []
        func append(_ url: URL) {
            let standardized = url.standardizedFileURL
            if dirs.contains(where: { $0.path == standardized.path }) {
                return
            }
            var isDirectory: ObjCBool = false
            if FileManager.default.fileExists(atPath: standardized.path, isDirectory: &isDirectory),
               isDirectory.boolValue {
                dirs.append(standardized)
            }
        }

        if let explicitPath = ProcessInfo.processInfo.environment["THE_SWIFT_ICON_DIR"] {
            append(URL(fileURLWithPath: explicitPath, isDirectory: true))
        }

        if let resourceURL = Bundle.module.resourceURL {
            append(resourceURL.appendingPathComponent("icons", isDirectory: true))
            append(resourceURL)
            appendCandidates(from: resourceURL)
        }
        if let mainResourceURL = Bundle.main.resourceURL {
            append(mainResourceURL.appendingPathComponent("icons", isDirectory: true))
            append(mainResourceURL.appendingPathComponent("assets/icons", isDirectory: true))
            appendCandidates(from: mainResourceURL)
        }

        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath, isDirectory: true)
        appendCandidates(from: cwd)

        let sourceDir = URL(fileURLWithPath: #filePath, isDirectory: false)
            .deletingLastPathComponent()
        append(sourceDir.appendingPathComponent("Resources/icons", isDirectory: true))
        append(sourceDir.appendingPathComponent("../../../assets/icons", isDirectory: true))
        appendCandidates(from: sourceDir)

        return dirs

        func appendCandidates(from start: URL, maxDepth: Int = 8) {
            var current = start.standardizedFileURL
            for _ in 0..<maxDepth {
                append(current.appendingPathComponent("assets/icons", isDirectory: true))
                append(current.appendingPathComponent("icons", isDirectory: true))
                let parent = current.deletingLastPathComponent()
                if parent.path == current.path {
                    break
                }
                current = parent
            }
        }
    }
}

// ImageIO exports this symbol on modern macOS and it reliably converts SVG bytes to PNG bytes.
@_silgen_name("CGCreatePNGDataFromSVGData")
private func cgCreatePngDataFromSvgData(_ data: CFData, _ options: CFDictionary?) -> Unmanaged<CFData>?

private func createPngDataFromSvgData(_ data: Data) -> CFData? {
    cgCreatePngDataFromSvgData(data as CFData, nil)?.takeRetainedValue()
}
