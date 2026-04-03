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
    final class Coordinator {
        private let accessoryController = NSTitlebarAccessoryViewController()
        private let accessoryHostingView = NSHostingView(rootView: AnyView(EmptyView()))
        private let accessoryIdentifier = NSUserInterfaceItemIdentifier("TheSwiftPOC.VCSAccessory")

        init() {
            accessoryController.layoutAttribute = .right
            accessoryController.identifier = accessoryIdentifier
            accessoryController.view = accessoryHostingView
            accessoryHostingView.translatesAutoresizingMaskIntoConstraints = false
        }

        func configure(window: NSWindow, chrome: EditorChromeModel) {
            applyWindowChrome(window: window, chrome: chrome)
            applyVCSAccessory(window: window, chrome: chrome)
        }

        private func applyWindowChrome(window: NSWindow, chrome: EditorChromeModel) {
            let backgroundColor = chrome.backgroundColor
            window.titleVisibility = .visible
            window.titlebarAppearsTransparent = true
            window.titlebarSeparatorStyle = .none
            window.toolbarStyle = .unifiedCompact
            window.backgroundColor = backgroundColor
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

            if let titlebarView = titlebarView(for: window) {
                titlebarView.wantsLayer = true
                titlebarView.layer?.backgroundColor = backgroundColor.cgColor
            }
        }

        private func applyVCSAccessory(window: NSWindow, chrome: EditorChromeModel) {
            accessoryHostingView.rootView = AnyView(EditorTitlebarVCSView(chrome: chrome))
            accessoryHostingView.layoutSubtreeIfNeeded()

            let shouldShow = !(chrome.document.vcsText?.isEmpty ?? true)
            let existingIndex = window.titlebarAccessoryViewControllers.firstIndex(where: {
                $0.identifier == accessoryIdentifier
            })

            if shouldShow {
                if existingIndex == nil {
                    window.addTitlebarAccessoryViewController(accessoryController)
                }
            } else if let existingIndex {
                window.removeTitlebarAccessoryViewController(at: existingIndex)
            }
        }

        private func windowTitle(for chrome: EditorChromeModel) -> String {
            if let relativePath = chrome.document.relativePath, !relativePath.isEmpty {
                return "\(relativePath)/\(chrome.document.name)"
            }
            return chrome.document.name
        }

        private func titlebarView(for window: NSWindow) -> NSView? {
            guard let themeFrame = window.contentView?.superview else { return nil }
            if themeFrame.responds(to: Selector(("titlebarView"))) {
                return themeFrame.value(forKey: "titlebarView") as? NSView
            }
            return themeFrame.subviews.first(where: { NSStringFromClass(type(of: $0)).contains("Titlebar") })
        }
    }
}

private struct EditorTitlebarVCSView: View {
    let chrome: EditorChromeModel

    var body: some View {
        Group {
            if let vcsText = chrome.document.vcsText, !vcsText.isEmpty {
                HStack(spacing: 6) {
                    Image(systemName: symbolName(for: "git_branch", isDirectory: false))
                        .font(.system(size: 11, weight: .semibold))
                    Text(vcsText)
                        .font(.system(size: 12, weight: .medium))
                        .lineLimit(1)
                }
                .foregroundStyle(.secondary)
                .padding(.horizontal, 10)
                .padding(.vertical, 4)
            }
        }
        .fixedSize()
    }
}

private func chromeBackgroundColor(base: NSColor) -> NSColor {
    let window = NSColor.windowBackgroundColor.usingColorSpace(.sRGB) ?? .windowBackgroundColor
    let editor = base.usingColorSpace(.sRGB) ?? base
    return window.blended(withFraction: 0.72, of: editor) ?? editor
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
