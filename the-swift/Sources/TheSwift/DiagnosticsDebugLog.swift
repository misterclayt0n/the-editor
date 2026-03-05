import Foundation

enum DiagnosticsDebugLog {
    private static let enabledFlag = ProcessInfo.processInfo.environment["THE_SWIFT_DEBUG_DIAGNOSTICS"] == "1"
    private static let pickerPerfEnabledFlag = ProcessInfo.processInfo.environment["THE_SWIFT_DEBUG_PICKER_PERF"] == "1"
    private static let terminalMouseEnabledFlag = ProcessInfo.processInfo.environment["THE_SWIFT_DEBUG_TERMINAL_MOUSE"] == "1"
    private static let diagnosticsFilePath: String? = {
        guard let raw = ProcessInfo.processInfo.environment["THE_SWIFT_DEBUG_DIAGNOSTICS_FILE"] else {
            return nil
        }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }()
    private static let lock = NSLock()
    private static let startNanos = DispatchTime.now().uptimeNanoseconds
    private static var lastValues: [String: String] = [:]

    static var enabled: Bool {
        enabledFlag
    }

    static var pickerPerfEnabled: Bool {
        pickerPerfEnabledFlag
    }

    static var terminalMouseEnabled: Bool {
        terminalMouseEnabledFlag
    }

    static func log(_ message: @autoclosure () -> String) {
        guard enabledFlag else { return }
        write(message())
    }

    static func logChanged(key: String, value: @autoclosure () -> String) {
        guard enabledFlag else { return }
        let next = value()
        var shouldWrite = false
        lock.lock()
        if lastValues[key] != next {
            lastValues[key] = next
            shouldWrite = true
        }
        lock.unlock()
        guard shouldWrite else { return }
        write("[\(key)] \(next)")
    }

    static func pickerPerfLog(_ message: @autoclosure () -> String) {
        guard pickerPerfEnabledFlag else { return }
        write("[pickerperf] \(message())")
    }

    static func terminalMouseLog(_ message: @autoclosure () -> String) {
        guard terminalMouseEnabledFlag else { return }
        write("[termmouse] \(message())")
    }

    private static func write(_ message: String) {
        let now = DispatchTime.now().uptimeNanoseconds
        let elapsedMs: UInt64
        if now >= startNanos {
            elapsedMs = (now - startNanos) / 1_000_000
        } else {
            elapsedMs = 0
        }
        let line = "[diagdbg +\(elapsedMs)ms] \(message)\n"
        guard let data = line.data(using: .utf8) else { return }
        FileHandle.standardError.write(data)
        if let diagnosticsFilePath {
            appendToDiagnosticsFile(path: diagnosticsFilePath, data: data)
        }
    }

    private static func appendToDiagnosticsFile(path: String, data: Data) {
        lock.lock()
        defer { lock.unlock() }
        if FileManager.default.fileExists(atPath: path) {
            if let handle = FileHandle(forWritingAtPath: path) {
                handle.seekToEndOfFile()
                handle.write(data)
                try? handle.close()
            }
            return
        }
        FileManager.default.createFile(atPath: path, contents: data)
    }
}
