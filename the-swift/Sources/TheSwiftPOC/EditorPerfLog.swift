import Foundation
import os.signpost

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

func agentPerfEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_AGENT_PERF"] == "1"
}

func agentDebugEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_AGENT_DEBUG"] == "1"
}

func selectionDebugEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_SELECTION_DEBUG"] == "1"
}

func layoutDebugEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_LAYOUT_DEBUG"] == "1"
}

func sidebarPerfEnabled() -> Bool {
    ProcessInfo.processInfo.environment["THE_EDITOR_SIDEBAR_PERF"] == "1"
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

func agentPerfLog(_ message: @autoclosure () -> String) {
    guard agentPerfEnabled() else { return }
    let tsMs = Int((CFAbsoluteTimeGetCurrent() * 1000).rounded())
    let line = "[TheSwiftPOC:agent-perf \(tsMs)] \(message())\n"
    fputs(line, stderr)
    appendPerfLineToSharedLogFile(line)
}

func agentDebugLog(_ message: @autoclosure () -> String) {
    guard agentDebugEnabled() else { return }
    let tsMs = Int((CFAbsoluteTimeGetCurrent() * 1000).rounded())
    let line = "[TheSwiftPOC:agent \(tsMs)] \(message())\n"
    fputs(line, stderr)
    appendPerfLineToSharedLogFile(line)
}

func selectionDebugLog(_ message: @autoclosure () -> String) {
    guard selectionDebugEnabled() else { return }
    let tsMs = Int((CFAbsoluteTimeGetCurrent() * 1000).rounded())
    let line = "[TheSwiftPOC:selection \(tsMs)] \(message())\n"
    fputs(line, stderr)
    appendPerfLineToSharedLogFile(line)
}

func layoutDebugLog(_ message: @autoclosure () -> String) {
    guard layoutDebugEnabled() else { return }
    let tsMs = Int((CFAbsoluteTimeGetCurrent() * 1000).rounded())
    let line = "[TheSwiftPOC:layout \(tsMs)] \(message())\n"
    fputs(line, stderr)
    appendPerfLineToSharedLogFile(line)
}

func sidebarPerfLog(_ message: @autoclosure () -> String) {
    guard sidebarPerfEnabled() else { return }
    let tsMs = Int((CFAbsoluteTimeGetCurrent() * 1000).rounded())
    let line = "[TheSwiftPOC:sidebar-perf \(tsMs)] \(message())\n"
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

@discardableResult
func measureAgentPerf<T>(_ label: String, _ body: () -> T) -> T {
    let start = CFAbsoluteTimeGetCurrent()
    let result = body()
    let elapsedMs = (CFAbsoluteTimeGetCurrent() - start) * 1000
    agentPerfLog("\(label) ms=\(String(format: "%.2f", elapsedMs))")
    return result
}

private struct AgentPerfDurationStats {
    var count: Int = 0
    var totalMs: Double = 0
    var maxMs: Double = 0
}

private final class AgentPerfTelemetry: @unchecked Sendable {
    static let shared = AgentPerfTelemetry()

    private let lock = NSLock()
    private var bucketStartedAt = CFAbsoluteTimeGetCurrent()
    private var counters: [String: Int] = [:]
    private var durations: [String: AgentPerfDurationStats] = [:]

    func increment(_ key: String, by amount: Int = 1) {
        guard agentPerfEnabled() else { return }
        let line = withLockedState(key: key, amount: amount, durationKey: nil, durationMs: nil)
        if let line {
            agentPerfLog(line)
        }
    }

    func recordDuration(_ key: String, ms: Double) {
        guard agentPerfEnabled() else { return }
        let line = withLockedState(key: nil, amount: 0, durationKey: key, durationMs: ms)
        if let line {
            agentPerfLog(line)
        }
    }

    private func withLockedState(key: String?, amount: Int, durationKey: String?, durationMs: Double?) -> String? {
        lock.lock()
        defer { lock.unlock() }

        let now = CFAbsoluteTimeGetCurrent()
        let line = flushLineIfNeeded(now: now)

        if let key {
            counters[key, default: 0] += amount
        }
        if let durationKey, let durationMs {
            var stats = durations[durationKey, default: AgentPerfDurationStats()]
            stats.count += 1
            stats.totalMs += durationMs
            stats.maxMs = max(stats.maxMs, durationMs)
            durations[durationKey] = stats
        }

        return line
    }

    private func flushLineIfNeeded(now: CFAbsoluteTime) -> String? {
        let elapsed = now - bucketStartedAt
        guard elapsed >= 1, !counters.isEmpty || !durations.isEmpty else { return nil }

        let counterSummary = counters.keys.sorted().map { "\($0)=\(counters[$0] ?? 0)" }
        let durationSummary = durations.keys.sorted().map { key -> String in
            let stats = durations[key] ?? AgentPerfDurationStats()
            let averageMs = stats.count > 0 ? stats.totalMs / Double(stats.count) : 0
            return "\(key){count=\(stats.count),totalMs=\(String(format: "%.2f", stats.totalMs)),avgMs=\(String(format: "%.2f", averageMs)),maxMs=\(String(format: "%.2f", stats.maxMs))}"
        }

        counters.removeAll(keepingCapacity: true)
        durations.removeAll(keepingCapacity: true)
        bucketStartedAt = now

        let windowMs = Int((elapsed * 1000).rounded())
        let components = (["bucket.windowMs=\(windowMs)"] + counterSummary + durationSummary)
        return components.joined(separator: " ")
    }
}

private let agentSignpostLog = OSLog(subsystem: "TheSwiftPOC", category: .pointsOfInterest)

func agentPerfIncrement(_ key: String, by amount: Int = 1) {
    AgentPerfTelemetry.shared.increment(key, by: amount)
}

func agentPerfRecordDuration(_ key: String, ms: Double) {
    AgentPerfTelemetry.shared.recordDuration(key, ms: ms)
}

@discardableResult
func measureAgentSignpostedInterval<T>(_ signpostName: StaticString, counterKey: String, _ body: () -> T) -> T {
    guard agentPerfEnabled() else { return body() }
    let signpostID = OSSignpostID(log: agentSignpostLog)
    os_signpost(.begin, log: agentSignpostLog, name: signpostName, signpostID: signpostID)
    let start = CFAbsoluteTimeGetCurrent()
    let result = body()
    let elapsedMs = (CFAbsoluteTimeGetCurrent() - start) * 1000
    os_signpost(.end, log: agentSignpostLog, name: signpostName, signpostID: signpostID, "ms=%{public}.2f", elapsedMs)
    agentPerfRecordDuration(counterKey, ms: elapsedMs)
    return result
}

func agentPerfSignpostEvent(_ name: StaticString) {
    guard agentPerfEnabled() else { return }
    os_signpost(.event, log: agentSignpostLog, name: name)
}

private struct SidebarPerfDurationStats {
    var count: Int = 0
    var totalMs: Double = 0
    var maxMs: Double = 0
}

private final class SidebarPerfTelemetry: @unchecked Sendable {
    static let shared = SidebarPerfTelemetry()

    private let lock = NSLock()
    private var bucketStartedAt = CFAbsoluteTimeGetCurrent()
    private var counters: [String: Int] = [:]
    private var durations: [String: SidebarPerfDurationStats] = [:]

    func increment(_ key: String, by amount: Int = 1) {
        guard sidebarPerfEnabled() else { return }
        let line = withLockedState(key: key, amount: amount, durationKey: nil, durationMs: nil)
        if let line {
            sidebarPerfLog(line)
        }
    }

    func recordDuration(_ key: String, ms: Double) {
        guard sidebarPerfEnabled() else { return }
        let line = withLockedState(key: nil, amount: 0, durationKey: key, durationMs: ms)
        if let line {
            sidebarPerfLog(line)
        }
    }

    private func withLockedState(key: String?, amount: Int, durationKey: String?, durationMs: Double?) -> String? {
        lock.lock()
        defer { lock.unlock() }

        let now = CFAbsoluteTimeGetCurrent()
        let line = flushLineIfNeeded(now: now)

        if let key {
            counters[key, default: 0] += amount
        }
        if let durationKey, let durationMs {
            var stats = durations[durationKey, default: SidebarPerfDurationStats()]
            stats.count += 1
            stats.totalMs += durationMs
            stats.maxMs = max(stats.maxMs, durationMs)
            durations[durationKey] = stats
        }

        return line
    }

    private func flushLineIfNeeded(now: CFAbsoluteTime) -> String? {
        let elapsed = now - bucketStartedAt
        guard elapsed >= 1, !counters.isEmpty || !durations.isEmpty else { return nil }

        let counterSummary = counters.keys.sorted().map { "\($0)=\(counters[$0] ?? 0)" }
        let durationSummary = durations.keys.sorted().map { key -> String in
            let stats = durations[key] ?? SidebarPerfDurationStats()
            let averageMs = stats.count > 0 ? stats.totalMs / Double(stats.count) : 0
            return "\(key){count=\(stats.count),totalMs=\(String(format: "%.2f", stats.totalMs)),avgMs=\(String(format: "%.2f", averageMs)),maxMs=\(String(format: "%.2f", stats.maxMs))}"
        }

        counters.removeAll(keepingCapacity: true)
        durations.removeAll(keepingCapacity: true)
        bucketStartedAt = now

        let windowMs = Int((elapsed * 1000).rounded())
        let components = (["bucket.windowMs=\(windowMs)"] + counterSummary + durationSummary)
        return components.joined(separator: " ")
    }
}

private let sidebarSignpostLog = OSLog(subsystem: "TheSwiftPOC", category: "SidebarPerf")

func sidebarPerfIncrement(_ key: String, by amount: Int = 1) {
    SidebarPerfTelemetry.shared.increment(key, by: amount)
}

func sidebarPerfRecordDuration(_ key: String, ms: Double) {
    SidebarPerfTelemetry.shared.recordDuration(key, ms: ms)
}

@discardableResult
func measureSidebarSignpostedInterval<T>(_ signpostName: StaticString, counterKey: String, _ body: () -> T) -> T {
    guard sidebarPerfEnabled() else { return body() }
    let signpostID = OSSignpostID(log: sidebarSignpostLog)
    os_signpost(.begin, log: sidebarSignpostLog, name: signpostName, signpostID: signpostID)
    let start = CFAbsoluteTimeGetCurrent()
    let result = body()
    let elapsedMs = (CFAbsoluteTimeGetCurrent() - start) * 1000
    os_signpost(.end, log: sidebarSignpostLog, name: signpostName, signpostID: signpostID, "ms=%{public}.2f", elapsedMs)
    sidebarPerfRecordDuration(counterKey, ms: elapsedMs)
    return result
}
