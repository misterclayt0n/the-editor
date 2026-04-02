import AppKit
import Foundation

func editorDebugLog(_ message: @autoclosure () -> String) {
    guard ProcessInfo.processInfo.environment["THE_EDITOR_SWIFT_DEBUG_VIEWPORT"] == "1" else {
        return
    }
    fputs("[TheSwiftPOC] \(message())\n", stderr)
}

func editorDebugDescribe(_ rect: CGRect) -> String {
    "(x:\(Int(rect.origin.x)), y:\(Int(rect.origin.y)), w:\(Int(rect.size.width)), h:\(Int(rect.size.height)))"
}

func editorDebugDescribe(_ size: CGSize) -> String {
    "(w:\(Int(size.width)), h:\(Int(size.height)))"
}
