import Foundation

private func appendPerfLineToSharedLogFile(_ line: String) {
    guard let rawPath = ProcessInfo.processInfo.environment["THE_TERM_DEBUG_RENDER_PERF_FILE"]?.trimmingCharacters(in: .whitespacesAndNewlines),
          !rawPath.isEmpty else {
        return
    }
    let url = URL(fileURLWithPath: rawPath)
    let directory = url.deletingLastPathComponent()
    try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let data = Data(line.utf8)
    if FileManager.default.fileExists(atPath: url.path),
       let handle = try? FileHandle(forWritingTo: url) {
        defer { try? handle.close() }
        try? handle.seekToEnd()
        try? handle.write(contentsOf: data)
    } else {
        try? data.write(to: url)
    }
}

func themePerfEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_THEME_PROFILE"] == "1"
}

func completionPerfEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_COMPLETION_PERF"] == "1"
}

func scrollPerfEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_SCROLL_PERF"] == "1"
}

func agentFollowDebugEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_AGENT_FOLLOW_DEBUG"] == "1"
}

func themePerfLog(_ message: @autoclosure () -> String) {
    guard themePerfEnabled() else { return }
    let line = "[TheSwiftPOC:perf] \(message())\n"
    fputs(line, stderr)
    appendPerfLineToSharedLogFile(line)
}

func completionPerfLog(_ message: @autoclosure () -> String) {
    guard completionPerfEnabled() else { return }
    let line = "[TheSwiftPOC:completion-perf] \(message())\n"
    fputs(line, stderr)
    appendPerfLineToSharedLogFile(line)
}

func scrollPerfLog(_ message: @autoclosure () -> String) {
    guard scrollPerfEnabled() else { return }
    let tsMs = Int((CFAbsoluteTimeGetCurrent() * 1000).rounded())
    let line = "[TheSwiftPOC:scroll-perf \(tsMs)] \(message())\n"
    fputs(line, stderr)
    appendPerfLineToSharedLogFile(line)
}

func agentFollowDebugLog(_ message: @autoclosure () -> String) {
    guard agentFollowDebugEnabled() else { return }
    let tsMs = Int((CFAbsoluteTimeGetCurrent() * 1000).rounded())
    let line = "[TheSwiftPOC:agent-follow \(tsMs)] \(message())\n"
    fputs(line, stderr)
    appendPerfLineToSharedLogFile(line)
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
