import Foundation

func gutterDebugEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_GUTTER_DEBUG"] == "1"
}

func gutterDebugLog(_ message: @autoclosure () -> String) {
    guard gutterDebugEnabled() else { return }
    fputs("[TheSwiftPOC:gutter] \(message())\n", stderr)
}
