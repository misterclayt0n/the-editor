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
        }

        CommandGroup(after: .toolbar) {
            Button("Command Palette") {
                controller.toggleCommandPalette()
            }
            .keyboardShortcut("p", modifiers: [.command, .shift])

            Divider()

            Button("Toggle PI Sidebar") {
                controller.togglePiSidebar()
            }
            .keyboardShortcut("i", modifiers: [.command, .shift])

            Button("New Terminal") {
                controller.openTerminalInActivePane()
            }
            .keyboardShortcut("t", modifiers: [.command, .shift])

            Button("Close Active Terminal") {
                controller.closeTerminalInActivePane()
            }
        }
    }
}

@main
struct TheSwiftPOCApp: App {
    @NSApplicationDelegateAdaptor(TheSwiftPOCAppDelegate.self) private var appDelegate
    @StateObject private var model: EditorAppModel

    init() {
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
        .defaultSize(width: 900, height: 600)
        .commands {
            EditorCommandMenu(controller: model.controller)
        }
    }
}

private struct EditorContainerView: View {
    @ObservedObject var controller: EditorSurfaceController

    var body: some View {
        ZStack {
            EditorChromeView(controller: controller)
            EditorResizeOverlayView(controller: controller)
            EditorInputPromptView(controller: controller)
            EditorCommandPaletteView(controller: controller)
            EditorFilePickerView(controller: controller)
        }
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
