import Foundation

func themePerfEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_THEME_PROFILE"] == "1"
}

func themePerfLog(_ message: @autoclosure () -> String) {
    guard themePerfEnabled() else { return }
    fputs("[TheSwiftPOC:perf] \(message())\n", stderr)
}

@discardableResult
func measureThemePerf<T>(_ label: String, _ body: () -> T) -> T {
    let start = CFAbsoluteTimeGetCurrent()
    let result = body()
    let elapsedMs = (CFAbsoluteTimeGetCurrent() - start) * 1000
    themePerfLog("\(label) ms=\(String(format: "%.2f", elapsedMs))")
    return result
}
