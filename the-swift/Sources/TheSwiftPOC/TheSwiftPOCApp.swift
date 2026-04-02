import AppKit
import SwiftUI

final class TheSwiftPOCAppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        NSApp.windows.first?.makeKeyAndOrderFront(nil)
    }
}

@main
struct TheSwiftPOCApp: App {
    @NSApplicationDelegateAdaptor(TheSwiftPOCAppDelegate.self) private var appDelegate
    private let initialPath = ProcessInfo.processInfo.arguments
        .dropFirst()
        .first(where: { $0 != "--" })

    var body: some Scene {
        WindowGroup {
            EditorContainerView(initialPath: initialPath)
                .frame(minWidth: 900, minHeight: 600)
        }
        .defaultSize(width: 900, height: 600)
        .windowResizability(.contentSize)
    }
}

private struct EditorContainerView: View {
    let initialPath: String?

    var body: some View {
        RustEditorRepresentable(initialPath: initialPath)
    }
}
