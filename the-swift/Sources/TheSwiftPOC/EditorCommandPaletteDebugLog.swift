import Foundation

func commandPaletteDebugEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_COMMAND_PALETTE_DEBUG"] == "1"
}

func commandPaletteDebugLog(_ message: @autoclosure () -> String) {
    guard commandPaletteDebugEnabled() else { return }
    fputs("[TheSwiftPOC:command-palette] \(message())\n", stderr)
}
