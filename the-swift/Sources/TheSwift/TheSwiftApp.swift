import SwiftUI
import AppKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
    }
}

@main
struct TheSwiftApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate

    var body: some Scene {
        let filePath = Self.firstFileArgument()
        WindowGroup {
            EditorView(filePath: filePath)
                .frame(minWidth: 640, minHeight: 360)
        }
    }

    private static func firstFileArgument() -> String? {
        let args = CommandLine.arguments.dropFirst()
        var iter = args.makeIterator()
        while let arg = iter.next() {
            if arg == "--" {
                return iter.next()
            }
            if arg.hasPrefix("-") {
                continue
            }
            return arg
        }
        return nil
    }
}
