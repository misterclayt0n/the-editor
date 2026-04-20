import AppKit
import Combine
import Foundation
import SwiftUI

struct EditorAgentCommand: Identifiable, Equatable {
    let name: String
    let description: String
    let source: String

    var id: String { "\(source):\(name)" }

    var isBuiltin: Bool {
        source == "builtin"
    }
}

struct EditorAgentRecentSession: Identifiable, Equatable {
    let id: String
    let path: String
    let name: String?
    let firstMessage: String
    let modified: String

    var title: String {
        let trimmedName = name?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        if !trimmedName.isEmpty { return trimmedName }
        let trimmedMessage = firstMessage.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmedMessage.isEmpty { return trimmedMessage }
        return URL(fileURLWithPath: path).lastPathComponent
    }
}

struct EditorAgentModel: Identifiable, Equatable {
    let provider: String
    let id: String
    let name: String
    let reference: String
    let isCurrent: Bool

    var displayName: String {
        id
    }
}

struct EditorAgentFooterInfo: Equatable {
    let cwd: String
    let gitBranch: String?
    let sessionName: String?
    let totalInput: Int
    let totalOutput: Int
    let totalCacheRead: Int
    let totalCacheWrite: Int
    let totalCost: Double
    let usingSubscription: Bool
    let contextTokens: Int?
    let contextPercent: Double?
    let contextWindow: Int?
    let autoCompactEnabled: Bool
    let modelProvider: String?
    let modelID: String?
    let modelName: String?
    let modelSupportsReasoning: Bool
    let thinkingLevel: String
    let availableProviderCount: Int
}

struct EditorAgentFollowEvent: Equatable {
    enum Kind: String, Equatable {
        case read
        case edit
        case write
    }

    enum Phase: String, Equatable {
        case before
        case after
    }

    let kind: Kind
    let phase: Phase
    let path: String
    let lineStart: Int?
    let lineEnd: Int?
    let summary: String?

    var displayText: String {
        let fileName = URL(fileURLWithPath: path).lastPathComponent
        if let lineStart, let lineEnd, lineEnd > lineStart {
            return "\(fileName):\(lineStart)-\(lineEnd)"
        }
        if let lineStart {
            return "\(fileName):\(lineStart)"
        }
        return fileName
    }
}

struct EditorAgentCompactionStatus: Equatable {
    enum Reason: String, Equatable {
        case manual
        case threshold
        case overflow

        init(rawValue: String) {
            switch rawValue {
            case "threshold":
                self = .threshold
            case "overflow":
                self = .overflow
            default:
                self = .manual
            }
        }
    }

    let reason: Reason

    var title: String {
        switch reason {
        case .manual:
            return "Compacting context…"
        case .threshold:
            return "Auto-compacting…"
        case .overflow:
            return "Context overflow detected, auto-compacting…"
        }
    }

    var placeholder: String {
        switch reason {
        case .manual:
            return "pi is compacting context…"
        case .threshold, .overflow:
            return "pi is auto-compacting…"
        }
    }

    var isAutomatic: Bool {
        reason != .manual
    }
}

struct EditorAgentTranscriptItem: Identifiable, Equatable, Hashable {
    enum Kind: String, Hashable {
        case user
        case assistant
        case thinking
        case note
        case tool
    }

    enum NoteStyle: String, Hashable {
        case plain
        case compactionSummary
        case branchSummary
    }

    let id: String
    var correlationID: String?
    var kind: Kind
    var title: String?
    var text: String
    var isStreaming: Bool
    var status: String?
    var contextSummary: String?
    var toolInputJSON: String?
    var renderedMarkdown: EditorRenderedMarkdown?
    var noteStyle: NoteStyle = .plain
    var tokensBefore: Int?
    var revision: Int = 0
}

@MainActor
final class EditorAgentPanelStore: ObservableObject {
    @Published private(set) var items: [EditorAgentTranscriptItem] = [] {
        didSet {
            if oldValue != items {
                transcriptRevision &+= 1
            }
        }
    }
    @Published private(set) var transcriptRevision: Int = 0
    @Published private(set) var commands: [EditorAgentCommand] = []
    @Published private(set) var recentSessions: [EditorAgentRecentSession] = []
    @Published private(set) var models: [EditorAgentModel] = []
    @Published private(set) var footerInfo: EditorAgentFooterInfo?
    @Published private(set) var isRuntimeReady = false
    @Published private(set) var isRunning = false
    @Published private(set) var compactionStatus: EditorAgentCompactionStatus?
    @Published private(set) var isFollowingAgent = false
    @Published private(set) var followStatusText: String?
    @Published private(set) var sessionTitle = "Agent"
    @Published private(set) var sessionSubtitle = "pi"
    @Published private(set) var contextUsageText: String?
    @Published var errorMessage: String?

    private unowned let controller: EditorSurfaceController
    private let agentItemID: UInt
    private unowned let supervisor: EditorAgentSessionSupervisor
    private var subscriptionToken: UUID?
    private var hasStarted = false
    private var markdownBackfillTask: Task<Void, Never>?
    private var markdownRenderGeneration: UInt64 = 0
    private var pendingTranscriptFlushTask: Task<Void, Never>?
    private var recentSessionsTask: Task<Void, Never>?
    private var hasLoadedRecentSessions = false
    private var followPaneID: UInt?
    private var pendingAssistantDeltas: [String: String] = [:]
    private var pendingThinkingDeltas: [String: String] = [:]
    private var pendingToolDeltas: [String: [String]] = [:]
    private var pendingToolInputJSONByID: [String: String] = [:]

    private let immediateAssistantMarkdownRenderCount = 8
    private let deferredAssistantMarkdownBatchSize = 2
    private let transcriptFlushInterval: Duration = .milliseconds(80)

    init(controller: EditorSurfaceController, agentItemID: UInt, supervisor: EditorAgentSessionSupervisor) {
        self.controller = controller
        self.agentItemID = agentItemID
        self.supervisor = supervisor
        subscriptionToken = supervisor.subscribe(agentItemID: agentItemID) { [weak self] event, payload in
            self?.handleEvent(event: event, payload: payload)
        }
    }

    deinit {
        markdownBackfillTask?.cancel()
        pendingTranscriptFlushTask?.cancel()
        recentSessionsTask?.cancel()
        Task { @MainActor [supervisor, agentItemID, controller] in
            supervisor.setFollowEnabled(for: agentItemID, enabled: false)
            controller.setAgentControlledPane(nil)
        }
        if let subscriptionToken {
            Task { @MainActor [supervisor, agentItemID] in
                supervisor.unsubscribe(agentItemID: agentItemID, token: subscriptionToken)
            }
        }
    }

    func startIfNeeded() {
        guard !hasStarted else { return }
        hasStarted = true
        Task {
            do {
                let response = try await supervisor.ensureSessionSnapshot(for: agentItemID)
                applySessionSnapshot(response)
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    func activateAgentSurfaceIfNeeded() {
        controller.activateAgentOpenItemIfNeeded(agentItemID: agentItemID)
    }

    func setAgentFollowEnabled(_ enabled: Bool) {
        guard isFollowingAgent != enabled else { return }
        isFollowingAgent = enabled
        supervisor.setFollowEnabled(for: agentItemID, enabled: enabled)
        if !enabled {
            followStatusText = nil
            followPaneID = nil
            controller.setAgentControlledPane(nil)
        }
    }

    func toggleAgentFollow() {
        setAgentFollowEnabled(!isFollowingAgent)
    }

    func sendPrompt(_ rawText: String) {
        let text = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        errorMessage = nil
        Task {
            do {
                _ = try await supervisor.sendPrompt(for: agentItemID, text: text)
                invalidateRecentSessions()
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    func abort() {
        Task {
            do {
                try await supervisor.abort(for: agentItemID)
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    func newSession() {
        Task {
            do {
                let response = try await supervisor.createNewSession(for: agentItemID)
                applySessionSnapshot(response)
                invalidateRecentSessions()
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    func refreshRecentSessions(force: Bool = false) {
        if recentSessionsTask != nil {
            return
        }
        if !force, hasLoadedRecentSessions {
            return
        }

        recentSessionsTask = Task { [weak self] in
            guard let self else { return }
            defer { self.recentSessionsTask = nil }
            do {
                let response = try await supervisor.listRecentSessions(cwd: editorWorkingDirectory)
                guard !Task.isCancelled else { return }
                recentSessions = parseRecentSessions(response)
                hasLoadedRecentSessions = true
            } catch {
                guard !Task.isCancelled else { return }
                errorMessage = error.localizedDescription
            }
        }
    }

    func resumeSession(path: String) {
        Task {
            do {
                let response = try await supervisor.openSession(for: agentItemID, path: path)
                applySessionSnapshot(response)
                invalidateRecentSessions()
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    func invalidateRecentSessions() {
        hasLoadedRecentSessions = false
    }

    func refreshModels() {
        Task {
            do {
                let response = try await supervisor.listModels(for: agentItemID)
                models = parseModels(response["result"])
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    func setModel(provider: String, modelID: String) {
        Task {
            do {
                let response = try await supervisor.setModel(for: agentItemID, provider: provider, modelID: modelID)
                applySessionStatus(response)
                if let responseModels = response["models"] {
                    models = parseModels(responseModels)
                }
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    func compact(customInstructions: String?) {
        Task {
            do {
                let response = try await supervisor.compact(for: agentItemID, customInstructions: customInstructions)
                applySessionSnapshot(response)
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    func cycleThinkingLevel() {
        Task {
            do {
                let response = try await supervisor.cycleThinkingLevel(for: agentItemID)
                applySessionStatus(response)
                if let responseModels = response["models"] {
                    models = parseModels(responseModels)
                }
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    func setSessionName(_ name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            errorMessage = "Session name cannot be empty."
            return
        }
        Task {
            do {
                let response = try await supervisor.setSessionName(for: agentItemID, name: trimmed)
                applySessionStatus(response)
                invalidateRecentSessions()
                refreshRecentSessions(force: true)
            } catch {
                errorMessage = error.localizedDescription
            }
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

    private func handleEvent(event: String, payload: [String: Any]) {
        agentDebugLog("event=\(event) payloadKeys=\(payload.keys.sorted()) items=\(items.count) running=\(isRunning) pendingAssistant=\(pendingAssistantDeltas.count) pendingTool=\(pendingToolDeltas.count)")
        switch event {
        case "user_message":
            if let text = payload["text"] as? String {
                items.append(
                    EditorAgentTranscriptItem(
                        id: UUID().uuidString,
                        correlationID: payload["id"] as? String,
                        kind: .user,
                        title: nil,
                        text: text,
                        isStreaming: false,
                        status: nil,
                        contextSummary: payload["context"] as? String,
                        toolInputJSON: nil,
                        renderedMarkdown: nil
                    )
                )
                isRunning = true
            }
        case "assistant_delta":
            let id = payload["id"] as? String ?? UUID().uuidString
            let delta = payload["delta"] as? String ?? ""
            guard !delta.isEmpty else { return }
            pendingAssistantDeltas[id, default: ""] += delta
            isRunning = true
            schedulePendingTranscriptFlush()
        case "assistant_completed":
            let id = payload["id"] as? String ?? UUID().uuidString
            let text = payload["text"] as? String ?? ""
            completeAssistantMessage(
                id: id,
                finalText: text,
                stopReason: payload["stopReason"] as? String,
                errorMessage: payload["errorMessage"] as? String
            )
        case "note_message":
            if let text = payload["text"] as? String {
                let noteStyle = EditorAgentTranscriptItem.NoteStyle(rawValue: payload["noteStyle"] as? String ?? "") ?? .plain
                let renderedMarkdown = renderedMarkdownForNote(text: text, noteStyle: noteStyle)
                items.append(
                    EditorAgentTranscriptItem(
                        id: UUID().uuidString,
                        correlationID: payload["id"] as? String,
                        kind: .note,
                        title: nil,
                        text: text,
                        isStreaming: false,
                        status: nil,
                        contextSummary: nil,
                        toolInputJSON: nil,
                        renderedMarkdown: renderedMarkdown,
                        noteStyle: noteStyle,
                        tokensBefore: payload["tokensBefore"] as? Int
                    )
                )
            }
        case "compaction_start":
            let reason = EditorAgentCompactionStatus.Reason(rawValue: payload["reason"] as? String ?? "manual")
            compactionStatus = EditorAgentCompactionStatus(reason: reason)
            errorMessage = nil
        case "compaction_end":
            let reason = EditorAgentCompactionStatus.Reason(rawValue: payload["reason"] as? String ?? compactionStatus?.reason.rawValue ?? "manual")
            let wasAutomatic = reason != .manual
            compactionStatus = nil
            if let snapshot = payload["snapshot"] as? [String: Any] {
                applySessionSnapshot(snapshot, preserveRunningState: (payload["willRetry"] as? Bool) == true)
            }
            if (payload["aborted"] as? Bool) == true {
                if wasAutomatic {
                    errorMessage = nil
                } else {
                    errorMessage = "Compaction cancelled"
                }
            } else if let errorText = (payload["errorMessage"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines),
                      !errorText.isEmpty {
                errorMessage = errorText
            }
        case "thinking_started":
            let id = payload["id"] as? String ?? UUID().uuidString
            if let index = items.lastIndex(where: { $0.correlationID == id && $0.kind == .thinking }) {
                items[index].isStreaming = true
                touchRevision(&items[index])
            } else {
                items.append(
                    EditorAgentTranscriptItem(
                        id: UUID().uuidString,
                        correlationID: id,
                        kind: .thinking,
                        title: nil,
                        text: "",
                        isStreaming: true,
                        status: nil,
                        contextSummary: nil,
                        toolInputJSON: nil,
                        renderedMarkdown: nil
                    )
                )
            }
            isRunning = true
        case "thinking_delta":
            let id = payload["id"] as? String ?? UUID().uuidString
            let delta = payload["delta"] as? String ?? ""
            guard !delta.isEmpty else { return }
            pendingThinkingDeltas[id, default: ""] += delta
            isRunning = true
            schedulePendingTranscriptFlush()
        case "thinking_completed":
            let id = payload["id"] as? String ?? UUID().uuidString
            let text = payload["text"] as? String ?? ""
            completeThinkingMessage(id: id, finalText: text)
        case "tool_started":
            let correlationID = payload["id"] as? String
            if let correlationID,
               let index = items.lastIndex(where: { $0.correlationID == correlationID && $0.kind == .tool && ($0.isStreaming || $0.status == "running") }) {
                items[index].title = payload["summary"] as? String ?? (payload["toolName"] as? String) ?? items[index].title
                items[index].contextSummary = payload["toolName"] as? String ?? items[index].contextSummary
                if let inputJSON = payload["inputJSON"] as? String, !inputJSON.isEmpty {
                    items[index].toolInputJSON = inputJSON
                }
                items[index].isStreaming = true
                items[index].status = "running"
                touchRevision(&items[index])
            } else {
                items.append(
                    EditorAgentTranscriptItem(
                        id: UUID().uuidString,
                        correlationID: correlationID,
                        kind: .tool,
                        title: payload["summary"] as? String ?? (payload["toolName"] as? String) ?? "Tool",
                        text: "",
                        isStreaming: true,
                        status: "running",
                        contextSummary: payload["toolName"] as? String,
                        toolInputJSON: payload["inputJSON"] as? String,
                        renderedMarkdown: nil
                    )
                )
            }
            isRunning = true
        case "tool_updated":
            let id = payload["id"] as? String ?? ""
            let delta = payload["text"] as? String ?? ""
            guard !id.isEmpty, !delta.isEmpty else { return }
            pendingToolDeltas[id, default: []].append(delta)
            if let inputJSON = payload["inputJSON"] as? String, !inputJSON.isEmpty {
                pendingToolInputJSONByID[id] = inputJSON
            }
            isRunning = true
            schedulePendingTranscriptFlush()
        case "tool_completed":
            let id = payload["id"] as? String ?? ""
            guard !id.isEmpty else { return }
            if let inputJSON = payload["inputJSON"] as? String, !inputJSON.isEmpty {
                pendingToolInputJSONByID[id] = inputJSON
            }
            completeToolMessage(
                id: id,
                finalText: payload["text"] as? String,
                isError: (payload["isError"] as? Bool) == true
            )
        case "agent_follow":
            guard let followEvent = parseFollowEvent(payload) else { return }
            followStatusText = followEvent.displayText
            guard isFollowingAgent else { return }
            followPaneID = controller.revealAgentFollowLocation(
                path: followEvent.path,
                lineStart: followEvent.lineStart,
                lineEnd: followEvent.lineEnd,
                agentItemID: agentItemID,
                preferredPaneID: followPaneID
            )
            controller.setAgentControlledPane(followPaneID)
        case "session_status":
            applySessionStatus(payload)
            finalizeStreamingTranscriptItems()
            isRunning = false
        case "runtime_error":
            errorMessage = payload["message"] as? String ?? "Agent runtime error"
            finalizeStreamingTranscriptItems(markToolsFailed: true)
            isRunning = false
        default:
            break
        }
    }

    private func schedulePendingTranscriptFlush() {
        guard pendingTranscriptFlushTask == nil else {
            agentDebugLog("flush.schedule skipped existingTask pendingAssistant=\(pendingAssistantDeltas.count) pendingThinking=\(pendingThinkingDeltas.count) pendingTool=\(pendingToolDeltas.count)")
            return
        }
        agentDebugLog("flush.schedule pendingAssistant=\(pendingAssistantDeltas.count) pendingThinking=\(pendingThinkingDeltas.count) pendingTool=\(pendingToolDeltas.count)")
        pendingTranscriptFlushTask = Task { @MainActor [weak self] in
            guard let self else { return }
            try? await Task.sleep(for: self.transcriptFlushInterval)
            guard !Task.isCancelled else { return }
            self.pendingTranscriptFlushTask = nil
            self.flushPendingTranscriptUpdates()
        }
    }

    private func flushPendingTranscriptUpdates() {
        pendingTranscriptFlushTask?.cancel()
        pendingTranscriptFlushTask = nil
        guard !pendingAssistantDeltas.isEmpty || !pendingThinkingDeltas.isEmpty || !pendingToolDeltas.isEmpty else {
            agentDebugLog("flush.noop items=\(items.count)")
            return
        }

        let assistantSummary = pendingAssistantDeltas.mapValues { $0.count }
        let thinkingSummary = pendingThinkingDeltas.mapValues { $0.count }
        let toolSummary = pendingToolDeltas.mapValues { $0.count }
        agentDebugLog("flush.begin items=\(items.count) pendingAssistant=\(assistantSummary) pendingThinking=\(thinkingSummary) pendingTool=\(toolSummary)")
        var nextItems = items
        for (id, delta) in pendingAssistantDeltas where !delta.isEmpty {
            applyAssistantDelta(id: id, delta: delta, to: &nextItems)
        }
        for (id, delta) in pendingThinkingDeltas where !delta.isEmpty {
            applyThinkingDelta(id: id, delta: delta, to: &nextItems)
        }
        for (id, deltas) in pendingToolDeltas where !deltas.isEmpty {
            applyToolDeltas(id: id, deltas: deltas, to: &nextItems)
        }

        pendingAssistantDeltas.removeAll()
        pendingThinkingDeltas.removeAll()
        pendingToolDeltas.removeAll()

        if nextItems != items {
            agentDebugLog("flush.commit oldItems=\(items.count) newItems=\(nextItems.count)")
            items = nextItems
        } else {
            agentDebugLog("flush.nochange items=\(items.count)")
        }
    }

    private func completeAssistantMessage(id: String, finalText: String, stopReason: String?, errorMessage: String?) {
        agentDebugLog("assistant.complete.begin id=\(id) finalChars=\(finalText.count) stopReason=\(stopReason ?? "nil") items=\(items.count) pendingDeltaChars=\(pendingAssistantDeltas[id]?.count ?? 0)")
        if finalText.isEmpty,
           let pendingDelta = pendingAssistantDeltas.removeValue(forKey: id),
           !pendingDelta.isEmpty {
            var nextItems = items
            applyAssistantDelta(id: id, delta: pendingDelta, to: &nextItems)
            if nextItems != items {
                items = nextItems
            }
        } else {
            pendingAssistantDeltas.removeValue(forKey: id)
        }

        var nextItems = items
        if let index = nextItems.lastIndex(where: { $0.correlationID == id && $0.kind == .assistant }) {
            let resolvedText = finalText.isEmpty ? nextItems[index].text : finalText
            if resolvedText.isEmpty {
                nextItems.remove(at: index)
            } else {
                nextItems[index].text = resolvedText
                nextItems[index].isStreaming = false
                nextItems[index].renderedMarkdown = controller.renderMarkdown(resolvedText)
                touchRevision(&nextItems[index])
            }
        } else if !finalText.isEmpty {
            nextItems.append(
                EditorAgentTranscriptItem(
                    id: UUID().uuidString,
                    correlationID: id,
                    kind: .assistant,
                    title: nil,
                    text: finalText,
                    isStreaming: false,
                    status: nil,
                    contextSummary: nil,
                    toolInputJSON: nil,
                    renderedMarkdown: controller.renderMarkdown(finalText)
                )
            )
        }

        if let terminalText = assistantTerminalMessage(stopReason: stopReason, errorMessage: errorMessage) {
            upsertAssistantTerminalNote(assistantID: id, text: terminalText, into: &nextItems)
        }

        if nextItems != items {
            if let index = nextItems.lastIndex(where: { $0.correlationID == id && $0.kind == .assistant }) {
                let item = nextItems[index]
                agentDebugLog("assistant.complete.commit id=\(id) chars=\(item.text.count) revision=\(item.revision) renderedBlocks=\(item.renderedMarkdown?.blocks.count ?? 0)")
            } else {
                agentDebugLog("assistant.complete.commit id=\(id) noAssistantRow stopReason=\(stopReason ?? "nil")")
            }
            items = nextItems
        } else {
            agentDebugLog("assistant.complete.nochange id=\(id)")
        }
    }

    private func completeToolMessage(id: String, finalText: String?, isError: Bool) {
        agentDebugLog("tool.complete.begin id=\(id) finalChars=\(finalText?.count ?? 0) isError=\(isError) pendingDeltaCount=\(pendingToolDeltas[id]?.count ?? 0)")
        if finalText?.isEmpty != false,
           let pendingDeltas = pendingToolDeltas.removeValue(forKey: id),
           !pendingDeltas.isEmpty {
            var nextItems = items
            applyToolDeltas(id: id, deltas: pendingDeltas, to: &nextItems)
            if nextItems != items {
                items = nextItems
            }
        } else {
            pendingToolDeltas.removeValue(forKey: id)
        }

        guard let index = items.lastIndex(where: { $0.correlationID == id && $0.kind == .tool }) else { return }
        items[index].status = isError ? "failed" : "done"
        items[index].isStreaming = false
        if let finalText, !finalText.isEmpty {
            items[index].text = finalText
        }
        if let inputJSON = items[index].toolInputJSON ?? pendingToolInputJSONByID[id] {
            items[index].toolInputJSON = inputJSON
        }
        pendingToolInputJSONByID.removeValue(forKey: id)
        touchRevision(&items[index])
    }

    private func assistantTerminalMessage(stopReason: String?, errorMessage: String?) -> String? {
        let normalizedReason = stopReason?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let normalizedError = errorMessage?.trimmingCharacters(in: .whitespacesAndNewlines)
        switch normalizedReason {
        case "aborted":
            return normalizedError?.nilIfEmpty ?? "Operation aborted"
        case "error":
            return normalizedError?.nilIfEmpty ?? "Error"
        default:
            return nil
        }
    }

    private func upsertAssistantTerminalNote(assistantID: String, text: String, into items: inout [EditorAgentTranscriptItem]) {
        let noteCorrelationID = "\(assistantID)-terminal"
        if let index = items.lastIndex(where: { $0.correlationID == noteCorrelationID && $0.kind == .note }) {
            items[index].text = text
            items[index].isStreaming = false
            touchRevision(&items[index])
            return
        }
        items.append(
            EditorAgentTranscriptItem(
                id: UUID().uuidString,
                correlationID: noteCorrelationID,
                kind: .note,
                title: nil,
                text: text,
                isStreaming: false,
                status: nil,
                contextSummary: nil,
                toolInputJSON: nil,
                renderedMarkdown: nil
            )
        )
    }

    private func applyAssistantDelta(id: String, delta: String, to items: inout [EditorAgentTranscriptItem]) {
        agentDebugLog("assistant.delta id=\(id) deltaChars=\(delta.count) existing=\(items.lastIndex(where: { $0.correlationID == id && $0.kind == .assistant }) != nil)")
        if let index = items.lastIndex(where: { $0.correlationID == id && $0.kind == .assistant }) {
            items[index].text += delta
            items[index].isStreaming = true
            items[index].renderedMarkdown = nil
            touchRevision(&items[index])
        } else {
            items.append(
                EditorAgentTranscriptItem(
                    id: UUID().uuidString,
                    correlationID: id,
                    kind: .assistant,
                    title: nil,
                    text: delta,
                    isStreaming: true,
                    status: nil,
                    contextSummary: nil,
                    toolInputJSON: nil,
                    renderedMarkdown: nil
                )
            )
        }
    }

    private func completeThinkingMessage(id: String, finalText: String) {
        agentDebugLog("thinking.complete.begin id=\(id) finalChars=\(finalText.count) items=\(items.count) pendingDeltaChars=\(pendingThinkingDeltas[id]?.count ?? 0)")
        if finalText.isEmpty,
           let pendingDelta = pendingThinkingDeltas.removeValue(forKey: id),
           !pendingDelta.isEmpty {
            var nextItems = items
            applyThinkingDelta(id: id, delta: pendingDelta, to: &nextItems)
            if nextItems != items {
                items = nextItems
            }
        } else {
            pendingThinkingDeltas.removeValue(forKey: id)
        }

        var nextItems = items
        if let index = nextItems.lastIndex(where: { $0.correlationID == id && $0.kind == .thinking }) {
            let resolvedText = finalText.isEmpty ? nextItems[index].text : finalText
            nextItems[index].text = resolvedText
            nextItems[index].isStreaming = false
            touchRevision(&nextItems[index])
        } else if !finalText.isEmpty {
            nextItems.append(
                EditorAgentTranscriptItem(
                    id: UUID().uuidString,
                    correlationID: id,
                    kind: .thinking,
                    title: nil,
                    text: finalText,
                    isStreaming: false,
                    status: nil,
                    contextSummary: nil,
                    toolInputJSON: nil,
                    renderedMarkdown: nil
                )
            )
        }

        if nextItems != items {
            items = nextItems
        }
    }

    private func applyThinkingDelta(id: String, delta: String, to items: inout [EditorAgentTranscriptItem]) {
        agentDebugLog("thinking.delta id=\(id) deltaChars=\(delta.count) existing=\(items.lastIndex(where: { $0.correlationID == id && $0.kind == .thinking }) != nil)")
        if let index = items.lastIndex(where: { $0.correlationID == id && $0.kind == .thinking }) {
            items[index].text += delta
            items[index].isStreaming = true
            touchRevision(&items[index])
        } else {
            items.append(
                EditorAgentTranscriptItem(
                    id: UUID().uuidString,
                    correlationID: id,
                    kind: .thinking,
                    title: nil,
                    text: delta,
                    isStreaming: true,
                    status: nil,
                    contextSummary: nil,
                    toolInputJSON: nil,
                    renderedMarkdown: nil
                )
            )
        }
    }

    private func applyToolDeltas(id: String, deltas: [String], to items: inout [EditorAgentTranscriptItem]) {
        agentDebugLog("tool.delta id=\(id) chunks=\(deltas.count) totalChars=\(deltas.reduce(0) { $0 + $1.count })")
        guard let index = items.lastIndex(where: { $0.correlationID == id && $0.kind == .tool }) else { return }
        let combinedDelta = deltas.joined(separator: "\n")
        if items[index].text.isEmpty {
            items[index].text = combinedDelta
        } else {
            items[index].text += "\n" + combinedDelta
        }
        touchRevision(&items[index])
    }

    private func touchRevision(_ item: inout EditorAgentTranscriptItem) {
        item.revision &+= 1
    }

    private func resetPendingTranscriptBuffers() {
        pendingTranscriptFlushTask?.cancel()
        pendingTranscriptFlushTask = nil
        pendingAssistantDeltas.removeAll()
        pendingThinkingDeltas.removeAll()
        pendingToolDeltas.removeAll()
        pendingToolInputJSONByID.removeAll()
    }

    private func applySessionSnapshot(_ payload: [String: Any], preserveRunningState: Bool = false) {
        markdownBackfillTask?.cancel()
        resetPendingTranscriptBuffers()
        markdownRenderGeneration &+= 1
        let renderGeneration = markdownRenderGeneration

        let totalStart = CFAbsoluteTimeGetCurrent()
        let parsedItems = measureAgentPerf("sessionSnapshot.parseHistory") {
            parseHistory(payload["history"])
        }
        items = parsedItems
        let assistantCount = parsedItems.lazy.filter { $0.kind == .assistant }.count
        let totalChars = parsedItems.reduce(into: 0) { $0 += $1.text.count }
        measureAgentPerf("sessionSnapshot.renderMarkdownForRecentAssistantItems") {
            renderMarkdownForRecentAssistantItems(limit: immediateAssistantMarkdownRenderCount)
        }
        scheduleDeferredMarkdownRendering(generation: renderGeneration)
        commands = measureAgentPerf("sessionSnapshot.parseCommands") {
            parseCommands(payload["commands"])
        }
        models = measureAgentPerf("sessionSnapshot.parseModels") {
            parseModels(payload["models"])
        }
        footerInfo = parseFooterInfo(payload["footer"])
        sessionTitle = (payload["sessionName"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty ?? "Agent"
        if let model = (payload["model"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines), !model.isEmpty {
            sessionSubtitle = model
        } else {
            sessionSubtitle = "pi"
        }
        if let usage = (payload["footer"] as? [String: Any]).flatMap(formatFooterContextUsage)
            ?? ((payload["contextUsage"] as? [String: Any]).flatMap(formatContextUsage)) {
            contextUsageText = usage
        } else {
            contextUsageText = nil
        }
        isRuntimeReady = true
        compactionStatus = nil
        if !preserveRunningState {
            isRunning = false
        }
        errorMessage = nil
        let totalMs = (CFAbsoluteTimeGetCurrent() - totalStart) * 1000
        agentPerfLog(
            "sessionSnapshot.done items=\(parsedItems.count) assistants=\(assistantCount) commands=\(commands.count) models=\(models.count) chars=\(totalChars) totalMs=\(String(format: "%.2f", totalMs))"
        )
    }

    private func applySessionStatus(_ payload: [String: Any]) {
        footerInfo = parseFooterInfo(payload["footer"])
        sessionTitle = footerInfo?.sessionName?.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty ?? "Agent"
        if let model = (payload["model"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines), !model.isEmpty {
            sessionSubtitle = model
        } else {
            sessionSubtitle = "pi"
        }
        if let usage = (payload["footer"] as? [String: Any]).flatMap(formatFooterContextUsage)
            ?? ((payload["contextUsage"] as? [String: Any]).flatMap(formatContextUsage)) {
            contextUsageText = usage
        }
    }

    private func parseFollowEvent(_ payload: [String: Any]) -> EditorAgentFollowEvent? {
        guard let path = (payload["path"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines), !path.isEmpty,
              let kind = EditorAgentFollowEvent.Kind(rawValue: payload["kind"] as? String ?? ""),
              let phase = EditorAgentFollowEvent.Phase(rawValue: payload["phase"] as? String ?? "")
        else {
            return nil
        }

        return EditorAgentFollowEvent(
            kind: kind,
            phase: phase,
            path: path,
            lineStart: followEventInt(payload["lineStart"]),
            lineEnd: followEventInt(payload["lineEnd"]),
            summary: payload["summary"] as? String
        )
    }

    private func followEventInt(_ value: Any?) -> Int? {
        switch value {
        case let int as Int:
            return int
        case let number as NSNumber:
            return number.intValue
        case let string as String:
            return Int(string)
        default:
            return nil
        }
    }

    private func finalizeStreamingTranscriptItems(markToolsFailed: Bool = false) {
        agentDebugLog("finalize.begin markToolsFailed=\(markToolsFailed) items=\(items.count)")
        flushPendingTranscriptUpdates()

        var nextItems = items
        var hasChanges = false
        for index in nextItems.indices where nextItems[index].isStreaming || nextItems[index].status == "running" {
            if nextItems[index].isStreaming {
                nextItems[index].isStreaming = false
                hasChanges = true
            }
            if nextItems[index].kind == .tool, nextItems[index].status == "running" {
                nextItems[index].status = markToolsFailed ? "failed" : "done"
                hasChanges = true
            }
            if hasChanges {
                touchRevision(&nextItems[index])
            }
        }

        pendingAssistantDeltas.removeAll()
        pendingThinkingDeltas.removeAll()
        pendingToolDeltas.removeAll()
        pendingToolInputJSONByID.removeAll()

        if hasChanges, nextItems != items {
            let assistantStreamingCount = nextItems.filter { $0.kind == .assistant && $0.isStreaming }.count
            let runningToolCount = nextItems.filter { $0.kind == .tool && $0.status == "running" }.count
            agentDebugLog("finalize.commit items=\(nextItems.count) assistantStreaming=\(assistantStreamingCount) runningTools=\(runningToolCount)")
            items = nextItems
        } else {
            agentDebugLog("finalize.nochange items=\(items.count)")
        }
    }

    private func renderMarkdownForRecentAssistantItems(limit: Int) {
        guard limit > 0 else { return }
        let indices = Array(items.indices.reversed().filter { items[$0].kind == .assistant }.prefix(limit))
        guard !indices.isEmpty else { return }

        var renderedCount = 0
        var totalMs: Double = 0
        var slowestMs: Double = 0
        var slowestID = ""
        for index in indices {
            let started = CFAbsoluteTimeGetCurrent()
            renderMarkdownIfNeeded(forItemAt: index)
            let elapsedMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
            renderedCount += 1
            totalMs += elapsedMs
            if elapsedMs > slowestMs {
                slowestMs = elapsedMs
                slowestID = items[index].id
            }
        }
        if renderedCount > 0 {
            agentPerfLog(
                "renderMarkdownForRecentAssistantItems count=\(renderedCount) totalMs=\(String(format: "%.2f", totalMs)) avgMs=\(String(format: "%.2f", totalMs / Double(renderedCount))) slowestMs=\(String(format: "%.2f", slowestMs)) slowestID=\(slowestID)"
            )
        }
    }

    private func scheduleDeferredMarkdownRendering(generation: UInt64) {
        let pendingIndices = items.indices.filter {
            items[$0].kind == .assistant && items[$0].renderedMarkdown == nil && !items[$0].text.isEmpty
        }
        guard !pendingIndices.isEmpty else { return }

        markdownBackfillTask = Task { @MainActor [weak self] in
            guard let self else { return }
            let started = CFAbsoluteTimeGetCurrent()
            var renderedCount = 0

            for chunkStart in stride(from: 0, to: pendingIndices.count, by: deferredAssistantMarkdownBatchSize) {
                guard !Task.isCancelled, generation == self.markdownRenderGeneration else { return }
                let chunkEnd = min(chunkStart + deferredAssistantMarkdownBatchSize, pendingIndices.count)
                for index in pendingIndices[chunkStart..<chunkEnd] {
                    guard self.items.indices.contains(index), self.items[index].renderedMarkdown == nil else { continue }
                    self.renderMarkdownIfNeeded(forItemAt: index)
                    renderedCount += 1
                }
                if chunkEnd < pendingIndices.count {
                    await Task.yield()
                }
            }

            let totalMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
            agentPerfLog(
                "renderMarkdownDeferred count=\(renderedCount) totalMs=\(String(format: "%.2f", totalMs)) batchSize=\(self.deferredAssistantMarkdownBatchSize)"
            )
        }
    }

    private func renderMarkdownIfNeeded(forItemAt index: Int) {
        guard items.indices.contains(index), items[index].kind == .assistant else { return }
        agentDebugLog("markdown.render.begin index=\(index) id=\(items[index].id) chars=\(items[index].text.count) revision=\(items[index].revision)")
        let started = CFAbsoluteTimeGetCurrent()
        let renderedMarkdown = controller.renderMarkdown(items[index].text)
        items[index].renderedMarkdown = renderedMarkdown
        touchRevision(&items[index])
        let elapsedMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
        if elapsedMs >= 8 {
            agentPerfLog(
                "assistantMarkdown.render id=\(items[index].id) chars=\(items[index].text.count) blocks=\(renderedMarkdown.blocks.count) runs=\(renderedMarkdown.runs.count) ms=\(String(format: "%.2f", elapsedMs))"
            )
        }
    }

    private func parseHistory(_ raw: Any?) -> [EditorAgentTranscriptItem] {
        guard let rows = raw as? [[String: Any]] else { return [] }
        return rows.compactMap { row in
            guard let id = row["id"] as? String,
                  let kindRaw = row["kind"] as? String,
                  let kind = EditorAgentTranscriptItem.Kind(rawValue: kindRaw),
                  let text = row["text"] as? String
            else {
                return nil
            }
            let noteStyle = EditorAgentTranscriptItem.NoteStyle(rawValue: row["noteStyle"] as? String ?? "") ?? .plain
            let renderedMarkdown: EditorRenderedMarkdown?
            if kind == .note {
                renderedMarkdown = renderedMarkdownForNote(text: text, noteStyle: noteStyle)
            } else {
                renderedMarkdown = nil
            }
            return EditorAgentTranscriptItem(
                id: id,
                correlationID: row["correlationID"] as? String ?? id,
                kind: kind,
                title: row["title"] as? String,
                text: text,
                isStreaming: row["isStreaming"] as? Bool ?? false,
                status: row["status"] as? String,
                contextSummary: row["context"] as? String,
                toolInputJSON: row["toolInputJSON"] as? String,
                renderedMarkdown: renderedMarkdown,
                noteStyle: noteStyle,
                tokensBefore: row["tokensBefore"] as? Int
            )
        }
    }

    private func renderedMarkdownForNote(text: String, noteStyle: EditorAgentTranscriptItem.NoteStyle) -> EditorRenderedMarkdown? {
        guard noteStyle != .plain, !text.isEmpty else { return nil }
        return controller.renderMarkdown(text)
    }

    private func parseCommands(_ raw: Any?) -> [EditorAgentCommand] {
        guard let rows = raw as? [[String: Any]] else { return [] }
        return rows.compactMap { row in
            guard let name = row["name"] as? String else { return nil }
            return EditorAgentCommand(
                name: name,
                description: row["description"] as? String ?? "",
                source: row["source"] as? String ?? "prompt"
            )
        }
    }

    private func parseRecentSessions(_ raw: [String: Any]) -> [EditorAgentRecentSession] {
        guard let rows = raw["items"] as? [[String: Any]] else {
            if let directRows = raw["result"] as? [[String: Any]] {
                return directRows.compactMap(EditorAgentRecentSession.init)
            }
            return []
        }
        return rows.compactMap(EditorAgentRecentSession.init)
    }

    private func parseModels(_ raw: Any?) -> [EditorAgentModel] {
        guard let rows = raw as? [[String: Any]] else { return [] }
        return rows.compactMap { row in
            guard let provider = row["provider"] as? String,
                  let id = row["id"] as? String
            else {
                return nil
            }
            return EditorAgentModel(
                provider: provider,
                id: id,
                name: row["name"] as? String ?? id,
                reference: row["reference"] as? String ?? "\(provider)/\(id)",
                isCurrent: row["isCurrent"] as? Bool ?? false
            )
        }
    }

    private func parseFooterInfo(_ raw: Any?) -> EditorAgentFooterInfo? {
        guard let row = raw as? [String: Any],
              let cwd = row["cwd"] as? String,
              !cwd.isEmpty
        else {
            return nil
        }
        return EditorAgentFooterInfo(
            cwd: cwd,
            gitBranch: row["gitBranch"] as? String,
            sessionName: row["sessionName"] as? String,
            totalInput: row["totalInput"] as? Int ?? 0,
            totalOutput: row["totalOutput"] as? Int ?? 0,
            totalCacheRead: row["totalCacheRead"] as? Int ?? 0,
            totalCacheWrite: row["totalCacheWrite"] as? Int ?? 0,
            totalCost: row["totalCost"] as? Double ?? 0,
            usingSubscription: row["usingSubscription"] as? Bool ?? false,
            contextTokens: row["contextTokens"] as? Int,
            contextPercent: row["contextPercent"] as? Double,
            contextWindow: row["contextWindow"] as? Int,
            autoCompactEnabled: row["autoCompactEnabled"] as? Bool ?? false,
            modelProvider: row["modelProvider"] as? String,
            modelID: row["modelID"] as? String,
            modelName: row["modelName"] as? String,
            modelSupportsReasoning: row["modelSupportsReasoning"] as? Bool ?? false,
            thinkingLevel: row["thinkingLevel"] as? String ?? "off",
            availableProviderCount: row["availableProviderCount"] as? Int ?? 0
        )
    }

    private func formatFooterContextUsage(_ footer: [String: Any]) -> String? {
        guard let contextWindow = footer["contextWindow"] as? Int, contextWindow > 0 else { return nil }
        if let percent = footer["contextPercent"] as? Double {
            return "\(String(format: "%.1f", percent))%%/\(formatTokenCount(contextWindow))"
        }
        return "?/\(formatTokenCount(contextWindow))"
    }

    private func formatContextUsage(_ usage: [String: Any]) -> String? {
        guard let contextWindow = usage["contextWindow"] as? Int, contextWindow > 0 else { return nil }
        if let percent = usage["percent"] as? Double {
            return "\(String(format: "%.1f", percent))%%/\(formatTokenCount(contextWindow))"
        }
        if let tokens = usage["tokens"] as? Int {
            return "\(tokens)/\(contextWindow)"
        }
        return "?/\(formatTokenCount(contextWindow))"
    }

    private func formatTokenCount(_ count: Int) -> String {
        if count < 1_000 { return "\(count)" }
        if count < 10_000 { return String(format: "%.1fk", Double(count) / 1_000) }
        if count < 1_000_000 { return "\(Int(round(Double(count) / 1_000)))k" }
        if count < 10_000_000 { return String(format: "%.1fM", Double(count) / 1_000_000) }
        return "\(Int(round(Double(count) / 1_000_000)))M"
    }

    private func handleRuntimeRequest(method: String, params: [String: Any]) throws -> Any {
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
        case "editor.editFile":
            return try editFile(params: params)
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

    private func requirePath(_ params: [String: Any]) throws -> String {
        guard let rawPath = params["path"] as? String, !rawPath.isEmpty else {
            throw NSError(domain: "EditorAgentPanel", code: 6, userInfo: [NSLocalizedDescriptionKey: "Missing path."])
        }
        return rawPath
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

struct ExactTextReplacement {
    let oldText: String
    let newText: String

    init(dictionary: [String: Any]) throws {
        guard let oldText = dictionary["oldText"] as? String,
              let newText = dictionary["newText"] as? String
        else {
            throw NSError(domain: "EditorAgentPanel", code: 7, userInfo: [NSLocalizedDescriptionKey: "Invalid edit payload."])
        }
        self.oldText = oldText
        self.newText = newText
    }
}

func applyExactTextReplacements(
    _ edits: [ExactTextReplacement],
    to original: String,
    path: String
) throws -> (updated: String, firstChangedLine: Int?) {
    var ranges: [(range: Range<String.Index>, replacement: ExactTextReplacement)] = []

    for edit in edits {
        guard !edit.oldText.isEmpty else {
            throw NSError(domain: "EditorAgentPanel", code: 8, userInfo: [NSLocalizedDescriptionKey: "oldText must not be empty for \(path)."])
        }
        var searchRange = original.startIndex..<original.endIndex
        var matches: [Range<String.Index>] = []
        while let range = original.range(of: edit.oldText, options: [], range: searchRange) {
            matches.append(range)
            searchRange = range.upperBound..<original.endIndex
        }
        guard matches.count == 1, let range = matches.first else {
            throw NSError(
                domain: "EditorAgentPanel",
                code: 9,
                userInfo: [NSLocalizedDescriptionKey: "Expected exactly one match for oldText in \(path), found \(matches.count)."]
            )
        }
        ranges.append((range, edit))
    }

    let sortedRanges = ranges.sorted { $0.range.lowerBound < $1.range.lowerBound }
    for (lhs, rhs) in zip(sortedRanges, sortedRanges.dropFirst()) {
        guard lhs.range.upperBound <= rhs.range.lowerBound else {
            throw NSError(
                domain: "EditorAgentPanel",
                code: 10,
                userInfo: [NSLocalizedDescriptionKey: "Edit ranges overlap in \(path). Merge nearby edits into a single replacement."]
            )
        }
    }

    var updated = original
    for entry in sortedRanges.reversed() {
        updated.replaceSubrange(entry.range, with: entry.replacement.newText)
    }

    let firstChangedLine = sortedRanges.first.map { rangeEntry in
        original.distance(from: original.startIndex, to: rangeEntry.range.lowerBound)
    }.map { utf16Offset in
        let prefix = original.utf16.prefix(utf16Offset)
        return prefix.reduce(into: 1) { count, scalar in
            if scalar == 10 {
                count += 1
            }
        }
    }

    return (updated, firstChangedLine)
}

private final class EditorAgentRuntimeStdoutProcessor: @unchecked Sendable {
    final class MessageBox: @unchecked Sendable {
        let object: [String: Any]

        init(object: [String: Any]) {
            self.object = object
        }
    }

    var onMessage: ((MessageBox) -> Void)?

    private let queue = DispatchQueue(label: "EditorAgentRuntimeTransport.stdout")
    private var buffer = Data()
    private var isInvalidated = false

    func append(_ data: Data) {
        queue.async { [weak self] in
            guard let self, !self.isInvalidated else { return }
            self.buffer.append(data)
            while let newlineIndex = self.buffer.firstIndex(of: 0x0A) {
                let line = Data(self.buffer.prefix(upTo: newlineIndex))
                self.buffer.removeSubrange(...newlineIndex)
                guard !line.isEmpty,
                      let object = try? JSONSerialization.jsonObject(with: line) as? [String: Any]
                else {
                    continue
                }
                self.onMessage?(MessageBox(object: object))
            }
        }
    }

    func invalidate() {
        queue.async { [weak self] in
            self?.isInvalidated = true
            self?.buffer.removeAll(keepingCapacity: false)
        }
    }
}

@MainActor
final class EditorAgentRuntimeTransport {
    typealias EventHandler = @MainActor (_ sessionPath: String?, _ event: String, _ payload: [String: Any]) -> Void
    typealias RequestHandler = @MainActor (_ method: String, _ params: [String: Any]) async throws -> Any

    var onEvent: EventHandler?
    var onRequest: RequestHandler?

    private var process: Process?
    private var stdinPipe: Pipe?
    private var stdoutPipe: Pipe?
    private var stderrPipe: Pipe?
    private var stdoutProcessor: EditorAgentRuntimeStdoutProcessor?
    private var pending: [String: CheckedContinuation<Data, Error>] = [:]
    private var nextRequestID = 1

    func start(cwd: String) throws {
        guard process == nil else { return }
        guard let helperURL = Bundle.module.url(forResource: "pi-agent-panel-helper", withExtension: "mjs") else {
            throw NSError(domain: "EditorAgentPanel", code: 20, userInfo: [NSLocalizedDescriptionKey: "Missing pi helper resource."])
        }
        guard let nodeURL = Self.resolveNodeBinary() else {
            throw NSError(domain: "EditorAgentPanel", code: 21, userInfo: [NSLocalizedDescriptionKey: "Unable to find a Node.js binary for the pi agent helper."])
        }
        guard let piDist = Self.resolvePiCodingAgentDist() else {
            throw NSError(domain: "EditorAgentPanel", code: 22, userInfo: [NSLocalizedDescriptionKey: "Unable to locate @mariozechner/pi-coding-agent."])
        }
        let typeBoxDist = Self.resolveTypeBoxDist(fromPiDist: piDist)

        let process = Process()
        let stdinPipe = Pipe()
        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.executableURL = nodeURL
        process.arguments = [helperURL.path]
        process.currentDirectoryURL = URL(fileURLWithPath: cwd)
        var environment = ProcessInfo.processInfo.environment
        environment["PI_CODING_AGENT_DIST"] = piDist.path
        environment["PI_TYPEBOX_DIST"] = typeBoxDist.path
        process.environment = environment
        process.standardInput = stdinPipe
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe
        process.terminationHandler = { [weak self] process in
            Task { @MainActor in
                self?.handleTermination(status: process.terminationStatus)
            }
        }

        let stdoutProcessor = EditorAgentRuntimeStdoutProcessor()
        stdoutProcessor.onMessage = { [weak self] box in
            Task { @MainActor [weak self] in
                self?.handleDecodedMessage(box.object)
            }
        }

        self.process = process
        self.stdinPipe = stdinPipe
        self.stdoutPipe = stdoutPipe
        self.stderrPipe = stderrPipe
        self.stdoutProcessor = stdoutProcessor

        stdoutPipe.fileHandleForReading.readabilityHandler = { [weak stdoutProcessor] handle in
            let data = handle.availableData
            guard !data.isEmpty else { return }
            stdoutProcessor?.append(data)
        }
        stderrPipe.fileHandleForReading.readabilityHandler = { handle in
            let data = handle.availableData
            guard !data.isEmpty, let text = String(data: data, encoding: .utf8), !text.isEmpty else { return }
            fputs("[EditorAgentPanel] \(text)", stderr)
        }

        try process.run()
    }

    func stop() {
        stdoutPipe?.fileHandleForReading.readabilityHandler = nil
        stderrPipe?.fileHandleForReading.readabilityHandler = nil
        process?.terminate()
        process = nil
        stdinPipe = nil
        stdoutPipe = nil
        stderrPipe = nil
        stdoutProcessor?.invalidate()
        stdoutProcessor = nil
        for (_, continuation) in pending {
            continuation.resume(throwing: NSError(domain: "EditorAgentPanel", code: 23, userInfo: [NSLocalizedDescriptionKey: "Agent runtime stopped."]))
        }
        pending.removeAll()
    }

    func request(method: String, params: [String: Any]) async throws -> [String: Any] {
        guard process != nil else {
            throw NSError(domain: "EditorAgentPanel", code: 24, userInfo: [NSLocalizedDescriptionKey: "Agent runtime is not running."])
        }
        let requestID = String(nextRequestID)
        nextRequestID += 1
        let data = try await withCheckedThrowingContinuation { continuation in
            pending[requestID] = continuation
            do {
                try send([
                    "type": "request",
                    "id": requestID,
                    "method": method,
                    "params": params,
                ])
            } catch {
                pending.removeValue(forKey: requestID)
                continuation.resume(throwing: error)
            }
        }
        let object = try JSONSerialization.jsonObject(with: data)
        return object as? [String: Any] ?? ["result": object]
    }

    private func send(_ object: [String: Any]) throws {
        let data = try JSONSerialization.data(withJSONObject: object)
        guard let stdinPipe else {
            throw NSError(domain: "EditorAgentPanel", code: 25, userInfo: [NSLocalizedDescriptionKey: "Missing stdin pipe."])
        }
        stdinPipe.fileHandleForWriting.write(data)
        stdinPipe.fileHandleForWriting.write(Data([0x0A]))
    }

    private func handleDecodedMessage(_ object: [String: Any]) {
        guard let type = object["type"] as? String else {
            return
        }
        switch type {
        case "response":
            guard let id = object["id"] as? String,
                  let continuation = pending.removeValue(forKey: id)
            else {
                return
            }
            if let error = object["error"] as? [String: Any] {
                continuation.resume(throwing: NSError(domain: "EditorAgentPanel", code: 26, userInfo: [NSLocalizedDescriptionKey: error["message"] as? String ?? "Unknown runtime error"]))
            } else {
                let payload = object["result"] ?? NSNull()
                if let data = try? JSONSerialization.data(withJSONObject: payload) {
                    continuation.resume(returning: data)
                } else if let data = try? JSONSerialization.data(withJSONObject: ["result": String(describing: payload)]) {
                    continuation.resume(returning: data)
                } else {
                    continuation.resume(throwing: NSError(domain: "EditorAgentPanel", code: 28, userInfo: [NSLocalizedDescriptionKey: "Invalid runtime response payload."]))
                }
            }
        case "event":
            guard let event = object["event"] as? String else { return }
            onEvent?(object["sessionPath"] as? String, event, object["payload"] as? [String: Any] ?? [:])
        case "request":
            guard let method = object["method"] as? String,
                  let id = object["id"] as? String
            else {
                return
            }
            Task { @MainActor in
                do {
                    let result = try await onRequest?(method, object["params"] as? [String: Any] ?? [:]) ?? [:]
                    try send([
                        "type": "response",
                        "id": id,
                        "result": result,
                    ])
                } catch {
                    try? send([
                        "type": "response",
                        "id": id,
                        "error": ["message": error.localizedDescription],
                    ])
                }
            }
        default:
            break
        }
    }

    private func handleTermination(status: Int32) {
        process = nil
        stdinPipe = nil
        stdoutPipe = nil
        stderrPipe = nil
        stdoutProcessor?.invalidate()
        stdoutProcessor = nil
        let error = NSError(domain: "EditorAgentPanel", code: 27, userInfo: [NSLocalizedDescriptionKey: "Agent runtime exited with status \(status)."])
        for (_, continuation) in pending {
            continuation.resume(throwing: error)
        }
        pending.removeAll()
        onEvent?(nil, "runtime_error", ["message": error.localizedDescription])
    }

    private static func resolveNodeBinary() -> URL? {
        let fileManager = FileManager.default
        let home = NSHomeDirectory()
        let directCandidates = [
            ProcessInfo.processInfo.environment["PI_NODE_BINARY"],
            "\(home)/.nvm/versions/node/v24.13.0/bin/node",
            "\(home)/.nvm/current/bin/node",
            "/opt/homebrew/bin/node",
            "/usr/local/bin/node",
            "/usr/bin/node",
        ].compactMap { $0 }
        for candidate in directCandidates where fileManager.isExecutableFile(atPath: candidate) {
            return URL(fileURLWithPath: candidate)
        }
        let nvmRoot = URL(fileURLWithPath: home).appendingPathComponent(".nvm/versions/node", isDirectory: true)
        if let enumerator = fileManager.enumerator(at: nvmRoot, includingPropertiesForKeys: nil) {
            for case let url as URL in enumerator where url.lastPathComponent == "node" && fileManager.isExecutableFile(atPath: url.path) {
                return url
            }
        }
        return nil
    }

    private static func resolvePiCodingAgentDist() -> URL? {
        let fileManager = FileManager.default
        let home = NSHomeDirectory()
        let candidates = [
            ProcessInfo.processInfo.environment["PI_CODING_AGENT_DIST"],
            "\(home)/.nvm/versions/node/v24.13.0/lib/node_modules/@mariozechner/pi-coding-agent/dist/index.js",
            "/opt/homebrew/lib/node_modules/@mariozechner/pi-coding-agent/dist/index.js",
            "/usr/local/lib/node_modules/@mariozechner/pi-coding-agent/dist/index.js",
        ].compactMap { $0 }
        for candidate in candidates where fileManager.fileExists(atPath: candidate) {
            return URL(fileURLWithPath: candidate)
        }
        let nvmRoot = URL(fileURLWithPath: home).appendingPathComponent(".nvm/versions/node", isDirectory: true)
        if let enumerator = fileManager.enumerator(at: nvmRoot, includingPropertiesForKeys: nil) {
            for case let url as URL in enumerator where url.path.hasSuffix("/lib/node_modules/@mariozechner/pi-coding-agent/dist/index.js") {
                return url
            }
        }
        return nil
    }

    private static func resolveTypeBoxDist(fromPiDist piDist: URL) -> URL {
        piDist.deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("node_modules/@sinclair/typebox/build/esm/index.mjs")
    }
}

private extension EditorAgentRecentSession {
    init?(dictionary: [String: Any]) {
        guard let id = dictionary["id"] as? String,
              let path = dictionary["path"] as? String
        else {
            return nil
        }
        self.id = id
        self.path = path
        self.name = dictionary["name"] as? String
        self.firstMessage = dictionary["firstMessage"] as? String ?? ""
        self.modified = dictionary["modified"] as? String ?? ""
    }
}

private extension String {
    var nilIfEmpty: String? {
        isEmpty ? nil : self
    }
}
