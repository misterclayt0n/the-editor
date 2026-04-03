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
        CommandGroup(after: .toolbar) {
            Button("Command Palette") {
                controller.toggleCommandPalette()
            }
            .keyboardShortcut("p", modifiers: [.command, .shift])
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
        .windowResizability(.contentSize)
        .commands {
            EditorCommandMenu(controller: model.controller)
        }
    }
}

private struct EditorContainerView: View {
    @ObservedObject var controller: EditorSurfaceController

    var body: some View {
        ZStack {
            RustEditorRepresentable(controller: controller)
            EditorCommandPaletteView(controller: controller)
            EditorFilePickerView(controller: controller)
        }
    }
}
