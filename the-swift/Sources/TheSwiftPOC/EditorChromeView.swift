import AppKit
import SwiftUI

struct EditorChromeModel {
    let document: EditorDocumentChrome
    let statusBar: EditorStatusBarState
    let backgroundColor: NSColor

    static let empty = EditorChromeModel(
        document: .empty,
        statusBar: .empty,
        backgroundColor: .windowBackgroundColor
    )
}

struct EditorChromeView: View {
    @ObservedObject var controller: EditorSurfaceController

    var body: some View {
        VStack(spacing: 0) {
            EditorSurfaceRepresentable(controller: controller)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            EditorStatusAccessoryView(chrome: controller.chrome, mode: controller.currentMode)
        }
        .background(EditorWindowChromeAccessor(chrome: controller.chrome))
    }
}

private struct EditorStatusAccessoryView: View {
    let chrome: EditorChromeModel
    let mode: EditorMode

    private var metadataItems: [EditorStatusItem] {
        [
            chrome.document.languageName.map {
                EditorStatusItem(icon: "curlybraces", text: $0, emphasis: .muted)
            },
            chrome.document.encodingName.map {
                EditorStatusItem(icon: "textformat", text: $0, emphasis: .muted)
            },
            chrome.document.lineEndingName.map {
                EditorStatusItem(icon: "return", text: $0, emphasis: .muted)
            },
        ].compactMap { $0 }
    }

    var body: some View {
        HStack(spacing: 12) {
            ModePill(mode: mode)

            Spacer(minLength: 12)

            HStack(spacing: 10) {
                ForEach(chrome.statusBar.items) { item in
                    StatusAccessoryItemView(item: item)
                }

                ForEach(metadataItems) { item in
                    StatusAccessoryItemView(item: item)
                }

                Text(chrome.statusBar.cursorText)
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundStyle(.primary)
                    .lineLimit(1)
                    .padding(.leading, 4)
            }
        }
        .padding(.horizontal, 14)
        .frame(height: 28)
        .background(Color(nsColor: chromeBackgroundColor(base: chrome.backgroundColor)))
        .overlay(alignment: .top) {
            Divider()
        }
        .accessibilityElement(children: .contain)
    }
}

private struct StatusAccessoryItemView: View {
    let item: EditorStatusItem

    var body: some View {
        HStack(spacing: 5) {
            if let icon = item.icon {
                Image(systemName: symbolName(for: icon, isDirectory: false))
                    .font(.system(size: 10, weight: .semibold))
            }

            if !item.text.isEmpty {
                Text(item.text)
                    .font(textFont)
                    .lineLimit(1)
            }
        }
        .foregroundStyle(foregroundStyle)
        .accessibilityElement(children: .combine)
    }

    private var textFont: Font {
        switch item.emphasis {
        case .normal:
            return .system(size: 11, weight: .regular)
        case .muted:
            return .system(size: 11, weight: .regular)
        case .strong:
            return .system(size: 11, weight: .semibold)
        }
    }

    private var foregroundStyle: Color {
        if let icon = item.icon {
            switch icon {
            case "diagnostic_error":
                return .red
            case "diagnostic_warning":
                return .orange
            case "diagnostic_info":
                return .blue
            case "diagnostic_hint":
                return .teal
            case "pi":
                return .purple
            case "copilot", "copilot_disabled", "copilot_init", "copilot_error", "supermaven", "supermaven_disabled", "supermaven_init", "supermaven_error":
                return .accentColor
            default:
                break
            }
        }

        switch item.emphasis {
        case .normal:
            return .primary
        case .muted:
            return .secondary
        case .strong:
            return .primary
        }
    }
}

private struct ModePill: View {
    let mode: EditorMode

    var body: some View {
        Text(label)
            .font(.system(size: 10, weight: .semibold, design: .rounded))
            .foregroundStyle(foreground)
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(
                Capsule(style: .continuous)
                    .fill(background)
            )
            .accessibilityLabel(label)
    }

    private var label: String {
        switch mode {
        case .normal:
            return "Normal"
        case .insert:
            return "Insert"
        case .select:
            return "Select"
        case .command:
            return "Command"
        }
    }

    private var foreground: Color {
        switch mode {
        case .normal:
            return .secondary
        case .insert:
            return .accentColor
        case .select:
            return .purple
        case .command:
            return .orange
        }
    }

    private var background: Color {
        switch mode {
        case .normal:
            return Color.secondary.opacity(0.12)
        case .insert:
            return Color.accentColor.opacity(0.14)
        case .select:
            return Color.purple.opacity(0.14)
        case .command:
            return Color.orange.opacity(0.14)
        }
    }
}

private struct EditorWindowChromeAccessor: NSViewRepresentable {
    let chrome: EditorChromeModel

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        view.isHidden = true
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        DispatchQueue.main.async {
            guard let window = nsView.window else { return }
            context.coordinator.configure(window: window, chrome: chrome)
        }
    }

    @MainActor
    final class Coordinator: NSObject, NSToolbarDelegate {
        private let toolbarIdentifier = NSToolbar.Identifier("TheSwiftPOC.TitlebarToolbar")
        private let documentItemIdentifier = NSToolbarItem.Identifier("TheSwiftPOC.DocumentInfo")
        private let vcsItemIdentifier = NSToolbarItem.Identifier("TheSwiftPOC.VCSInfo")
        private let documentHostingView = NSHostingView(rootView: EditorTitlebarDocumentView(document: .empty))
        private let vcsHostingView = NSHostingView(rootView: EditorTitlebarVCSView(vcsText: nil))
        private weak var observedWindow: NSWindow?
        private var lastChrome: EditorChromeModel = .empty
        private lazy var toolbar: NSToolbar = {
            let toolbar = NSToolbar(identifier: toolbarIdentifier)
            toolbar.delegate = self
            toolbar.displayMode = .iconOnly
            toolbar.allowsUserCustomization = false
            toolbar.autosavesConfiguration = false
            toolbar.showsBaselineSeparator = false
            return toolbar
        }()

        override init() {
            super.init()
            documentHostingView.translatesAutoresizingMaskIntoConstraints = false
            documentHostingView.setContentCompressionResistancePriority(.required, for: .horizontal)
            documentHostingView.setContentHuggingPriority(.required, for: .horizontal)
            vcsHostingView.translatesAutoresizingMaskIntoConstraints = false
            vcsHostingView.setContentCompressionResistancePriority(.required, for: .horizontal)
            vcsHostingView.setContentHuggingPriority(.required, for: .horizontal)
        }

        deinit {
            NotificationCenter.default.removeObserver(self)
        }

        func configure(window: NSWindow, chrome: EditorChromeModel) {
            attachWindowObserversIfNeeded(window: window)
            installToolbarIfNeeded(window: window)
            lastChrome = chrome
            applyWindowChrome(window: window, chrome: chrome)
            updateToolbarContent(window: window, chrome: chrome)
        }

        private func attachWindowObserversIfNeeded(window: NSWindow) {
            guard observedWindow !== window else { return }
            NotificationCenter.default.removeObserver(self)
            observedWindow = window
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(windowDidChangeState(_:)),
                name: NSWindow.didBecomeMainNotification,
                object: window
            )
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(windowDidChangeState(_:)),
                name: NSWindow.didResignMainNotification,
                object: window
            )
        }

        private func installToolbarIfNeeded(window: NSWindow) {
            if window.toolbar?.identifier != toolbarIdentifier {
                window.toolbar = toolbar
            }
            window.toolbarStyle = .unifiedCompact
        }

        @objc private func windowDidChangeState(_ notification: Notification) {
            guard let window = notification.object as? NSWindow else { return }
            applyWindowChrome(window: window, chrome: lastChrome)
            updateToolbarContent(window: window, chrome: lastChrome)
        }

        private func applyWindowChrome(window: NSWindow, chrome: EditorChromeModel) {
            let backgroundColor = chrome.backgroundColor
            window.titleVisibility = .hidden
            window.titlebarAppearsTransparent = false
            window.titlebarSeparatorStyle = .none
            window.toolbarStyle = .unifiedCompact
            window.backgroundColor = backgroundColor.withAlphaComponent(1)
            window.isDocumentEdited = chrome.document.isModified
            window.appearance = backgroundColor.isLightColor
                ? NSAppearance(named: .aqua)
                : NSAppearance(named: .darkAqua)

            let title = windowTitle(for: chrome)
            if window.title != title {
                window.title = title
            }

            if let absolutePath = chrome.document.absolutePath, !absolutePath.isEmpty {
                window.representedURL = URL(fileURLWithPath: absolutePath)
            } else {
                window.representedURL = nil
            }

            applyTitlebarBackground(window: window, color: backgroundColor)
        }

        private func updateToolbarContent(window: NSWindow, chrome: EditorChromeModel) {
            documentHostingView.rootView = EditorTitlebarDocumentView(document: chrome.document)
            vcsHostingView.rootView = EditorTitlebarVCSView(vcsText: chrome.document.vcsText)
            documentHostingView.invalidateIntrinsicContentSize()
            vcsHostingView.invalidateIntrinsicContentSize()
            window.toolbar?.validateVisibleItems()
        }

        private func applyTitlebarBackground(window: NSWindow, color: NSColor) {
            if #available(macOS 26.0, *) {
                if let titlebarView = titlebarView(for: window) {
                    titlebarView.wantsLayer = true
                    titlebarView.layer?.backgroundColor = color.cgColor
                }
                titlebarBackgroundView(for: window)?.isHidden = true
            } else {
                window.titlebarAppearsTransparent = true
                guard let titlebarContainer = titlebarContainer(for: window) else { return }
                titlebarContainer.wantsLayer = true
                titlebarContainer.layer?.backgroundColor = color.cgColor
                hideFirstEffectView(in: titlebarContainer)
            }
        }

        private func windowTitle(for chrome: EditorChromeModel) -> String {
            if let relativePath = chrome.document.relativePath, !relativePath.isEmpty {
                return "\(relativePath)/\(chrome.document.name)"
            }
            return chrome.document.name
        }

        private func titlebarContainer(for window: NSWindow) -> NSView? {
            if !window.styleMask.contains(.fullScreen) {
                guard let contentView = window.contentView else { return nil }
                return firstViewFromRoot(startingAt: contentView, classNameContains: "NSTitlebarContainerView")
            }

            for appWindow in NSApplication.shared.windows {
                guard NSStringFromClass(type(of: appWindow)).contains("NSToolbarFullScreenWindow") else { continue }
                guard appWindow.parent == window else { continue }
                guard let contentView = appWindow.contentView else { continue }
                return firstViewFromRoot(startingAt: contentView, classNameContains: "NSTitlebarContainerView")
            }
            return nil
        }

        private func titlebarView(for window: NSWindow) -> NSView? {
            titlebarContainer(for: window).flatMap { firstView(from: $0, classNameContains: "NSTitlebarView") }
        }

        private func titlebarBackgroundView(for window: NSWindow) -> NSView? {
            titlebarContainer(for: window).flatMap { firstView(from: $0, classNameContains: "NSTitlebarBackgroundView") }
        }

        private func firstViewFromRoot(startingAt view: NSView, classNameContains needle: String) -> NSView? {
            var root = view
            while let superview = root.superview {
                root = superview
            }
            return firstView(from: root, classNameContains: needle)
        }

        private func firstView(from root: NSView, classNameContains needle: String) -> NSView? {
            if NSStringFromClass(type(of: root)).contains(needle) {
                return root
            }
            for subview in root.subviews {
                if let match = firstView(from: subview, classNameContains: needle) {
                    return match
                }
            }
            return nil
        }

        private func hideFirstEffectView(in root: NSView) {
            if let effectView = firstEffectView(in: root) {
                effectView.isHidden = true
            }
        }

        private func firstEffectView(in root: NSView) -> NSVisualEffectView? {
            if let effectView = root as? NSVisualEffectView {
                return effectView
            }
            for subview in root.subviews {
                if let match = firstEffectView(in: subview) {
                    return match
                }
            }
            return nil
        }

        func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
            [documentItemIdentifier, .flexibleSpace, vcsItemIdentifier]
        }

        func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
            [documentItemIdentifier, .flexibleSpace, vcsItemIdentifier]
        }

        func toolbar(
            _ toolbar: NSToolbar,
            itemForItemIdentifier itemIdentifier: NSToolbarItem.Identifier,
            willBeInsertedIntoToolbar flag: Bool
        ) -> NSToolbarItem? {
            let item = NSToolbarItem(itemIdentifier: itemIdentifier)
            item.isBordered = false

            switch itemIdentifier {
            case documentItemIdentifier:
                item.view = documentHostingView
                item.visibilityPriority = .high
                return item
            case vcsItemIdentifier:
                item.view = vcsHostingView
                item.visibilityPriority = .low
                return item
            default:
                return nil
            }
        }
    }
}

private struct EditorTitlebarDocumentView: View {
    let document: EditorDocumentChrome

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: symbolName(for: document.icon, isDirectory: false))
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.secondary)

            Text(document.name)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.primary)
                .lineLimit(1)
        }
        .fixedSize()
        .allowsHitTesting(false)
        .accessibilityElement(children: .combine)
    }
}

private struct EditorTitlebarVCSView: View {
    let vcsText: String?

    private var trimmedVCSText: String? {
        let trimmed = vcsText?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let trimmed, !trimmed.isEmpty else { return nil }
        return trimmed
    }

    var body: some View {
        Group {
            if let trimmedVCSText {
                HStack(spacing: 6) {
                    Image(systemName: symbolName(for: "git_branch", isDirectory: false))
                        .font(.system(size: 11, weight: .semibold))
                    Text(trimmedVCSText)
                        .font(.system(size: 12, weight: .medium))
                        .lineLimit(1)
                }
                .foregroundStyle(.secondary)
                .fixedSize()
                .allowsHitTesting(false)
                .accessibilityElement(children: .combine)
            } else {
                Color.clear
                    .frame(width: 1, height: 1)
                    .allowsHitTesting(false)
                    .accessibilityHidden(true)
            }
        }
    }
}

private func chromeBackgroundColor(base: NSColor) -> NSColor {
    base.usingColorSpace(.sRGB) ?? base
}

private extension NSColor {
    var isLightColor: Bool {
        guard let color = usingColorSpace(.sRGB) else { return false }
        let luminance = (0.299 * color.redComponent) + (0.587 * color.greenComponent) + (0.114 * color.blueComponent)
        return luminance > 0.7
    }
}

private func symbolName(for icon: String, isDirectory: Bool) -> String {
    switch icon {
    case "folder", "folder_open", "folder_search":
        return isDirectory ? "folder.fill" : "folder"
    case "book":
        return "book.closed"
    case "swift":
        return "swift"
    case "rust", "file_rust":
        return "gearshape.2"
    case "file_markdown":
        return "doc.text"
    case "terminal":
        return "terminal"
    case "image":
        return "photo"
    case "json", "file_toml", "settings", "tool_hammer":
        return "doc.badge.gearshape"
    case "git_branch":
        return "point.topleft.down.curvedto.point.bottomright.up"
    case "diagnostic_error":
        return "xmark.circle.fill"
    case "diagnostic_warning":
        return "exclamationmark.triangle.fill"
    case "diagnostic_info":
        return "info.circle.fill"
    case "diagnostic_hint":
        return "lightbulb.fill"
    case "pi":
        return "sparkles"
    case "curlybraces", "textformat", "return":
        return icon
    case "copilot", "supermaven":
        return "wand.and.stars"
    case "copilot_disabled", "supermaven_disabled":
        return "slash.circle"
    case "copilot_init", "supermaven_init":
        return "arrow.triangle.2.circlepath"
    case "copilot_error", "supermaven_error":
        return "exclamationmark.circle"
    default:
        return isDirectory ? "folder" : "doc"
    }
}
