import AppKit
import Foundation

@MainActor
final class EditorAgentSessionSupervisor {
    typealias EventHandler = @MainActor (_ event: String, _ payload: [String: Any]) -> Void

    private unowned let controller: EditorSurfaceController
    private let transport = EditorAgentRuntimeTransport()

    private var hasStarted = false
    private var sessionPathByAgentItemID: [UInt: String] = [:]
    private var subscribersByAgentItemID: [UInt: [UUID: EventHandler]] = [:]
    private var followEnabledByAgentItemID: [UInt: Bool] = [:]

    init(controller: EditorSurfaceController) {
        self.controller = controller
        transport.onEvent = { [weak self] sessionPath, event, payload in
            self?.handleEvent(sessionPath: sessionPath, event: event, payload: payload)
        }
        transport.onRequest = { [weak self] method, params in
            guard let self else { throw NSError(domain: "EditorAgentPanel", code: 1) }
            return try await self.handleRuntimeRequest(method: method, params: params)
        }
    }

    deinit {
        Task { @MainActor [transport] in
            transport.stop()
        }
    }

    var editorWorkingDirectory: String {
        if let root = controller.fileTree.root, !root.isEmpty {
            return root
        }
        if let absolutePath = controller.chrome.document.absolutePath, !absolutePath.isEmpty {
            return URL(fileURLWithPath: absolutePath).deletingLastPathComponent().path
        }
        return FileManager.default.currentDirectoryPath
    }

    func subscribe(agentItemID: UInt, handler: @escaping EventHandler) -> UUID {
        let token = UUID()
        var handlers = subscribersByAgentItemID[agentItemID] ?? [:]
        handlers[token] = handler
        subscribersByAgentItemID[agentItemID] = handlers
        return token
    }

    func unsubscribe(agentItemID: UInt, token: UUID) {
        guard var handlers = subscribersByAgentItemID[agentItemID] else { return }
        handlers.removeValue(forKey: token)
        if handlers.isEmpty {
            subscribersByAgentItemID.removeValue(forKey: agentItemID)
        } else {
            subscribersByAgentItemID[agentItemID] = handlers
        }
    }

    func setFollowEnabled(for agentItemID: UInt, enabled: Bool) {
        followEnabledByAgentItemID[agentItemID] = enabled
    }

    func ensureSessionSnapshot(for agentItemID: UInt) async throws -> [String: Any] {
        if let sessionPath = sessionPathByAgentItemID[agentItemID] {
            try startIfNeeded()
            return try await transport.request(method: "getSessionSnapshot", params: ["sessionPath": sessionPath])
        }

        try startIfNeeded()
        let snapshot = try await transport.request(method: "createSession", params: ["cwd": editorWorkingDirectory])
        sessionPathByAgentItemID[agentItemID] = try requireSessionPath(snapshot)
        return snapshot
    }

    func sendPrompt(for agentItemID: UInt, text: String) async throws -> [String: Any] {
        let sessionPath = try await ensureSessionPath(for: agentItemID)
        return try await transport.request(method: "prompt", params: [
            "sessionPath": sessionPath,
            "text": text,
        ])
    }

    func abort(for agentItemID: UInt) async throws {
        let sessionPath = try await ensureSessionPath(for: agentItemID)
        _ = try await transport.request(method: "abort", params: ["sessionPath": sessionPath])
    }

    func createNewSession(for agentItemID: UInt) async throws -> [String: Any] {
        let previousSessionPath = sessionPathByAgentItemID[agentItemID]
        try startIfNeeded()
        let snapshot = try await transport.request(method: "createSession", params: ["cwd": editorWorkingDirectory])
        let sessionPath = try requireSessionPath(snapshot)
        sessionPathByAgentItemID[agentItemID] = sessionPath
        await releaseSessionIfUnused(previousSessionPath, excluding: agentItemID)
        return snapshot
    }

    func openSession(for agentItemID: UInt, path: String) async throws -> [String: Any] {
        let previousSessionPath = sessionPathByAgentItemID[agentItemID]
        try startIfNeeded()
        let snapshot = try await transport.request(method: "openSession", params: ["path": path])
        let sessionPath = try requireSessionPath(snapshot)
        sessionPathByAgentItemID[agentItemID] = sessionPath
        if previousSessionPath != sessionPath {
            await releaseSessionIfUnused(previousSessionPath, excluding: agentItemID)
        }
        return snapshot
    }

    func listRecentSessions(cwd: String) async throws -> [String: Any] {
        try startIfNeeded()
        return try await transport.request(method: "listSessions", params: ["cwd": cwd])
    }

    func listModels(for agentItemID: UInt) async throws -> [String: Any] {
        let sessionPath = try await ensureSessionPath(for: agentItemID)
        return try await transport.request(method: "listModels", params: ["sessionPath": sessionPath])
    }

    func setModel(for agentItemID: UInt, provider: String, modelID: String) async throws -> [String: Any] {
        let sessionPath = try await ensureSessionPath(for: agentItemID)
        return try await transport.request(method: "setModel", params: [
            "sessionPath": sessionPath,
            "provider": provider,
            "modelId": modelID,
        ])
    }

    func compact(for agentItemID: UInt, customInstructions: String?) async throws -> [String: Any] {
        let sessionPath = try await ensureSessionPath(for: agentItemID)
        return try await transport.request(method: "compact", params: [
            "sessionPath": sessionPath,
            "customInstructions": customInstructions as Any,
        ])
    }

    func cycleThinkingLevel(for agentItemID: UInt) async throws -> [String: Any] {
        let sessionPath = try await ensureSessionPath(for: agentItemID)
        return try await transport.request(method: "cycleThinkingLevel", params: [
            "sessionPath": sessionPath,
        ])
    }

    func setSessionName(for agentItemID: UInt, name: String) async throws -> [String: Any] {
        let sessionPath = try await ensureSessionPath(for: agentItemID)
        return try await transport.request(method: "setSessionName", params: [
            "sessionPath": sessionPath,
            "name": name,
        ])
    }

    func listInlineRewriteModels(cwd: String) async throws -> [String: Any] {
        try startIfNeeded()
        return try await transport.request(method: "listInlineRewriteModels", params: [
            "cwd": cwd,
        ])
    }

    func inlineRewrite(
        cwd: String,
        filePath: String?,
        sourceLabel: String,
        lineStart: Int?,
        lineEnd: Int?,
        language: String?,
        selectionText: String,
        prompt: String,
        provider: String?,
        modelID: String?,
        modelSourceSessionPath: String?,
        thinkingLevel: String
    ) async throws -> [String: Any] {
        try startIfNeeded()
        return try await transport.request(method: "inlineRewrite", params: [
            "cwd": cwd,
            "filePath": filePath as Any,
            "sourceLabel": sourceLabel,
            "lineStart": lineStart as Any,
            "lineEnd": lineEnd as Any,
            "language": language as Any,
            "selectionText": selectionText,
            "prompt": prompt,
            "provider": provider as Any,
            "modelId": modelID as Any,
            "modelSourceSessionPath": modelSourceSessionPath as Any,
            "thinkingLevel": thinkingLevel,
        ])
    }

    func releaseAgentItem(_ agentItemID: UInt) {
        subscribersByAgentItemID.removeValue(forKey: agentItemID)
        let sessionPath = sessionPathByAgentItemID.removeValue(forKey: agentItemID)
        Task { @MainActor [weak self] in
            await self?.releaseSessionIfUnused(sessionPath, excluding: nil)
        }
    }

    private func startIfNeeded() throws {
        guard !hasStarted else { return }
        try transport.start(cwd: editorWorkingDirectory)
        hasStarted = true
    }

    private func ensureSessionPath(for agentItemID: UInt) async throws -> String {
        if let sessionPath = sessionPathByAgentItemID[agentItemID] {
            return sessionPath
        }
        let snapshot = try await ensureSessionSnapshot(for: agentItemID)
        return try requireSessionPath(snapshot)
    }

    private func requireSessionPath(_ payload: [String: Any]) throws -> String {
        let sessionPath = (payload["sessionPath"] as? String)
            ?? (payload["sessionFile"] as? String)
        guard let sessionPath, !sessionPath.isEmpty else {
            throw NSError(
                domain: "EditorAgentPanel",
                code: 29,
                userInfo: [NSLocalizedDescriptionKey: "Missing session path in agent response."]
            )
        }
        return sessionPath
    }

    private func releaseSessionIfUnused(_ sessionPath: String?, excluding excludedAgentItemID: UInt?) async {
        guard let sessionPath, hasStarted else { return }
        let isStillInUse = sessionPathByAgentItemID.contains { agentItemID, mappedSessionPath in
            guard mappedSessionPath == sessionPath else { return false }
            if let excludedAgentItemID {
                return agentItemID != excludedAgentItemID
            }
            return true
        }
        guard !isStillInUse else { return }
        _ = try? await transport.request(method: "closeSession", params: ["sessionPath": sessionPath])
    }

    private func handleEvent(sessionPath: String?, event: String, payload: [String: Any]) {
        let targetAgentItemIDs: [UInt]
        if let sessionPath, !sessionPath.isEmpty {
            targetAgentItemIDs = sessionPathByAgentItemID.compactMap { agentItemID, mappedSessionPath in
                mappedSessionPath == sessionPath ? agentItemID : nil
            }
        } else {
            targetAgentItemIDs = Array(subscribersByAgentItemID.keys)
        }

        for agentItemID in targetAgentItemIDs {
            let handlers = Array((subscribersByAgentItemID[agentItemID] ?? [:]).values)
            for handler in handlers {
                handler(event, payload)
            }
        }
    }

    private func handleRuntimeRequest(method: String, params: [String: Any]) async throws -> Any {
        switch method {
        case "editor.context":
            return [
                "cwd": editorWorkingDirectory,
                "activeFile": controller.chrome.document.absolutePath as Any,
                "selection": controller.primarySelectionText(),
            ]
        case "editor.readFile":
            return try readFile(params: params)
        case "editor.writeFile":
            return try writeFile(params: params)
        case "editor.writeFileAnimated":
            return try await writeFileAnimated(params: params)
        case "editor.previewEdit":
            return try previewEdit(params: params)
        case "editor.editFile":
            return try editFile(params: params)
        case "editor.editFileAnimated":
            return try await editFileAnimated(params: params)
        default:
            throw NSError(domain: "EditorAgentPanel", code: 404, userInfo: [NSLocalizedDescriptionKey: "Unknown editor bridge method: \(method)"])
        }
    }

    private func readFile(params: [String: Any]) throws -> [String: Any] {
        let absolutePath = try requirePath(params)
        if isCurrentDirtyDocument(at: absolutePath) {
            throw NSError(
                domain: "EditorAgentPanel",
                code: 2,
                userInfo: [NSLocalizedDescriptionKey: "Reading unsaved in-memory buffer content is not yet supported in this build. Save the file or read a file without unsaved changes."]
            )
        }
        let text = try String(contentsOfFile: absolutePath, encoding: .utf8)
        return [
            "text": text,
            "source": "disk",
        ]
    }

    private func writeFile(params: [String: Any]) throws -> [String: Any] {
        let absolutePath = try requirePath(params)
        let content = params["content"] as? String ?? ""
        if isCurrentDirtyDocument(at: absolutePath) {
            throw NSError(
                domain: "EditorAgentPanel",
                code: 3,
                userInfo: [NSLocalizedDescriptionKey: "Refusing to overwrite the active editor buffer while it has unsaved changes."]
            )
        }
        let previous = (try? String(contentsOfFile: absolutePath, encoding: .utf8)) ?? ""
        let parent = URL(fileURLWithPath: absolutePath).deletingLastPathComponent()
        try FileManager.default.createDirectory(at: parent, withIntermediateDirectories: true, attributes: nil)
        try content.write(toFile: absolutePath, atomically: true, encoding: .utf8)
        controller.refreshSnapshot()
        return [
            "bytes": Data(content.utf8).count,
            "diff": simpleDiffSummary(path: absolutePath, before: previous, after: content),
        ]
    }

    private func writeFileAnimated(params: [String: Any]) async throws -> [String: Any] {
        let absolutePath = try requirePath(params)
        let content = params["content"] as? String ?? ""
        if isCurrentDirtyDocument(at: absolutePath) {
            throw NSError(
                domain: "EditorAgentPanel",
                code: 3,
                userInfo: [NSLocalizedDescriptionKey: "Refusing to overwrite the active editor buffer while it has unsaved changes."]
            )
        }

        let previous = (try? String(contentsOfFile: absolutePath, encoding: .utf8)) ?? ""
        let shouldAnimate = isFollowEnabled(forSessionPath: params["sessionPath"] as? String)
        let firstChangedLine = firstChangedLineBetween(before: previous, after: content)

        if shouldAnimate {
            await controller.animateAgentFollowTextUpdate(
                path: absolutePath,
                originalText: previous,
                updatedText: content,
                lineStart: firstChangedLine,
                lineEnd: firstChangedLine
            )
        }

        do {
            let parent = URL(fileURLWithPath: absolutePath).deletingLastPathComponent()
            try FileManager.default.createDirectory(at: parent, withIntermediateDirectories: true, attributes: nil)
            try content.write(toFile: absolutePath, atomically: true, encoding: .utf8)
            controller.refreshSnapshot()
            return [
                "bytes": Data(content.utf8).count,
                "diff": simpleDiffSummary(path: absolutePath, before: previous, after: content),
            ]
        } catch {
            if shouldAnimate {
                _ = controller.setAgentFollowPreview(path: absolutePath, text: previous, lineStart: firstChangedLine, lineEnd: firstChangedLine)
            }
            throw error
        }
    }

    private func previewEdit(params: [String: Any]) throws -> [String: Any] {
        let absolutePath = try requirePath(params)
        if isCurrentDirtyDocument(at: absolutePath) {
            throw NSError(
                domain: "EditorAgentPanel",
                code: 4,
                userInfo: [NSLocalizedDescriptionKey: "Refusing to preview edits for the active editor buffer while it has unsaved changes."]
            )
        }
        guard let rawEdits = params["edits"] as? [[String: Any]], !rawEdits.isEmpty else {
            throw NSError(domain: "EditorAgentPanel", code: 5, userInfo: [NSLocalizedDescriptionKey: "No edits provided."])
        }
        let original = try String(contentsOfFile: absolutePath, encoding: .utf8)
        let replacements = try rawEdits.map(ExactTextReplacement.init)
        let result = try applyExactTextReplacements(replacements, to: original, path: absolutePath)
        return [
            "firstChangedLine": result.firstChangedLine as Any,
        ]
    }

    private func editFile(params: [String: Any]) throws -> [String: Any] {
        let absolutePath = try requirePath(params)
        if isCurrentDirtyDocument(at: absolutePath) {
            throw NSError(
                domain: "EditorAgentPanel",
                code: 4,
                userInfo: [NSLocalizedDescriptionKey: "Refusing to edit the active editor buffer while it has unsaved changes."]
            )
        }
        guard let rawEdits = params["edits"] as? [[String: Any]], !rawEdits.isEmpty else {
            throw NSError(domain: "EditorAgentPanel", code: 5, userInfo: [NSLocalizedDescriptionKey: "No edits provided."])
        }
        let original = try String(contentsOfFile: absolutePath, encoding: .utf8)
        let replacements = try rawEdits.map(ExactTextReplacement.init)
        let result = try applyExactTextReplacements(replacements, to: original, path: absolutePath)
        try result.updated.write(toFile: absolutePath, atomically: true, encoding: .utf8)
        controller.refreshSnapshot()
        return [
            "diff": simpleDiffSummary(path: absolutePath, before: original, after: result.updated),
            "firstChangedLine": result.firstChangedLine as Any,
        ]
    }

    private func editFileAnimated(params: [String: Any]) async throws -> [String: Any] {
        let absolutePath = try requirePath(params)
        if isCurrentDirtyDocument(at: absolutePath) {
            throw NSError(
                domain: "EditorAgentPanel",
                code: 4,
                userInfo: [NSLocalizedDescriptionKey: "Refusing to edit the active editor buffer while it has unsaved changes."]
            )
        }
        guard let rawEdits = params["edits"] as? [[String: Any]], !rawEdits.isEmpty else {
            throw NSError(domain: "EditorAgentPanel", code: 5, userInfo: [NSLocalizedDescriptionKey: "No edits provided."])
        }

        let original = try String(contentsOfFile: absolutePath, encoding: .utf8)
        let replacements = try rawEdits.map(ExactTextReplacement.init)
        let result = try applyExactTextReplacements(replacements, to: original, path: absolutePath)
        let shouldAnimate = isFollowEnabled(forSessionPath: params["sessionPath"] as? String)

        if shouldAnimate {
            await controller.animateAgentFollowTextUpdate(
                path: absolutePath,
                originalText: original,
                updatedText: result.updated,
                lineStart: result.firstChangedLine,
                lineEnd: result.firstChangedLine
            )
        }

        do {
            try result.updated.write(toFile: absolutePath, atomically: true, encoding: .utf8)
            controller.refreshSnapshot()
            return [
                "diff": simpleDiffSummary(path: absolutePath, before: original, after: result.updated),
                "firstChangedLine": result.firstChangedLine as Any,
            ]
        } catch {
            if shouldAnimate {
                _ = controller.setAgentFollowPreview(path: absolutePath, text: original, lineStart: result.firstChangedLine, lineEnd: result.firstChangedLine)
            }
            throw error
        }
    }

    private func requirePath(_ params: [String: Any]) throws -> String {
        guard let rawPath = params["path"] as? String, !rawPath.isEmpty else {
            throw NSError(domain: "EditorAgentPanel", code: 6, userInfo: [NSLocalizedDescriptionKey: "Missing path."])
        }
        return rawPath
    }

    private func isFollowEnabled(forSessionPath sessionPath: String?) -> Bool {
        guard let sessionPath, !sessionPath.isEmpty else { return false }
        return sessionPathByAgentItemID.contains { agentItemID, mappedSessionPath in
            mappedSessionPath == sessionPath && (followEnabledByAgentItemID[agentItemID] ?? false)
        }
    }

    private func firstChangedLineBetween(before: String, after: String) -> Int? {
        guard before != after else { return nil }
        let beforeScalars = Array(before.utf16)
        let afterScalars = Array(after.utf16)
        let limit = min(beforeScalars.count, afterScalars.count)
        var offset = 0
        while offset < limit, beforeScalars[offset] == afterScalars[offset] {
            offset += 1
        }
        let prefix = before.utf16.prefix(offset)
        return prefix.reduce(into: 1) { count, scalar in
            if scalar == 10 {
                count += 1
            }
        }
    }

    private func isCurrentDirtyDocument(at path: String) -> Bool {
        guard let currentPath = controller.chrome.document.absolutePath else { return false }
        return currentPath == path && controller.chrome.document.isModified
    }

    private func simpleDiffSummary(path: String, before: String, after: String) -> String {
        let beforeLines = before.components(separatedBy: .newlines)
        let afterLines = after.components(separatedBy: .newlines)
        let changedCount = zipLongest(beforeLines, afterLines).reduce(into: 0) { partialResult, pair in
            if pair.0 != pair.1 {
                partialResult += 1
            }
        }
        let displayPath = URL(fileURLWithPath: path).lastPathComponent
        return "--- \(displayPath)\n+++ \(displayPath)\nChanged lines: \(changedCount)"
    }

    private func zipLongest(_ lhs: [String], _ rhs: [String]) -> [(String, String)] {
        let count = max(lhs.count, rhs.count)
        return (0..<count).map { index in
            let left = index < lhs.count ? lhs[index] : ""
            let right = index < rhs.count ? rhs[index] : ""
            return (left, right)
        }
    }
}
