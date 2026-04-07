import Foundation

func themePerfEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_THEME_PROFILE"] == "1"
}

func completionPerfEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_COMPLETION_PERF"] == "1"
}

func scrollPerfEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_SCROLL_PERF"] == "1"
}

func themePerfLog(_ message: @autoclosure () -> String) {
    guard themePerfEnabled() else { return }
    fputs("[TheSwiftPOC:perf] \(message())\n", stderr)
}

func completionPerfLog(_ message: @autoclosure () -> String) {
    guard completionPerfEnabled() else { return }
    fputs("[TheSwiftPOC:completion-perf] \(message())\n", stderr)
}

func scrollPerfLog(_ message: @autoclosure () -> String) {
    guard scrollPerfEnabled() else { return }
    let tsMs = Int((CFAbsoluteTimeGetCurrent() * 1000).rounded())
    fputs("[TheSwiftPOC:scroll-perf \(tsMs)] \(message())\n", stderr)
}

@discardableResult
func measureThemePerf<T>(_ label: String, _ body: () -> T) -> T {
    let start = CFAbsoluteTimeGetCurrent()
    let result = body()
    let elapsedMs = (CFAbsoluteTimeGetCurrent() - start) * 1000
    themePerfLog("\(label) ms=\(String(format: "%.2f", elapsedMs))")
    return result
}

@discardableResult
func measureCompletionPerf<T>(_ label: String, _ body: () -> T) -> T {
    let start = CFAbsoluteTimeGetCurrent()
    let result = body()
    let elapsedMs = (CFAbsoluteTimeGetCurrent() - start) * 1000
    completionPerfLog("\(label) ms=\(String(format: "%.2f", elapsedMs))")
    return result
}

@discardableResult
func measureScrollPerf<T>(_ label: String, _ body: () -> T) -> T {
    let start = CFAbsoluteTimeGetCurrent()
    let result = body()
    let elapsedMs = (CFAbsoluteTimeGetCurrent() - start) * 1000
    scrollPerfLog("\(label) ms=\(String(format: "%.2f", elapsedMs))")
    return result
}
