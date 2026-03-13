import AppKit
import Foundation
import TheEditorFFIBridge

struct SharedContextMenuItemSnapshot: Equatable {
    let id: String
    let title: String
    let enabled: Bool
    let destructive: Bool
    let separatorBefore: Bool
}

enum SecondaryClickMenuSupport {
    static func decodeSnapshot(_ data: ContextMenuSnapshotData) -> [SharedContextMenuItemSnapshot] {
        let count = Int(data.item_count())
        guard count > 0 else { return [] }

        var items: [SharedContextMenuItemSnapshot] = []
        items.reserveCapacity(count)
        for index in 0..<count {
            let item = data.item_at(UInt(index))
            items.append(
                SharedContextMenuItemSnapshot(
                    id: item.id().toString(),
                    title: item.title().toString(),
                    enabled: item.enabled(),
                    destructive: item.destructive(),
                    separatorBefore: item.separator_before()
                )
            )
        }
        return items
    }

    static func addSeparatorIfNeeded(to menu: NSMenu) {
        guard !menu.items.isEmpty else { return }
        guard menu.items.last?.isSeparatorItem == false else { return }
        menu.addItem(.separator())
    }

    static func promptForText(
        title: String,
        message: String,
        confirmTitle: String,
        initialValue: String = ""
    ) -> String? {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.alertStyle = .informational
        alert.addButton(withTitle: confirmTitle)
        alert.addButton(withTitle: "Cancel")

        let field = NSTextField(string: initialValue)
        field.frame = NSRect(x: 0, y: 0, width: 280, height: 24)
        field.placeholderString = initialValue.isEmpty ? "Name" : nil
        alert.accessoryView = field

        let response = alert.runModal()
        guard response == .alertFirstButtonReturn else {
            return nil
        }

        let value = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }

    static func copyToPasteboard(_ value: String) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(value, forType: .string)
    }

    static func pasteboardString() -> String? {
        if let value = NSPasteboard.general.string(forType: .string),
           !value.isEmpty {
            return value
        }
        return nil
    }

    static func revealInFinder(path: String) {
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
    }

    static func openInDefaultApp(path: String) {
        NSWorkspace.shared.open(URL(fileURLWithPath: path))
    }

    static func openInTerminal(directoryPath: String) {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/open")
        process.arguments = ["-a", "Terminal", directoryPath]
        try? process.run()
    }

    static func moveToTrash(path: String) -> Bool {
        do {
            try FileManager.default.trashItem(
                at: URL(fileURLWithPath: path),
                resultingItemURL: nil
            )
            return true
        } catch {
            NSSound.beep()
            return false
        }
    }

    static func relativePath(targetPath: String, rootPath: String) -> String? {
        guard !targetPath.isEmpty, !rootPath.isEmpty else {
            return nil
        }

        let target = URL(fileURLWithPath: targetPath).standardizedFileURL.pathComponents
        let root = URL(fileURLWithPath: rootPath).standardizedFileURL.pathComponents
        guard target.count >= root.count else {
            return nil
        }
        guard Array(target.prefix(root.count)) == root else {
            return nil
        }

        let relative = target.dropFirst(root.count).joined(separator: "/")
        return relative.isEmpty ? "." : relative
    }

    static func directoryPath(for path: String) -> String? {
        guard !path.isEmpty else {
            return nil
        }

        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(atPath: path, isDirectory: &isDirectory) else {
            return nil
        }

        if isDirectory.boolValue {
            return path
        }

        return URL(fileURLWithPath: path).deletingLastPathComponent().path
    }
}

private final class NativeContextMenuActionBox: NSObject {
    let handler: () -> Void

    init(_ handler: @escaping () -> Void) {
        self.handler = handler
    }
}

private final class NativeContextMenuDispatcher: NSObject {
    static let shared = NativeContextMenuDispatcher()

    @objc
    func performAction(_ sender: NSMenuItem) {
        (sender.representedObject as? NativeContextMenuActionBox)?.handler()
    }
}

extension NSMenu {
    func addActionItem(
        title: String,
        enabled: Bool = true,
        destructive: Bool = false,
        handler: @escaping () -> Void
    ) {
        let item = NSMenuItem(
            title: title,
            action: #selector(NativeContextMenuDispatcher.performAction(_:)),
            keyEquivalent: ""
        )
        item.target = NativeContextMenuDispatcher.shared
        item.representedObject = NativeContextMenuActionBox(handler)
        item.isEnabled = enabled
        if destructive {
            item.attributedTitle = NSAttributedString(
                string: title,
                attributes: [.foregroundColor: NSColor.systemRed]
            )
        }
        addItem(item)
    }
}
