import AppKit
import Foundation
import SwiftUI

final class GlobalTerminalSwitcherController: ObservableObject {
    static let shared = GlobalTerminalSwitcherController()

    private struct Session: Equatable {
        let ownerWindowNumber: Int
        let sessionId: UUID
        let items: [GlobalTerminalSurfaceEntry]
    }

    @Published private var session: Session? = nil

    private init() {}

    func snapshot(for window: NSWindow?) -> GlobalTerminalSwitcherSnapshot {
        guard let window,
              let session,
              session.ownerWindowNumber == window.windowNumber else {
            return .closed
        }
        return GlobalTerminalSwitcherSnapshot(
            isOpen: true,
            sessionId: session.sessionId,
            items: session.items
        )
    }

    func toggle(ownerWindow: NSWindow?, items: [GlobalTerminalSurfaceEntry]) -> Bool {
        guard let ownerWindow else {
            return false
        }
        if let session, session.ownerWindowNumber == ownerWindow.windowNumber {
            close(ownerWindow: ownerWindow)
        } else {
            open(ownerWindow: ownerWindow, items: items)
        }
        return true
    }

    func open(ownerWindow: NSWindow, items: [GlobalTerminalSurfaceEntry]) {
        session = Session(
            ownerWindowNumber: ownerWindow.windowNumber,
            sessionId: UUID(),
            items: items
        )
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "terminal.switcher.open window=\(ownerWindow.windowNumber) entries=\(items.count)"
            )
        }
    }

    func close(ownerWindow: NSWindow? = nil) {
        guard let session else {
            return
        }
        if let ownerWindow,
           session.ownerWindowNumber != ownerWindow.windowNumber {
            return
        }
        self.session = nil
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "terminal.switcher.close window=\(session.ownerWindowNumber)"
            )
        }
    }
}
