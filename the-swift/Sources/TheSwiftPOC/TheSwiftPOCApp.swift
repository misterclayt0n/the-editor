import AppKit
import SwiftUI

final class TheSwiftPOCAppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        NSApp.windows.first?.makeKeyAndOrderFront(nil)
    }
}

@MainActor
final class EditorAppModel: ObservableObject {
    let controller: EditorSurfaceController

    init(initialPath: String?) {
        self.controller = EditorSurfaceController(initialPath: initialPath)
    }
}

struct EditorCommandMenu: Commands {
    let controller: EditorSurfaceController

    var body: some Commands {
        CommandGroup(after: .textEditing) {
            Button("Find") {
                controller.openSearchPrompt()
            }
            .keyboardShortcut("f", modifiers: [.command])

            Button("Find Next") {
                controller.stepInputPromptNext()
            }
            .keyboardShortcut("g", modifiers: [.command])

            Button("Find Previous") {
                controller.stepInputPromptPrevious()
            }
            .keyboardShortcut("g", modifiers: [.command, .shift])

            Button("Close Find") {
                controller.closeInputPrompt()
            }
            .keyboardShortcut(.escape, modifiers: [])

            Divider()

            Button("Increase Buffer Font Size") {
                controller.increaseBufferFontSize()
            }
            .keyboardShortcut("=", modifiers: [.command])

            Button("Decrease Buffer Font Size") {
                controller.decreaseBufferFontSize()
            }
            .keyboardShortcut("-", modifiers: [.command])

            Button("Reset Buffer Font Size") {
                controller.resetBufferFontSize()
            }
            .keyboardShortcut("0", modifiers: [.command])
        }

        CommandGroup(after: .toolbar) {
            Button("Command Palette") {
                controller.toggleCommandPalette()
            }
            .keyboardShortcut("p", modifiers: [.command, .shift])

            Divider()

            Button("Split Vertically") {
                controller.splitActivePaneVertical()
            }
            .keyboardShortcut("d", modifiers: [.command])

            Button("Split Horizontally") {
                controller.splitActivePaneHorizontal()
            }
            .keyboardShortcut("d", modifiers: [.command, .shift])

            Divider()

            Button("New Terminal") {
                controller.openTerminalInActivePane()
            }
            .keyboardShortcut("t", modifiers: [.command, .shift])

            Button("Show Agent") {
                controller.openAgentInActivePane()
            }
            .keyboardShortcut("a", modifiers: [.command, .shift])

            Button("Close Active Terminal") {
                controller.closeTerminalInActivePane()
            }
        }

        CommandGroup(replacing: .windowArrangement) {
            Button("Close Current Surface") {
                controller.closeActivePaneItem()
            }
            .keyboardShortcut("w", modifiers: [.command])

            Divider()

            Button("Minimize") {
                NSApp.keyWindow?.miniaturize(nil)
            }
            .keyboardShortcut("m", modifiers: [.command])

            Button("Zoom") {
                NSApp.keyWindow?.zoom(nil)
            }

            Divider()

            Button("Bring All to Front") {
                NSApp.arrangeInFront(nil)
            }
        }

        CommandGroup(replacing: .appTermination) {
            Button("Quit the-editor") {
                controller.quitApplication()
            }
            .keyboardShortcut("q", modifiers: [.command])
        }
    }
}

@main
struct TheSwiftPOCApp: App {
    @NSApplicationDelegateAdaptor(TheSwiftPOCAppDelegate.self) private var appDelegate
    @StateObject private var model: EditorAppModel

    init() {
        EditorIconFont.registerIfNeeded()
        let initialPath = ProcessInfo.processInfo.arguments
            .dropFirst()
            .first(where: { $0 != "--" })
        _model = StateObject(wrappedValue: EditorAppModel(initialPath: initialPath))
    }

    var body: some Scene {
        WindowGroup {
            EditorContainerView(controller: model.controller)
                .frame(minWidth: 900, minHeight: 600)
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 900, height: 600)
        .commands {
            EditorCommandMenu(controller: model.controller)
        }
    }
}

private struct EditorContainerView: View {
    @ObservedObject var controller: EditorSurfaceController

    private var overlayColorScheme: ColorScheme {
        let bg = controller.chrome.backgroundColor.usingColorSpace(.sRGB) ?? controller.chrome.backgroundColor
        let luminance = (0.299 * bg.redComponent) + (0.587 * bg.greenComponent) + (0.114 * bg.blueComponent)
        return luminance >= 0.6 ? .light : .dark
    }

    var body: some View {
        ZStack {
            EditorChromeView(controller: controller)
            EditorResizeOverlayView(controller: controller)
            EditorInputPromptView(controller: controller)
            EditorCommandPaletteView(controller: controller)
            EditorFilePickerView(controller: controller)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(nsColor: controller.chrome.backgroundColor).ignoresSafeArea())
        .background(EditorRootLayoutDebugView(controller: controller))
        .ignoresSafeArea()
        .environment(\.colorScheme, overlayColorScheme)
    }
}

private struct EditorRootLayoutDebugView: View {
    @ObservedObject var controller: EditorSurfaceController

    var body: some View {
        GeometryReader { geometry in
            let viewportText: String = {
                guard let scene = controller.scene else { return "nil" }
                return "\(scene.info.viewportWidth)x\(scene.info.viewportHeight)"
            }()
            let signature = [
                "root",
                String(format: "size=%.1fx%.1f", geometry.size.width, geometry.size.height),
                String(format: "safeArea=top:%.1f leading:%.1f bottom:%.1f trailing:%.1f", geometry.safeAreaInsets.top, geometry.safeAreaInsets.leading, geometry.safeAreaInsets.bottom, geometry.safeAreaInsets.trailing),
                "viewport=\(viewportText)",
                "fileTreeVisible=\(controller.fileTree.isVisible)",
                "buffers=\(controller.bufferTabs.tabs.count)",
                "openItemsGroups=\(controller.openItems.groups.count)"
            ].joined(separator: " ")

            Color.clear
                .onAppear {
                    layoutDebugLog(signature)
                }
                .onChange(of: signature) { _, newValue in
                    layoutDebugLog(newValue)
                }
        }
        .allowsHitTesting(false)
    }
}

private struct EditorResizeOverlayView: View {
    @ObservedObject var controller: EditorSurfaceController

    private var sizeLabel: String? {
        guard controller.showsResizeOverlay, let scene = controller.scene else { return nil }
        return "\(scene.info.viewportWidth) × \(scene.info.viewportHeight)"
    }

    var body: some View {
        Group {
            if let sizeLabel {
                Text(sizeLabel)
                    .font(.system(size: 13, weight: .medium, design: .rounded))
                    .foregroundStyle(.primary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(
                        RoundedRectangle(cornerRadius: 8, style: .continuous)
                            .fill(.regularMaterial)
                            .shadow(color: .black.opacity(0.18), radius: 10, y: 4)
                    )
                    .allowsHitTesting(false)
                    .transition(.opacity)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
        .animation(.easeOut(duration: 0.15), value: sizeLabel)
    }
}
