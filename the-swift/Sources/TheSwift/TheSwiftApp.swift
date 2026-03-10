import SwiftUI
import AppKit
import UserNotifications

final class AppDelegate: NSObject, NSApplicationDelegate, UNUserNotificationCenterDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSWindow.allowsAutomaticWindowTabbing = true
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        if EditorNotificationPlatformSupport.supportsUserNotifications {
            UNUserNotificationCenter.current().delegate = self
            EditorSystemNotificationManager.shared.prepareAuthorizationIfNeeded()
        }
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        if EditorNotificationPlatformSupport.supportsUserNotifications {
            EditorSystemNotificationManager.shared.prepareAuthorizationIfNeeded()
        }
        EditorSystemNotificationManager.shared.clearDeliveredNotifications()
    }

    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.banner, .list, .sound])
    }

    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        NSApp.activate(ignoringOtherApps: true)
        completionHandler()
    }
}

@main
struct TheSwiftApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    static let editorWindowSceneId = "editor-window"

    var body: some Scene {
        WindowGroup(id: Self.editorWindowSceneId, for: EditorWindowRoute.self) { route in
            EditorWindowSceneRoot(route: route)
                .environment(\.font, FontLoader.uiFont(size: 13))
                .frame(minWidth: 640, minHeight: 360)
        }
        .commands {
            EditorAppCommands()
        }
    }

    private static var initialFileArgumentConsumed = false

    fileprivate static func consumeFirstFileArgumentIfNeeded() -> String? {
        if initialFileArgumentConsumed {
            return nil
        }
        initialFileArgumentConsumed = true
        return firstFileArgument()
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

private struct EditorWindowSceneRoot: View {
    @Binding var route: EditorWindowRoute?
    @State private var fallbackResolved = false
    @State private var fallbackFilePath: String? = nil
    @State private var fallbackResolutionScheduled = false

    init(route: Binding<EditorWindowRoute?>) {
        self._route = route
    }

    var body: some View {
        Group {
            if let route {
                EditorView(filePath: route.filePath, windowRoute: route)
                    .id("route-\(route.requestId.uuidString)")
            } else if fallbackResolved {
                EditorView(filePath: fallbackFilePath, windowRoute: nil)
                    .id("fallback-\(fallbackFilePath ?? "<nil>")")
            } else {
                Color.clear
                    .onAppear {
                        scheduleFallbackResolutionIfNeeded()
                    }
                    .onChange(of: route?.requestId) { _ in
                        scheduleFallbackResolutionIfNeeded()
                    }
            }
        }
    }

    private func scheduleFallbackResolutionIfNeeded() {
        guard route == nil else {
            fallbackResolved = true
            return
        }
        guard !fallbackResolved, !fallbackResolutionScheduled else {
            return
        }

        fallbackResolutionScheduled = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.01) {
            fallbackResolutionScheduled = false
            if route != nil {
                fallbackResolved = true
                return
            }
            if SwiftWindowTabsCoordinator.shared.hasPendingTabOpenRequests {
                scheduleFallbackResolutionIfNeeded()
                return
            }
            fallbackFilePath = TheSwiftApp.consumeFirstFileArgumentIfNeeded()
            fallbackResolved = true
        }
    }
}
