import AppKit
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
                guard let image = NSImage(contentsOf: url) else { continue }
                image.isTemplate = true
                return image
            }
        }
        return nil
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

        if let resourceURL = Bundle.module.resourceURL {
            append(resourceURL.appendingPathComponent("icons", isDirectory: true))
            append(resourceURL)
        }

        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath, isDirectory: true)
        append(cwd.appendingPathComponent("assets/icons", isDirectory: true))
        append(cwd.appendingPathComponent("../assets/icons", isDirectory: true))
        append(cwd.appendingPathComponent("../../assets/icons", isDirectory: true))

        let sourceDir = URL(fileURLWithPath: #filePath, isDirectory: false)
            .deletingLastPathComponent()
        append(sourceDir.appendingPathComponent("Resources/icons", isDirectory: true))
        append(sourceDir.appendingPathComponent("../../../../assets/icons", isDirectory: true))

        return dirs
    }
}
