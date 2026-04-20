import AppKit
import SwiftUI

// MARK: - Layout constants

private enum AgentLayout {
    static let maxContentWidth: CGFloat = 760
    static let horizontalPadding: CGFloat = 20
    static let transcriptSpacing: CGFloat = 10
    static let composerCornerRadius: CGFloat = 22
    static let composerContainerPadding: CGFloat = 10
    static let composerRowSpacing: CGFloat = 8
    static let composerControlSize: CGFloat = 30
    static let composerSendSize: CGFloat = 30
    static let composerIconSize: CGFloat = 14
    static let composerTextTopInset: CGFloat = 4
    static let composerTextLeadingInset: CGFloat = 10
    static let composerMinHeight: CGFloat = 46
    static let composerOverlayGap: CGFloat = 12
    static let composerAttachmentSpacing: CGFloat = 8
    static let composerAttachmentPreviewSize: CGFloat = 36
    static let composerAttachmentCornerRadius: CGFloat = 10
    static let userBubbleMaxWidthFraction: CGFloat = 0.78
    static let userBubbleCornerRadius: CGFloat = 16
    static let toolRowCornerRadius: CGFloat = 10
}

// MARK: - Public entry point

struct EditorAgentSidebarView: View {
    @ObservedObject var store: EditorAgentPanelStore
    @ObservedObject var hostModel: EditorAgentPaneHostModel

    @State private var inputText = ""
    @State private var transcriptJumpToLatestToken = 0
    @State private var isTranscriptPinnedToBottom = true
    @State private var showsJumpToLatest = false
    @State private var expandedToolItemIDs: Set<String> = []
    @State private var expandedSummaryItemIDs: Set<String> = []
    @State private var commandSelectionIndex = 0
    @State private var modelSelectionIndex = 0
    @State private var resumeSelectionIndex = 0
    @State private var composerBarHeight: CGFloat = 0
    @State private var composerBarFrame: CGRect = .zero
    @State private var suggestionsFrame: CGRect = .zero
    @State private var isComposerFocused = false

    private var backgroundColor: NSColor { hostModel.backgroundColor }
    private var selectionColor: NSColor { hostModel.selectionColor }
    private var topScrimHeight: CGFloat { hostModel.topScrimHeight }

    var body: some View {
        ZStack {
            Color(nsColor: backgroundColor)
                .ignoresSafeArea()

            VStack(spacing: 0) {
                transcriptArea
                    .layoutPriority(1)

                composerArea
            }
            .frame(maxWidth: AgentLayout.maxContentWidth)
            .frame(maxWidth: .infinity)
        }
        .overlay(alignment: .top) {
            AgentPaneTopScrim(height: topScrimHeight, backgroundColor: backgroundColor)
        }
        .environment(\.colorScheme, backgroundColor.agentIsLightColor ? .light : .dark)
        .contentShape(Rectangle())
        .simultaneousGesture(
            TapGesture().onEnded {
                store.activateAgentSurfaceIfNeeded()
            }
        )
        .onAppear {
            store.startIfNeeded()
            DispatchQueue.main.async {
                isComposerFocused = true
            }
        }
        .onChange(of: store.items) { _, newItems in
            expandedToolItemIDs.formIntersection(Set(newItems.lazy.filter { $0.kind == .tool }.map(\.id)))
            expandedSummaryItemIDs.formIntersection(Set(newItems.lazy.filter { $0.kind == .note && $0.noteStyle != .plain }.map(\.id)))
            if newItems.isEmpty {
                isTranscriptPinnedToBottom = true
                showsJumpToLatest = false
            }
        }
        .onChange(of: isModelPickerVisible) { _, visible in
            modelSelectionIndex = 0
            if visible {
                store.refreshModels()
            }
        }
        .onChange(of: isResumePickerVisible) { _, visible in
            resumeSelectionIndex = 0
            if visible {
                store.refreshRecentSessions(force: true)
            }
        }
        .onChange(of: filteredCommands.map(\.id).joined(separator: "|")) { _, _ in
            commandSelectionIndex = min(commandSelectionIndex, max(filteredCommands.count - 1, 0))
        }
        .onChange(of: filteredModels.map(\.id).joined(separator: "|")) { _, _ in
            modelSelectionIndex = min(modelSelectionIndex, max(filteredModels.count - 1, 0))
        }
        .onChange(of: filteredRecentSessions.map(\.id).joined(separator: "|")) { _, _ in
            resumeSelectionIndex = min(resumeSelectionIndex, max(filteredRecentSessions.count - 1, 0))
        }
        .onChange(of: inputText) { _, newValue in
            agentDebugLog("composer.input text=\(String(newValue.prefix(80)).replacingOccurrences(of: "\n", with: "\\n")) commands=\(filteredCommands.count) modelPicker=\(isModelPickerVisible) resumePicker=\(isResumePickerVisible)")
        }
    }

    // MARK: Transcript

    @ViewBuilder
    private var transcriptArea: some View {
        if store.items.isEmpty {
            AgentEmptyStateView(
                sessionSubtitle: store.sessionSubtitle,
                contextUsageText: store.contextUsageText,
                footerInfo: store.footerInfo,
                isRunning: store.isRunning,
                recentSessions: store.recentSessions,
                onNewSession: startNewSession,
                onResumeSession: resumeSession
            )
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .padding(.horizontal, AgentLayout.horizontalPadding)
            .padding(.top, topScrimHeight + 8)
            .task { store.refreshRecentSessions() }
        } else {
            AgentTranscriptView(
                items: store.items,
                transcriptRevision: store.transcriptRevision,
                selectionColor: selectionColor,
                jumpToLatestToken: $transcriptJumpToLatestToken,
                isPinnedToBottom: $isTranscriptPinnedToBottom,
                showsJumpToLatest: $showsJumpToLatest,
                expandedToolItemIDs: $expandedToolItemIDs,
                expandedSummaryItemIDs: $expandedSummaryItemIDs,
                topInset: topScrimHeight + 12
            )
            .padding(.horizontal, AgentLayout.horizontalPadding)
        }
    }

    // MARK: Composer

    @ViewBuilder
    private var composerArea: some View {
        VStack(spacing: 8) {
            if let compactionStatus = store.compactionStatus {
                AgentCompactionStatusView(
                    status: compactionStatus,
                    onCancel: store.abort
                )
                .padding(.horizontal, AgentLayout.horizontalPadding)
            }

            if let errorMessage = store.errorMessage, !errorMessage.isEmpty {
                AgentErrorBanner(message: errorMessage)
                    .padding(.horizontal, AgentLayout.horizontalPadding)
            }

            AgentComposerBar(
                inputText: $inputText,
                isFocused: $isComposerFocused,
                placeholder: placeholderText,
                isRunning: store.isRunning || store.compactionStatus != nil,
                isCompacting: store.compactionStatus != nil,
                isReady: store.isRuntimeReady,
                canSend: canSubmitComposer,
                isFollowingAgent: store.isFollowingAgent,
                followStatusText: store.followStatusText,
                onSend: sendCurrent,
                onFollowAndSend: followAndSendCurrent,
                onCancel: store.abort,
                onToggleFollow: store.toggleAgentFollow,
                onInsertCommand: insertCommand,
                onMoveUp: moveOverlaySelectionUp,
                onMoveDown: moveOverlaySelectionDown,
                onDismissOverlay: dismissOverlay,
                onCycleThinkingLevel: store.cycleThinkingLevel,
                onActivateSurface: store.activateAgentSurfaceIfNeeded,
                commands: filteredCommands,
                commandSelectionIndex: commandSelectionIndex,
                showsModelPicker: isModelPickerVisible,
                models: filteredModels,
                modelSelectionIndex: modelSelectionIndex,
                onPickModel: selectModel,
                showsResumePicker: isResumePickerVisible,
                recentSessionSuggestions: filteredRecentSessions,
                resumeSelectionIndex: resumeSelectionIndex,
                onPickResumeSession: resumeSession,
                footerInfo: store.footerInfo,
                sessionSubtitle: store.sessionSubtitle,
                contextUsageText: store.contextUsageText,
                onNewSession: startNewSession,
                onShowRecentSessions: { store.refreshRecentSessions(force: true) },
                recentSessions: store.recentSessions,
                onResumeSession: resumeSession
            )
            .overlay(alignment: .bottomLeading) {
                composerSuggestionsOverlay
                    .offset(y: -(composerBarHeight + AgentLayout.composerOverlayGap))
                    .zIndex(20)
            }
            .padding(.horizontal, AgentLayout.horizontalPadding)
            .background {
                AgentFrameObserver { frame in
                    let nextHeight = frame.height
                    if abs(composerBarHeight - nextHeight) > 0.5 {
                        composerBarHeight = nextHeight
                        agentDebugLog("composer.height value=\(Int(nextHeight.rounded())) commands=\(filteredCommands.count) modelPicker=\(isModelPickerVisible) resumePicker=\(isResumePickerVisible)")
                    }
                    if !frame.equalTo(composerBarFrame) {
                        composerBarFrame = frame
                        agentDebugLog("composer.frame x=\(Int(frame.minX.rounded())) y=\(Int(frame.minY.rounded())) w=\(Int(frame.width.rounded())) h=\(Int(frame.height.rounded()))")
                    }
                }
            }
            .onChange(of: overlayDebugSignature) { _, _ in
                if suggestionsFrame != .zero {
                    agentDebugLog("composer.overlay frame x=\(Int(suggestionsFrame.minX.rounded())) y=\(Int(suggestionsFrame.minY.rounded())) w=\(Int(suggestionsFrame.width.rounded())) h=\(Int(suggestionsFrame.height.rounded())) lift=\(Int((composerBarHeight + AgentLayout.composerOverlayGap).rounded()))")
                } else {
                    agentDebugLog("composer.overlay hidden lift=\(Int((composerBarHeight + AgentLayout.composerOverlayGap).rounded()))")
                }
            }
            .zIndex(10)
        }
        .padding(.top, 12)
        .padding(.bottom, 16)
    }

    // MARK: Derived state

    private var composerSuggestionsOverlay: some View {
        Group {
            if isModelPickerVisible {
                AgentModelSuggestionsView(
                    models: filteredModels,
                    selectedIndex: modelSelectionIndex,
                    onPick: selectModel
                )
            } else if isResumePickerVisible {
                AgentResumeSuggestionsView(
                    sessions: filteredRecentSessions,
                    selectedIndex: resumeSelectionIndex,
                    onPick: resumeSession
                )
            } else if !filteredCommands.isEmpty {
                AgentCommandSuggestionsView(
                    commands: filteredCommands,
                    selectedIndex: commandSelectionIndex,
                    onPick: insertCommand
                )
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .allowsHitTesting(isModelPickerVisible || isResumePickerVisible || !filteredCommands.isEmpty)
        .background {
            AgentFrameObserver { frame in
                guard !frame.equalTo(suggestionsFrame) else { return }
                suggestionsFrame = frame
                if frame == .zero {
                    agentDebugLog("composer.overlay.frame zero")
                } else {
                    agentDebugLog("composer.overlay.frame x=\(Int(frame.minX.rounded())) y=\(Int(frame.minY.rounded())) w=\(Int(frame.width.rounded())) h=\(Int(frame.height.rounded()))")
                }
            }
        }
    }

    private var overlayDebugSignature: String {
        "commands=\(filteredCommands.count)|models=\(filteredModels.count)|sessions=\(filteredRecentSessions.count)|modelPicker=\(isModelPickerVisible)|resumePicker=\(isResumePickerVisible)|height=\(Int(composerBarHeight.rounded()))"
    }

    private var trimmedInput: String {
        inputText.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var canSubmitComposer: Bool {
        guard store.isRuntimeReady, !store.isRunning, store.compactionStatus == nil else { return false }
        if isModelPickerVisible {
            return resolvedExactModelMatch != nil || filteredModels.indices.contains(modelSelectionIndex)
        }
        if isResumePickerVisible {
            return resolvedExactResumeSessionMatch != nil || filteredRecentSessions.indices.contains(resumeSelectionIndex)
        }
        if showsCommandSuggestions {
            return filteredCommands.indices.contains(commandSelectionIndex)
        }
        return !trimmedInput.isEmpty
    }

    private var placeholderText: String {
        if !store.isRuntimeReady { return "Starting pi runtime…" }
        if let compactionStatus = store.compactionStatus { return compactionStatus.placeholder }
        if store.isRunning { return "pi is working…" }
        return "Ask pi anything · / for commands"
    }

    private var slashCommandName: String? {
        guard trimmedInput.hasPrefix("/") else { return nil }
        let body = String(trimmedInput.dropFirst())
        let commandPart = body.split(maxSplits: 1, whereSeparator: { $0.isWhitespace }).first.map(String.init) ?? ""
        return commandPart
    }

    private var slashCommandArguments: String? {
        guard trimmedInput.hasPrefix("/") else { return nil }
        let body = String(trimmedInput.dropFirst())
        guard let firstWhitespace = body.firstIndex(where: { $0.isWhitespace }) else { return nil }
        let arguments = String(body[firstWhitespace...]).trimmingCharacters(in: .whitespacesAndNewlines)
        return arguments.isEmpty ? "" : arguments
    }

    private var exactSlashCommandMatch: EditorAgentCommand? {
        guard let slashCommandName else { return nil }
        return store.commands.first { $0.name.caseInsensitiveCompare(slashCommandName) == .orderedSame }
    }

    private var modelCommandQuery: String? {
        guard exactSlashCommandMatch?.name.caseInsensitiveCompare("model") == .orderedSame else { return nil }
        return slashCommandArguments ?? ""
    }

    private var isModelPickerVisible: Bool {
        modelCommandQuery != nil
    }

    private var resumeCommandQuery: String? {
        guard exactSlashCommandMatch?.name.caseInsensitiveCompare("resume") == .orderedSame else { return nil }
        return slashCommandArguments ?? ""
    }

    private var isResumePickerVisible: Bool {
        resumeCommandQuery != nil
    }

    private var showsCommandSuggestions: Bool {
        guard !isModelPickerVisible, !isResumePickerVisible, trimmedInput.hasPrefix("/") else { return false }
        return slashCommandArguments == nil || exactSlashCommandMatch == nil
    }

    private var filteredCommands: [EditorAgentCommand] {
        guard showsCommandSuggestions else { return [] }
        let query = slashCommandName ?? ""
        return fuzzyFilterCommands(store.commands, query: query)
    }

    private var filteredModels: [EditorAgentModel] {
        guard let query = modelCommandQuery else { return [] }
        return fuzzyFilterModels(store.models, query: query)
    }

    private var resolvedExactModelMatch: EditorAgentModel? {
        guard let query = modelCommandQuery else { return nil }
        return exactModelMatch(for: query, models: store.models)
    }

    private var filteredRecentSessions: [EditorAgentRecentSession] {
        guard let query = resumeCommandQuery else { return [] }
        return fuzzyFilterSessions(store.recentSessions, query: query)
    }

    private var resolvedExactResumeSessionMatch: EditorAgentRecentSession? {
        guard let query = resumeCommandQuery else { return nil }
        return exactRecentSessionMatch(for: query, sessions: store.recentSessions)
    }

    private func sendCurrent() {
        sendCurrent(enablingFollow: false)
    }

    private func followAndSendCurrent() {
        sendCurrent(enablingFollow: true)
    }

    private func sendCurrent(enablingFollow: Bool) {
        guard canSubmitComposer else { return }

        if let resolvedExactModelMatch {
            selectModel(resolvedExactModelMatch)
            return
        }
        if isModelPickerVisible, filteredModels.indices.contains(modelSelectionIndex) {
            selectModel(filteredModels[modelSelectionIndex])
            return
        }

        if let resolvedExactResumeSessionMatch {
            resumeSession(resolvedExactResumeSessionMatch)
            return
        }
        if isResumePickerVisible {
            guard let session = filteredRecentSessions[safe: resumeSelectionIndex] else {
                store.errorMessage = "No matching sessions to resume."
                return
            }
            resumeSession(session)
            return
        }

        if showsCommandSuggestions, exactSlashCommandMatch == nil, let command = filteredCommands[safe: commandSelectionIndex] {
            insertCommand(command)
            return
        }

        if let command = exactSlashCommandMatch, command.isBuiltin {
            handleBuiltinCommand(command)
            return
        }

        let text = trimmedInput
        inputText = ""
        transcriptJumpToLatestToken &+= 1
        if enablingFollow {
            store.setAgentFollowEnabled(true)
        }
        store.sendPrompt(text)
    }

    private func insertCommand(_ command: EditorAgentCommand) {
        inputText = "/\(command.name) "
        isComposerFocused = true
    }

    private func selectModel(_ model: EditorAgentModel) {
        inputText = ""
        store.setModel(provider: model.provider, modelID: model.id)
        isComposerFocused = true
    }

    private func startNewSession() {
        expandedToolItemIDs.removeAll()
        expandedSummaryItemIDs.removeAll()
        showsJumpToLatest = false
        inputText = ""
        store.newSession()
    }

    private func resumeSession(_ session: EditorAgentRecentSession) {
        expandedToolItemIDs.removeAll()
        expandedSummaryItemIDs.removeAll()
        showsJumpToLatest = false
        inputText = ""
        store.resumeSession(path: session.path)
        isComposerFocused = true
    }

    private func handleBuiltinCommand(_ command: EditorAgentCommand) {
        let arguments = (slashCommandArguments ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        switch command.name.lowercased() {
        case "new":
            startNewSession()
        case "name":
            guard !arguments.isEmpty else {
                store.errorMessage = "Usage: /name <session name>"
                return
            }
            inputText = ""
            store.setSessionName(arguments)
            isComposerFocused = true
        case "compact":
            inputText = ""
            store.compact(customInstructions: arguments.isEmpty ? nil : arguments)
            isComposerFocused = true
        case "copy":
            guard let text = store.items.last(where: { $0.kind == .assistant && !$0.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty })?.text else {
                store.errorMessage = "There is no assistant message to copy yet."
                return
            }
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
            inputText = ""
            isComposerFocused = true
        case "resume":
            store.errorMessage = "Pick a session from the list to resume."
        case "model":
            store.errorMessage = "Pick a model from the list to switch models."
        default:
            store.errorMessage = "/\(command.name) is not wired up in the editor agent pane yet."
        }
    }

    private func moveOverlaySelectionUp() {
        if isModelPickerVisible {
            guard !filteredModels.isEmpty else { return }
            modelSelectionIndex = modelSelectionIndex == 0 ? filteredModels.count - 1 : modelSelectionIndex - 1
            return
        }
        if isResumePickerVisible {
            guard !filteredRecentSessions.isEmpty else { return }
            resumeSelectionIndex = resumeSelectionIndex == 0 ? filteredRecentSessions.count - 1 : resumeSelectionIndex - 1
            return
        }
        guard !filteredCommands.isEmpty else { return }
        commandSelectionIndex = commandSelectionIndex == 0 ? filteredCommands.count - 1 : commandSelectionIndex - 1
    }

    private func moveOverlaySelectionDown() {
        if isModelPickerVisible {
            guard !filteredModels.isEmpty else { return }
            modelSelectionIndex = modelSelectionIndex == filteredModels.count - 1 ? 0 : modelSelectionIndex + 1
            return
        }
        if isResumePickerVisible {
            guard !filteredRecentSessions.isEmpty else { return }
            resumeSelectionIndex = resumeSelectionIndex == filteredRecentSessions.count - 1 ? 0 : resumeSelectionIndex + 1
            return
        }
        guard !filteredCommands.isEmpty else { return }
        commandSelectionIndex = commandSelectionIndex == filteredCommands.count - 1 ? 0 : commandSelectionIndex + 1
    }

    private func dismissOverlay() {
        if trimmedInput.hasPrefix("/") {
            inputText = ""
        }
    }
}

// MARK: - Empty state

private struct AgentEmptyStateView: View {
    let sessionSubtitle: String
    let contextUsageText: String?
    let footerInfo: EditorAgentFooterInfo?
    let isRunning: Bool
    let recentSessions: [EditorAgentRecentSession]
    let onNewSession: () -> Void
    let onResumeSession: (EditorAgentRecentSession) -> Void

    var body: some View {
        VStack(spacing: 24) {
            Spacer(minLength: 0)

            VStack(spacing: 14) {
                Image(systemName: "brain.head.profile")
                    .font(.system(size: 44, weight: .regular))
                    .foregroundStyle(.secondary)

                VStack(spacing: 6) {
                    Text("pi Agent")
                        .font(.system(size: 20, weight: .semibold))
                        .foregroundStyle(.primary)

                    HStack(spacing: 6) {
                        Text(footerInfo?.modelProvider.flatMap { provider in
                            let model = footerInfo?.modelID ?? sessionSubtitle
                            return provider.isEmpty ? model : "\(provider)/\(model)"
                        } ?? sessionSubtitle)
                        if let usage = contextUsageText {
                            Text("·")
                            Text(usage)
                        }
                    }
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.secondary)

                    Text("Ask pi for edits, reviews, or workspace help.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }
            }

            if !recentSessions.isEmpty {
                AgentRecentSessionsGrid(
                    sessions: recentSessions,
                    onSelect: onResumeSession
                )
                .frame(maxWidth: 520)
            }

            Spacer(minLength: 0)
        }
    }
}

private struct AgentRecentSessionsGrid: View {
    let sessions: [EditorAgentRecentSession]
    let onSelect: (EditorAgentRecentSession) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Recent sessions")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.secondary)
                .textCase(.uppercase)

            VStack(spacing: 6) {
                ForEach(sessions.prefix(5)) { session in
                    AgentRecentSessionRow(session: session) {
                        onSelect(session)
                    }
                }
            }
        }
    }
}

private struct AgentRecentSessionRow: View {
    let session: EditorAgentRecentSession
    let onTap: () -> Void
    @State private var isHovered = false

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 10) {
                Image(systemName: "clock")
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(.secondary)
                    .frame(width: 14)

                VStack(alignment: .leading, spacing: 2) {
                    Text(session.title)
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(.primary)
                        .lineLimit(1)
                    if !session.modified.isEmpty {
                        Text(session.modified)
                            .font(.system(size: 10))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                }

                Spacer(minLength: 8)

                Image(systemName: "arrow.right")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(.secondary)
                    .opacity(isHovered ? 1 : 0.5)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background {
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .fill(isHovered ? Color.primary.opacity(0.06) : Color.primary.opacity(0.03))
            }
            .overlay {
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(Color.primary.opacity(0.08), lineWidth: 0.5)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

// MARK: - Transcript

private struct AgentTranscriptView: View {
    let items: [EditorAgentTranscriptItem]
    let transcriptRevision: Int
    let selectionColor: NSColor
    @Binding var jumpToLatestToken: Int
    @Binding var isPinnedToBottom: Bool
    @Binding var showsJumpToLatest: Bool
    @Binding var expandedToolItemIDs: Set<String>
    @Binding var expandedSummaryItemIDs: Set<String>
    let topInset: CGFloat

    @State private var transcriptContentHeight: CGFloat = 0
    @State private var pendingScrollToBottomWorkItem: DispatchWorkItem?

    private let bottomAnchorID = "agent-transcript-bottom"

    private var rowEntries: [AgentTranscriptRowEntry] {
        items.map(AgentTranscriptRowEntry.init)
    }

    var body: some View {
        ZStack(alignment: .bottomTrailing) {
            ScrollViewReader { proxy in
                ScrollView {
                    VStack(alignment: .leading, spacing: AgentLayout.transcriptSpacing) {
                        ForEach(rowEntries) { entry in
                            AgentTranscriptRow(
                                item: entry.item,
                                selectionColor: selectionColor,
                                isToolExpanded: expandedToolItemIDs.contains(entry.item.id),
                                isSummaryExpanded: expandedSummaryItemIDs.contains(entry.item.id),
                                onToggleToolExpansion: {
                                    if expandedToolItemIDs.contains(entry.item.id) {
                                        expandedToolItemIDs.remove(entry.item.id)
                                    } else {
                                        expandedToolItemIDs.insert(entry.item.id)
                                    }
                                },
                                onToggleSummaryExpansion: {
                                    if expandedSummaryItemIDs.contains(entry.item.id) {
                                        expandedSummaryItemIDs.remove(entry.item.id)
                                    } else {
                                        expandedSummaryItemIDs.insert(entry.item.id)
                                    }
                                }
                            )
                        }

                        Color.clear
                            .frame(height: 1)
                            .id(bottomAnchorID)
                    }
                    .background {
                        GeometryReader { geometry in
                            Color.clear
                                .preference(key: AgentTranscriptContentHeightPreferenceKey.self, value: geometry.size.height)
                        }
                    }
                    .padding(.top, topInset)
                    .padding(.bottom, 12)
                    .frame(maxWidth: AgentLayout.maxContentWidth, alignment: .leading)
                    .frame(maxWidth: .infinity, alignment: .center)
                }
                .background(
                    AgentTranscriptScrollObserver(isNearBottom: $isPinnedToBottom)
                )
                .onAppear {
                    scheduleScrollTranscriptToBottom(using: proxy, animated: false)
                }
                .onDisappear {
                    pendingScrollToBottomWorkItem?.cancel()
                    pendingScrollToBottomWorkItem = nil
                }
                .onChange(of: transcriptRevision) { _, newValue in
                    agentDebugLog("transcriptRevision.change revision=\(newValue) items=\(items.count) pinned=\(isPinnedToBottom) lastItem=\(items.last?.id ?? "none")")
                    let nextShowsJumpToLatest = !isPinnedToBottom
                    if showsJumpToLatest != nextShowsJumpToLatest {
                        showsJumpToLatest = nextShowsJumpToLatest
                    }
                }
                .onChange(of: jumpToLatestToken) { _, _ in
                    showsJumpToLatest = false
                    scheduleScrollTranscriptToBottom(using: proxy, animated: true)
                }
                .onChange(of: isPinnedToBottom) { _, pinned in
                    if pinned, showsJumpToLatest {
                        showsJumpToLatest = false
                    }
                }
                .onPreferenceChange(AgentTranscriptContentHeightPreferenceKey.self) { newHeight in
                    guard isPinnedToBottom, newHeight > 0 else { return }
                    let oldHeight = transcriptContentHeight
                    guard abs(newHeight - oldHeight) > 0.5 else { return }
                    transcriptContentHeight = newHeight
                    agentDebugLog("transcriptHeight.change old=\(Int(oldHeight.rounded())) new=\(Int(newHeight.rounded())) pinned=\(isPinnedToBottom) revision=\(transcriptRevision)")
                    scheduleScrollTranscriptToBottom(using: proxy, animated: false)
                }
            }

            if showsJumpToLatest {
                AgentJumpToLatestButton {
                    jumpToLatestToken &+= 1
                }
                .padding(.trailing, 12)
                .padding(.bottom, 12)
                .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .animation(.easeOut(duration: 0.18), value: showsJumpToLatest)
    }

    private func scheduleScrollTranscriptToBottom(using proxy: ScrollViewProxy, animated: Bool) {
        pendingScrollToBottomWorkItem?.cancel()
        agentDebugLog("scrollToBottom.request animated=\(animated) revision=\(transcriptRevision) items=\(items.count)")

        let workItem = DispatchWorkItem {
            if animated {
                withAnimation(.easeOut(duration: 0.16)) {
                    proxy.scrollTo(bottomAnchorID, anchor: .bottom)
                }
            } else {
                var transaction = Transaction()
                transaction.disablesAnimations = true
                withTransaction(transaction) {
                    proxy.scrollTo(bottomAnchorID, anchor: .bottom)
                }
            }
            agentDebugLog("scrollToBottom.applied animated=\(animated) revision=\(transcriptRevision) items=\(items.count)")
        }

        pendingScrollToBottomWorkItem = workItem
        DispatchQueue.main.async(execute: workItem)
    }
}

private struct AgentTranscriptRowEntry: Identifiable {
    let item: EditorAgentTranscriptItem

    var id: String {
        item.id
    }

    init(item: EditorAgentTranscriptItem) {
        self.item = item
    }
}

private struct AgentTranscriptContentHeightPreferenceKey: PreferenceKey {
    static let defaultValue: CGFloat = 0

    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = nextValue()
    }
}

private struct AgentTranscriptRow: View {
    let item: EditorAgentTranscriptItem
    let selectionColor: NSColor
    let isToolExpanded: Bool
    let isSummaryExpanded: Bool
    let onToggleToolExpansion: () -> Void
    let onToggleSummaryExpansion: () -> Void

    var body: some View {
        switch item.kind {
        case .user:
            AgentUserMessageView(item: item, selectionColor: selectionColor)
        case .assistant:
            AgentAssistantMessageView(item: item)
                .equatable()
        case .thinking:
            AgentThinkingMessageView(item: item)
                .equatable()
        case .note:
            if item.noteStyle == .plain {
                AgentNoteRow(text: item.text)
                    .equatable()
            } else {
                AgentSummaryRow(
                    item: item,
                    isExpanded: isSummaryExpanded,
                    onToggleExpanded: onToggleSummaryExpansion
                )
            }
        case .tool:
            AgentToolRow(
                item: item,
                isExpanded: isToolExpanded,
                onToggleExpanded: onToggleToolExpansion
            )
        }
    }
}

private struct AgentJumpToLatestButton: View {
    let action: () -> Void
    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: "arrow.down")
                    .font(.system(size: 11, weight: .semibold))
                Text("Jump to latest")
                    .font(.system(size: 11, weight: .semibold))
            }
            .foregroundStyle(.primary)
            .padding(.horizontal, 12)
            .padding(.vertical, 9)
            .background(.regularMaterial, in: Capsule())
            .overlay {
                Capsule()
                    .strokeBorder(Color.primary.opacity(isHovered ? 0.16 : 0.10), lineWidth: 0.5)
            }
            .shadow(color: Color.black.opacity(0.08), radius: 10, y: 3)
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

private struct AgentTranscriptScrollObserver: NSViewRepresentable {
    @Binding var isNearBottom: Bool

    func makeCoordinator() -> Coordinator {
        Coordinator(isNearBottom: $isNearBottom)
    }

    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        context.coordinator.attach(to: view)
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        context.coordinator.isNearBottom = $isNearBottom
        context.coordinator.refreshIfNeeded(from: nsView)
    }

    @MainActor
    final class Coordinator: NSObject {
        var isNearBottom: Binding<Bool>
        weak var observedScrollView: NSScrollView?
        private let threshold: CGFloat = 72

        init(isNearBottom: Binding<Bool>) {
            self.isNearBottom = isNearBottom
        }

        func attach(to view: NSView) {
            refreshIfNeeded(from: view)
        }

        func refreshIfNeeded(from view: NSView) {
            guard let scrollView = view.enclosingScrollView else { return }
            guard observedScrollView !== scrollView else {
                updateNearBottom(scrollView)
                return
            }

            NotificationCenter.default.removeObserver(self)
            observedScrollView = scrollView
            scrollView.contentView.postsBoundsChangedNotifications = true
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(handleBoundsDidChange(_:)),
                name: NSView.boundsDidChangeNotification,
                object: scrollView.contentView
            )
            updateNearBottom(scrollView)
        }

        @objc private func handleBoundsDidChange(_ notification: Notification) {
            guard let scrollView = observedScrollView else { return }
            updateNearBottom(scrollView)
        }

        private func updateNearBottom(_ scrollView: NSScrollView) {
            guard let documentView = scrollView.documentView else { return }
            let visibleMaxY = scrollView.contentView.bounds.maxY
            let distanceToBottom = max(documentView.frame.maxY - visibleMaxY, 0)
            let nextValue = distanceToBottom <= threshold
            if isNearBottom.wrappedValue != nextValue {
                isNearBottom.wrappedValue = nextValue
            }
        }
    }
}

private struct AgentUserMessageView: View {
    let item: EditorAgentTranscriptItem
    let selectionColor: NSColor

    var body: some View {
        HStack {
            Spacer(minLength: 0)
            VStack(alignment: .trailing, spacing: 4) {
                if let context = item.contextSummary, !context.isEmpty {
                    HStack(spacing: 4) {
                        Image(systemName: "doc.text")
                            .font(.system(size: 9, weight: .semibold))
                        Text(context)
                            .font(.system(size: 10, weight: .medium))
                    }
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 3)
                    .background {
                        Capsule().fill(Color.primary.opacity(0.05))
                    }
                    .overlay {
                        Capsule().strokeBorder(Color.primary.opacity(0.08), lineWidth: 0.5)
                    }
                }

                Text(item.text)
                    .font(.system(size: 13))
                    .foregroundStyle(.primary)
                    .multilineTextAlignment(.leading)
                    .textSelection(.enabled)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 8)
                    .background {
                        RoundedRectangle(cornerRadius: AgentLayout.userBubbleCornerRadius, style: .continuous)
                            .fill(Color(nsColor: selectionColor).opacity(0.22))
                    }
                    .overlay {
                        RoundedRectangle(cornerRadius: AgentLayout.userBubbleCornerRadius, style: .continuous)
                            .strokeBorder(Color.primary.opacity(0.08), lineWidth: 0.5)
                    }
            }
            .frame(maxWidth: 520, alignment: .trailing)
        }
    }
}

@MainActor
private struct AgentAssistantMessageView: View, Equatable {
    let item: EditorAgentTranscriptItem

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if let renderedMarkdown = item.renderedMarkdown,
               !renderedMarkdown.blocks.isEmpty {
                AgentRenderedMarkdownView(rendered: renderedMarkdown)
                    .frame(maxWidth: .infinity, alignment: .leading)
            } else {
                Text(item.text)
                    .font(.system(size: 13))
                    .foregroundStyle(.primary)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }

            if item.isStreaming && item.text.isEmpty {
                AgentThinkingIndicator()
            }
        }
        .padding(.trailing, 10)
        .onAppear {
            agentDebugLog("assistantView.appear id=\(item.id) revision=\(item.revision) chars=\(item.text.count) renderedBlocks=\(item.renderedMarkdown?.blocks.count ?? 0) streaming=\(item.isStreaming)")
        }
        .onChange(of: item.revision) { _, newValue in
            agentDebugLog("assistantView.revision id=\(item.id) revision=\(newValue) chars=\(item.text.count) renderedBlocks=\(item.renderedMarkdown?.blocks.count ?? 0) streaming=\(item.isStreaming)")
        }
    }
}

@MainActor
private struct AgentThinkingMessageView: View, Equatable {
    let item: EditorAgentTranscriptItem

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: "brain")
                    .font(.system(size: 10, weight: .semibold))
                Text("Thinking")
                    .font(.system(size: 10, weight: .semibold))
                if item.isStreaming {
                    AgentSpinner()
                }
            }
            .foregroundStyle(.secondary)

            if !item.text.isEmpty {
                Text(item.text)
                    .font(.system(size: 12))
                    .italic()
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
            } else if item.isStreaming {
                AgentThinkingIndicator()
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(Color.primary.opacity(0.025))
        }
        .overlay {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.06), lineWidth: 0.5)
        }
        .padding(.trailing, 10)
    }
}

@MainActor
private struct AgentRenderedMarkdownView: View, Equatable {
    let rendered: EditorRenderedMarkdown

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(segments.enumerated()), id: \.offset) { _, segment in
                switch segment {
                case .text(let runs):
                    EditorDocsAttributedTextView(runs: runs)
                        .frame(maxWidth: .infinity, alignment: .leading)
                case .code(let language, let runs):
                    AgentCodeBlockView(language: language, runs: runs)
                        .padding(.vertical, 4)
                }
            }
        }
    }

    private var segments: [AgentRenderedMarkdownSegment] {
        var result: [AgentRenderedMarkdownSegment] = []
        var pendingTextBlocks: [EditorMarkdownBlock] = []

        func flushTextBlocks() {
            let runs = mergedRuns(for: pendingTextBlocks)
            if !runs.isEmpty {
                result.append(.text(runs))
            }
            pendingTextBlocks.removeAll()
        }

        for block in rendered.blocks {
            if block.kind == .codeFence {
                flushTextBlocks()
                result.append(.code(language: block.language, runs: runs(for: block)))
            } else {
                pendingTextBlocks.append(block)
            }
        }

        flushTextBlocks()
        return result
    }

    private func mergedRuns(for blocks: [EditorMarkdownBlock]) -> [EditorDocsRun] {
        guard !blocks.isEmpty else { return [] }
        var merged: [EditorDocsRun] = []

        for (index, block) in blocks.enumerated() {
            if block.kind != .blankLine {
                merged.append(contentsOf: runs(for: block))
            }

            guard index < blocks.count - 1 else { continue }
            let nextBlock = blocks[index + 1]
            let separator = separatorText(after: block, before: nextBlock)
            if !separator.isEmpty {
                merged.append(separatorRun(separator, currentRuns: merged, nextBlock: nextBlock))
            }
        }

        return merged
    }

    private func separatorText(after block: EditorMarkdownBlock, before nextBlock: EditorMarkdownBlock) -> String {
        if block.kind == .blankLine {
            return "\n"
        }
        if nextBlock.kind == .blankLine {
            return "\n"
        }
        return "\n"
    }

    private func separatorRun(_ text: String, currentRuns: [EditorDocsRun], nextBlock: EditorMarkdownBlock) -> EditorDocsRun {
        let referenceRun = currentRuns.last ?? runs(for: nextBlock).first
        return EditorDocsRun(
            text: text,
            style: referenceRun?.style ?? EditorResolvedStyle(
                fg: nil,
                bg: nil,
                underlineColor: nil,
                addModifiers: 0,
                removeModifiers: 0,
                underlineStyle: 0
            ),
            kind: .body,
            linkDestination: nil
        )
    }

    private func runs(for block: EditorMarkdownBlock) -> [EditorDocsRun] {
        guard block.runCount > 0,
              block.runStart >= 0,
              block.runStart + block.runCount <= rendered.runs.count
        else {
            return []
        }
        return Array(rendered.runs[block.runStart..<(block.runStart + block.runCount)])
    }
}

private enum AgentRenderedMarkdownSegment {
    case text([EditorDocsRun])
    case code(language: String?, runs: [EditorDocsRun])
}

private struct AgentCodeBlockView: View {
    let language: String?
    let runs: [EditorDocsRun]
    @State private var didCopy = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                Text((language?.isEmpty == false ? language! : "code").uppercased())
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundStyle(.secondary)
                Spacer(minLength: 8)
                Button {
                    copyCode()
                } label: {
                    Label(didCopy ? "Copied" : "Copy", systemImage: didCopy ? "checkmark" : "doc.on.doc")
                        .labelStyle(.titleAndIcon)
                        .font(.system(size: 10, weight: .semibold))
                }
                .buttonStyle(.plain)
                .foregroundStyle(didCopy ? Color.green : .secondary)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(Color.primary.opacity(0.04))

            EditorDocsAttributedTextView(
                runs: runs,
                textContainerInset: NSSize(width: 10, height: 10)
            )
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.bottom, 2)
        }
        .background {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(Color.primary.opacity(0.035))
        }
        .overlay {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.08), lineWidth: 0.5)
        }
        .clipShape(.rect(cornerRadius: 12))
    }

    private func copyCode() {
        let text = runs.map(\.text).joined()
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        didCopy = true
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(1.2))
            didCopy = false
        }
    }
}

@MainActor
private struct AgentNoteRow: View, Equatable {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 11, weight: .medium))
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .center)
            .padding(.vertical, 2)
            .textSelection(.enabled)
    }
}

private struct AgentSummaryRow: View {
    let item: EditorAgentTranscriptItem
    let isExpanded: Bool
    let onToggleExpanded: () -> Void

    private var accentColor: Color {
        switch item.noteStyle {
        case .compactionSummary:
            return .blue
        case .branchSummary:
            return .purple
        case .plain:
            return .secondary
        }
    }

    private var badgeTitle: String {
        switch item.noteStyle {
        case .compactionSummary:
            return "COMPACTION"
        case .branchSummary:
            return "BRANCH"
        case .plain:
            return "NOTE"
        }
    }

    private var title: String {
        switch item.noteStyle {
        case .compactionSummary:
            return "Context compacted"
        case .branchSummary:
            return "Branch summary"
        case .plain:
            return "Note"
        }
    }

    private var subtitle: String {
        switch item.noteStyle {
        case .compactionSummary:
            if let tokensBefore = item.tokensBefore {
                return "Compacted from \(agentFormattedTokenCount(tokensBefore)) tokens"
            }
            return "Conversation history was compacted into a summary"
        case .branchSummary:
            return "Summary from the branch this conversation returned from"
        case .plain:
            return item.text
        }
    }

    private var iconName: String {
        switch item.noteStyle {
        case .compactionSummary:
            return "sparkles.rectangle.stack"
        case .branchSummary:
            return "arrow.triangle.branch"
        case .plain:
            return "note.text"
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Button {
                guard !item.text.isEmpty else { return }
                withAnimation(.easeInOut(duration: 0.15)) {
                    onToggleExpanded()
                }
            } label: {
                HStack(alignment: .center, spacing: 10) {
                    Image(systemName: item.text.isEmpty ? "circle.fill" : (isExpanded ? "chevron.down" : "chevron.right"))
                        .font(.system(size: item.text.isEmpty ? 6 : 9, weight: .semibold))
                        .foregroundStyle(.tertiary)
                        .frame(width: 10)

                    Image(systemName: iconName)
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(accentColor)
                        .frame(width: 18, height: 18)
                        .background(accentColor.opacity(0.12), in: RoundedRectangle(cornerRadius: 9, style: .continuous))

                    VStack(alignment: .leading, spacing: 4) {
                        HStack(spacing: 6) {
                            Text(badgeTitle)
                                .font(.system(size: 9, weight: .semibold, design: .monospaced))
                                .foregroundStyle(accentColor)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(accentColor.opacity(0.10), in: Capsule())

                            if item.noteStyle == .compactionSummary, let tokensBefore = item.tokensBefore {
                                Text("\(agentFormattedTokenCount(tokensBefore)) tokens")
                                    .font(.system(size: 10, weight: .medium))
                                    .foregroundStyle(.secondary)
                            }
                        }

                        Text(title)
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(.primary)

                        Text(subtitle)
                            .font(.system(size: 11))
                            .foregroundStyle(.secondary)
                            .lineLimit(isExpanded ? nil : 2)
                    }

                    Spacer(minLength: 8)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(item.text.isEmpty)

            if isExpanded {
                Group {
                    if let renderedMarkdown = item.renderedMarkdown,
                       !renderedMarkdown.blocks.isEmpty {
                        AgentRenderedMarkdownView(rendered: renderedMarkdown)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    } else {
                        Text(item.text)
                            .font(.system(size: 12))
                            .foregroundStyle(.primary)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
                .padding(.horizontal, 12)
                .padding(.bottom, 12)
            }
        }
        .background {
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .fill(accentColor.opacity(0.06))
        }
        .overlay {
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .strokeBorder(accentColor.opacity(0.18), lineWidth: 0.75)
        }
    }
}

private func agentFormattedTokenCount(_ count: Int) -> String {
    NumberFormatter.localizedString(from: NSNumber(value: count), number: .decimal)
}

private struct AgentToolRow: View {
    let item: EditorAgentTranscriptItem
    let isExpanded: Bool
    let onToggleExpanded: () -> Void
    @State private var didCopy = false

    private var display: AgentToolDisplayModel {
        buildAgentToolDisplay(item)
    }

    private var statusColor: Color {
        if item.isStreaming { return .secondary }
        switch item.status {
        case "failed":
            return .red
        case "done":
            return .green
        default:
            return .secondary
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Button {
                guard !item.text.isEmpty else { return }
                withAnimation(.easeInOut(duration: 0.15)) {
                    onToggleExpanded()
                }
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: item.text.isEmpty ? "circle.fill" : (isExpanded ? "chevron.down" : "chevron.right"))
                        .font(.system(size: item.text.isEmpty ? 6 : 9, weight: .semibold))
                        .foregroundStyle(.tertiary)
                        .frame(width: 10)

                    Image(systemName: display.iconName)
                        .font(.system(size: 10, weight: .semibold))
                        .foregroundStyle(display.iconColor)
                        .frame(width: 16, height: 16)
                        .background(display.iconColor.opacity(0.12), in: RoundedRectangle(cornerRadius: 8, style: .continuous))

                    Text(display.badge)
                        .font(.system(size: 9, weight: .semibold, design: .monospaced))
                        .foregroundStyle(.tertiary)

                    Text(display.title)
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(.primary)
                        .lineLimit(1)

                    if let summary = display.summary {
                        Text(summary)
                            .font(.system(size: 12))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }

                    Spacer(minLength: 6)

                    if item.isStreaming {
                        AgentSpinner()
                    } else {
                        Circle()
                            .fill(statusColor)
                            .frame(width: 7, height: 7)
                    }
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(item.text.isEmpty)

            if isExpanded {
                VStack(alignment: .leading, spacing: 8) {
                    if let inputBody = display.inputBody, !inputBody.isEmpty {
                        AgentToolBodySection(
                            title: display.inputTitle ?? "Input",
                            text: inputBody,
                            didCopy: $didCopy
                        )
                    }

                    if let outputBody = display.outputBody, !outputBody.isEmpty {
                        AgentToolBodySection(
                            title: display.outputTitle ?? "Output",
                            text: outputBody,
                            didCopy: $didCopy
                        )
                    }
                }
                .padding(.horizontal, 10)
                .padding(.bottom, 10)
            }
        }
        .background {
            RoundedRectangle(cornerRadius: AgentLayout.toolRowCornerRadius, style: .continuous)
                .fill(Color.primary.opacity(0.03))
        }
        .overlay {
            RoundedRectangle(cornerRadius: AgentLayout.toolRowCornerRadius, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.06), lineWidth: 0.5)
        }
    }
}

private struct AgentToolDisplayModel {
    let badge: String
    let title: String
    let summary: String?
    let inputTitle: String?
    let inputBody: String?
    let outputTitle: String?
    let outputBody: String?
    let iconName: String
    let iconColor: Color
}

private struct AgentToolBodySection: View {
    let title: String
    let text: String
    @Binding var didCopy: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                Text(title)
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(.tertiary)
                Spacer(minLength: 8)
                Button {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(text, forType: .string)
                    didCopy = true
                    Task { @MainActor in
                        try? await Task.sleep(for: .seconds(1.2))
                        didCopy = false
                    }
                } label: {
                    Label(didCopy ? "Copied" : "Copy", systemImage: didCopy ? "checkmark" : "doc.on.doc")
                        .labelStyle(.titleAndIcon)
                        .font(.system(size: 10, weight: .semibold))
                }
                .buttonStyle(.plain)
                .foregroundStyle(didCopy ? Color.green : .secondary)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)

            Text(text)
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 10)
                .padding(.bottom, 10)
        }
        .background {
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Color.primary.opacity(0.025))
        }
        .overlay {
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.06), lineWidth: 0.5)
        }
    }
}

private func buildAgentToolDisplay(_ item: EditorAgentTranscriptItem) -> AgentToolDisplayModel {
    let normalizedToolName = item.contextSummary?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() ?? ""
    let toolName = normalizedToolName.isEmpty ? "tool" : normalizedToolName
    let normalizedTitle = item.title?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    let fallbackTitle = normalizedTitle.isEmpty ? toolName.capitalized : normalizedTitle
    let normalizedOutput = item.text.trimmingCharacters(in: .whitespacesAndNewlines)
    let rawOutput = normalizedOutput.isEmpty ? nil : normalizedOutput
    let inputObject = parseAgentToolJSONObject(item.toolInputJSON)

    let icon: (String, Color)
    switch toolName {
    case "read":
        icon = ("eye", .blue)
    case "edit":
        icon = ("square.and.pencil", .purple)
    case "write":
        icon = ("square.and.arrow.down", .green)
    case "bash":
        icon = ("terminal", .orange)
    case "grep", "search":
        icon = ("magnifyingglass", .teal)
    default:
        icon = ("wrench.and.screwdriver", .secondary)
    }

    switch toolName {
    case "read":
        let path = shortenAgentDisplayPath(agentJSONString(inputObject?["path"])) ?? fallbackTitle
        let offset = agentJSONInt(inputObject?["offset"])
        let limit = agentJSONInt(inputObject?["limit"])
        let rangeSuffix: String
        if offset != nil || limit != nil {
            let start = offset ?? 1
            if let limit {
                rangeSuffix = ":\(start)-\(start + max(limit - 1, 0))"
            } else {
                rangeSuffix = ":\(start)"
            }
        } else {
            rangeSuffix = ""
        }
        return AgentToolDisplayModel(
            badge: toolName.uppercased(),
            title: path + rangeSuffix,
            summary: rawOutput.flatMap(agentToolSummaryLine),
            inputTitle: nil,
            inputBody: nil,
            outputTitle: item.status == "failed" ? "Error" : nil,
            outputBody: item.status == "failed" ? rawOutput : nil,
            iconName: icon.0,
            iconColor: icon.1
        )
    case "write":
        let path = shortenAgentDisplayPath(agentJSONString(inputObject?["path"])) ?? fallbackTitle
        let content = agentJSONString(inputObject?["content"])?
            .trimmingCharacters(in: .newlines)
        let hideSuccessOutput = rawOutput?.hasPrefix("Successfully wrote ") == true && rawOutput?.contains(" bytes to ") == true
        return AgentToolDisplayModel(
            badge: toolName.uppercased(),
            title: path,
            summary: (hideSuccessOutput ? rawOutput : rawOutput).flatMap(agentToolSummaryLine),
            inputTitle: content?.isEmpty == false ? "Content" : nil,
            inputBody: content?.isEmpty == false ? content : nil,
            outputTitle: (hideSuccessOutput || rawOutput == nil) ? nil : "Result",
            outputBody: (hideSuccessOutput || rawOutput == nil) ? nil : rawOutput,
            iconName: icon.0,
            iconColor: icon.1
        )
    case "bash":
        let command = agentJSONString(inputObject?["command"]) ?? fallbackTitle
        let timeoutSuffix: String
        if let timeoutInt = agentJSONInt(inputObject?["timeout"]) {
            timeoutSuffix = " (timeout \(timeoutInt)s)"
        } else {
            timeoutSuffix = ""
        }
        return AgentToolDisplayModel(
            badge: toolName.uppercased(),
            title: "$ \(command.split(separator: "\n").first.map(String.init) ?? command)\(timeoutSuffix)",
            summary: rawOutput.flatMap(agentToolSummaryLine),
            inputTitle: nil,
            inputBody: nil,
            outputTitle: rawOutput == nil ? nil : "Output",
            outputBody: rawOutput,
            iconName: icon.0,
            iconColor: icon.1
        )
    case "edit":
        let path = shortenAgentDisplayPath(agentJSONString(inputObject?["path"])) ?? fallbackTitle
        let edits = inputObject?["edits"] as? [[String: Any]]
        let editsCount = edits?.count ?? 0
        let suffix = editsCount > 1 ? " (\(editsCount) edits)" : editsCount == 1 ? " (1 edit)" : ""
        let inputBody: String?
        if let firstEdit = edits?.first,
           let oldText = firstEdit["oldText"] as? String,
           let newText = firstEdit["newText"] as? String,
           editsCount == 1 {
            inputBody = "Replace:\n\(oldText.trimmingCharacters(in: .newlines))\n\nWith:\n\(newText.trimmingCharacters(in: .newlines))"
        } else {
            inputBody = nil
        }
        return AgentToolDisplayModel(
            badge: toolName.uppercased(),
            title: path + suffix,
            summary: rawOutput.flatMap(agentToolSummaryLine),
            inputTitle: inputBody == nil ? nil : "Edit",
            inputBody: inputBody,
            outputTitle: rawOutput == nil ? nil : "Result",
            outputBody: rawOutput,
            iconName: icon.0,
            iconColor: icon.1
        )
    default:
        return AgentToolDisplayModel(
            badge: toolName.uppercased(),
            title: fallbackTitle,
            summary: rawOutput.flatMap(agentToolSummaryLine),
            inputTitle: item.toolInputJSON?.isEmpty == false ? "Input" : nil,
            inputBody: item.toolInputJSON,
            outputTitle: rawOutput == nil ? nil : "Output",
            outputBody: rawOutput,
            iconName: icon.0,
            iconColor: icon.1
        )
    }
}

private func parseAgentToolJSONObject(_ text: String?) -> [String: Any]? {
    guard let text, !text.isEmpty,
          let data = text.data(using: .utf8),
          let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    else {
        return nil
    }
    return object
}

private func agentJSONString(_ value: Any?) -> String? {
    switch value {
    case let string as String:
        return string
    case let number as NSNumber:
        return number.stringValue
    default:
        return nil
    }
}

private func agentJSONInt(_ value: Any?) -> Int? {
    switch value {
    case let number as NSNumber:
        return number.intValue
    case let string as String:
        return Int(string)
    default:
        return nil
    }
}

private func shortenAgentDisplayPath(_ path: String?) -> String? {
    guard let path, !path.isEmpty else { return nil }
    let expandedHome = NSHomeDirectory()
    if path.hasPrefix(expandedHome) {
        return "~" + path.dropFirst(expandedHome.count)
    }
    return path
}

private func agentToolSummaryLine(_ text: String) -> String? {
    let lines = text
        .components(separatedBy: .newlines)
        .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
        .filter { !$0.isEmpty }

    guard !lines.isEmpty else { return nil }
    if let changedLine = lines.first(where: { $0.localizedCaseInsensitiveContains("changed lines:") }) {
        return changedLine
    }
    if let meaningful = lines.first(where: { !$0.hasPrefix("---") && !$0.hasPrefix("+++") }) {
        return meaningful.count > 96 ? String(meaningful.prefix(96)) + "…" : meaningful
    }
    let first = lines[0]
    return first.count > 96 ? String(first.prefix(96)) + "…" : first
}

private struct AgentSpinner: View {
    @State private var isSpinning = false

    var body: some View {
        Image(systemName: "arrow.triangle.2.circlepath")
            .font(.system(size: 9, weight: .semibold))
            .foregroundStyle(.secondary)
            .rotationEffect(.degrees(isSpinning ? 360 : 0))
            .animation(.linear(duration: 1.1).repeatForever(autoreverses: false), value: isSpinning)
            .onAppear { isSpinning = true }
    }
}

private struct AgentThinkingIndicator: View {
    var body: some View {
        HStack(spacing: 4) {
            ForEach(0..<3, id: \.self) { index in
                AgentThinkingDot(delay: Double(index) * 0.18)
            }
        }
    }
}

private struct AgentThinkingDot: View {
    let delay: Double
    @State private var isActive = false

    var body: some View {
        Circle()
            .fill(Color.secondary)
            .frame(width: 5, height: 5)
            .opacity(isActive ? 1 : 0.3)
            .animation(
                .easeInOut(duration: 0.55).repeatForever(autoreverses: true).delay(delay),
                value: isActive
            )
            .onAppear { isActive = true }
    }
}

// MARK: - Composer

private struct AgentFrameObserver: View {
    let onChange: (CGRect) -> Void

    var body: some View {
        GeometryReader { geometry in
            let frame = geometry.frame(in: .global)
            Color.clear
                .onAppear {
                    onChange(frame)
                }
                .onChange(of: frame) { _, newFrame in
                    onChange(newFrame)
                }
        }
    }
}

private struct AgentCompactionStatusView: View {
    let status: EditorAgentCompactionStatus
    let onCancel: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            AgentSpinner()

            VStack(alignment: .leading, spacing: 2) {
                Text(status.title)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(.primary)
                Text("Esc or Stop to cancel")
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
            }

            Spacer(minLength: 8)

            Button(action: onCancel) {
                Text("Cancel")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(.primary)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .background(Color.primary.opacity(0.06), in: Capsule())
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background {
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .fill(Color.primary.opacity(0.045))
        }
        .overlay {
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.08), lineWidth: 0.5)
        }
    }
}

private struct AgentComposerBar: View {
    @Binding var inputText: String
    @Binding var isFocused: Bool
    @State private var textViewHeight: CGFloat = AgentLayout.composerMinHeight

    let placeholder: String
    let isRunning: Bool
    let isCompacting: Bool
    let isReady: Bool
    let canSend: Bool
    let isFollowingAgent: Bool
    let followStatusText: String?
    let onSend: () -> Void
    let onFollowAndSend: () -> Void
    let onCancel: () -> Void
    let onToggleFollow: () -> Void
    let onInsertCommand: (EditorAgentCommand) -> Void
    let onMoveUp: () -> Void
    let onMoveDown: () -> Void
    let onDismissOverlay: () -> Void
    let onCycleThinkingLevel: () -> Void
    let onActivateSurface: () -> Void
    let commands: [EditorAgentCommand]
    let commandSelectionIndex: Int
    let showsModelPicker: Bool
    let models: [EditorAgentModel]
    let modelSelectionIndex: Int
    let onPickModel: (EditorAgentModel) -> Void
    let showsResumePicker: Bool
    let recentSessionSuggestions: [EditorAgentRecentSession]
    let resumeSelectionIndex: Int
    let onPickResumeSession: (EditorAgentRecentSession) -> Void

    let footerInfo: EditorAgentFooterInfo?
    let sessionSubtitle: String
    let contextUsageText: String?
    let onNewSession: () -> Void
    let onShowRecentSessions: () -> Void
    let recentSessions: [EditorAgentRecentSession]
    let onResumeSession: (EditorAgentRecentSession) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            textField

            if !detectedImages.isEmpty {
                AgentComposerImageStrip(images: detectedImages)
                    .padding(.top, 8)
            }

            controlsRow
                .padding(.top, 4)
        }
        .padding(AgentLayout.composerContainerPadding)
        .background {
            composerBackground
        }
        .overlay {
            composerBorder
        }
    }

    private var textField: some View {
        ZStack(alignment: .topLeading) {
            if inputText.isEmpty {
                Text(placeholder)
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(.tertiary)
                    .padding(.top, AgentLayout.composerTextTopInset)
                    .padding(.leading, AgentLayout.composerTextLeadingInset)
                    .allowsHitTesting(false)
            }

            AgentComposerNativeTextView(
                text: $inputText,
                measuredHeight: $textViewHeight,
                isFocused: $isFocused,
                isEditable: isReady,
                showsOverlayNavigation: showsOverlayKeyboardNavigation,
                shouldCancelOnEscape: isCompacting,
                onSubmit: onSend,
                onFollowAndSubmit: onFollowAndSend,
                onCancelOperation: onCancel,
                onMoveUp: onMoveUp,
                onMoveDown: onMoveDown,
                onDismissOverlay: onDismissOverlay,
                onCycleThinkingLevel: onCycleThinkingLevel,
                onActivateSurface: onActivateSurface
            )
            .frame(height: max(textViewHeight, AgentLayout.composerMinHeight))
        }
        .frame(minHeight: max(textViewHeight, AgentLayout.composerMinHeight), alignment: .topLeading)
    }

    private var detectedImages: [AgentComposerDetectedImage] {
        agentComposerDetectedImages(in: inputText)
    }

    private var showsOverlayKeyboardNavigation: Bool {
        showsModelPicker || showsResumePicker || !commands.isEmpty
    }

    @ViewBuilder
    private var controlsRow: some View {
        HStack(alignment: .bottom, spacing: AgentLayout.composerRowSpacing) {
            AgentPromptInfoBar(
                footerInfo: footerInfo,
                fallbackSubtitle: sessionSubtitle,
                fallbackContextUsageText: contextUsageText,
                isFollowingAgent: isFollowingAgent,
                followStatusText: followStatusText,
                onToggleFollow: onToggleFollow,
                onNewSession: onNewSession,
                recentSessions: recentSessions,
                onResumeSession: onResumeSession,
                onRefreshRecent: onShowRecentSessions
            )
            .frame(maxWidth: .infinity, alignment: .leading)

            sendButton
        }
    }

    @ViewBuilder
    private var sendButton: some View {
        if isRunning {
            Button(action: onCancel) {
                ZStack {
                    Circle().fill(Color.primary.opacity(0.9))
                    Image(systemName: "stop.fill")
                        .font(.system(size: 11, weight: .bold))
                        .foregroundStyle(Color(nsColor: .windowBackgroundColor))
                }
                .frame(width: AgentLayout.composerSendSize, height: AgentLayout.composerSendSize)
            }
            .buttonStyle(.plain)
            .transition(.opacity)
        } else {
            Button(action: {
                if NSEvent.modifierFlags.contains(.command) {
                    onFollowAndSend()
                } else {
                    onSend()
                }
            }) {
                ZStack {
                    Circle()
                        .fill(canSend ? Color.primary.opacity(0.92) : Color.primary.opacity(0.18))
                    Image(systemName: "arrow.up")
                        .font(.system(size: 13, weight: .bold))
                        .foregroundStyle(canSend ? Color(nsColor: .windowBackgroundColor) : .secondary)
                }
                .frame(width: AgentLayout.composerSendSize, height: AgentLayout.composerSendSize)
            }
            .buttonStyle(.plain)
            .disabled(!canSend)
            .animation(.spring(response: 0.25, dampingFraction: 0.8), value: canSend)
        }
    }

    @ViewBuilder
    private var composerBackground: some View {
        let shape = RoundedRectangle(cornerRadius: AgentLayout.composerCornerRadius, style: .continuous)
        if #available(macOS 26.0, *) {
            GlassEffectContainer {
                shape
                    .fill(Color.white.opacity(0.001))
                    .glassEffect(.regular, in: shape)
                shape.fill(Color.white.opacity(0.03))
            }
        } else {
            shape.fill(.ultraThinMaterial)
        }
    }

    @ViewBuilder
    private var composerBorder: some View {
        RoundedRectangle(cornerRadius: AgentLayout.composerCornerRadius, style: .continuous)
            .strokeBorder(
                Color.primary.opacity(isFocused ? 0.22 : 0.10),
                lineWidth: 0.6
            )
            .animation(.easeOut(duration: 0.15), value: isFocused)
            .allowsHitTesting(false)
    }
}

private struct AgentComposerDetectedImage: Identifiable, Hashable {
    let path: String

    var id: String { path }

    var fileName: String {
        URL(fileURLWithPath: path).lastPathComponent
    }
}

private func agentComposerDetectedImages(in text: String) -> [AgentComposerDetectedImage] {
    let pattern = #"(?:file:///|/)[^\s\"'<>]+\.(?:png|jpe?g|gif|webp|heic|heif|tiff?|bmp)"#
    guard let regex = try? NSRegularExpression(pattern: pattern, options: [.caseInsensitive]) else {
        return []
    }

    let nsText = text as NSString
    var seenPaths = Set<String>()
    var results: [AgentComposerDetectedImage] = []

    for match in regex.matches(in: text, options: [], range: NSRange(location: 0, length: nsText.length)) {
        let rawMatch = nsText.substring(with: match.range)
        guard let resolvedPath = agentResolveComposerImagePath(rawMatch),
              !seenPaths.contains(resolvedPath)
        else {
            continue
        }
        seenPaths.insert(resolvedPath)
        results.append(AgentComposerDetectedImage(path: resolvedPath))
    }

    return results
}

private func agentResolveComposerImagePath(_ rawPath: String) -> String? {
    let resolvedPath: String
    if rawPath.hasPrefix("file://"), let url = URL(string: rawPath), url.isFileURL {
        resolvedPath = url.path
    } else {
        resolvedPath = rawPath
    }

    guard FileManager.default.fileExists(atPath: resolvedPath),
          NSImage(contentsOfFile: resolvedPath) != nil
    else {
        return nil
    }

    return resolvedPath
}

private struct AgentComposerImageStrip: View {
    let images: [AgentComposerDetectedImage]
    @State private var selectedImage: AgentComposerDetectedImage?

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: AgentLayout.composerAttachmentSpacing) {
                ForEach(images) { image in
                    AgentComposerImageChip(image: image) {
                        selectedImage = image
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .sheet(item: $selectedImage) { image in
            AgentComposerImageDetailView(image: image)
        }
    }
}

private struct AgentComposerImageChip: View {
    let image: AgentComposerDetectedImage
    let onOpen: () -> Void
    @State private var isHovered = false

    private var nsImage: NSImage? {
        NSImage(contentsOfFile: image.path)
    }

    var body: some View {
        Button(action: onOpen) {
            HStack(spacing: 8) {
                Group {
                    if let nsImage {
                        Image(nsImage: nsImage)
                            .resizable()
                            .scaledToFill()
                    } else {
                        Image(systemName: "photo")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundStyle(.secondary)
                    }
                }
                .frame(
                    width: AgentLayout.composerAttachmentPreviewSize,
                    height: AgentLayout.composerAttachmentPreviewSize
                )
                .clipShape(.rect(cornerRadius: 8))
                .overlay {
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .strokeBorder(Color.primary.opacity(0.08), lineWidth: 0.5)
                }

                VStack(alignment: .leading, spacing: 2) {
                    Text(image.fileName)
                        .font(.system(size: 11, weight: .medium))
                        .foregroundStyle(.primary)
                        .lineLimit(1)
                    Text(image.path)
                        .font(.system(size: 10))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }

                Image(systemName: "arrow.up.left.and.arrow.down.right")
                    .font(.system(size: 10, weight: .medium))
                    .foregroundStyle(.tertiary)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 7)
            .background {
                RoundedRectangle(cornerRadius: AgentLayout.composerAttachmentCornerRadius, style: .continuous)
                    .fill(isHovered ? Color.primary.opacity(0.06) : Color.primary.opacity(0.03))
            }
            .overlay {
                RoundedRectangle(cornerRadius: AgentLayout.composerAttachmentCornerRadius, style: .continuous)
                    .strokeBorder(Color.primary.opacity(0.08), lineWidth: 0.5)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

private struct AgentComposerImageDetailView: View {
    let image: AgentComposerDetectedImage
    @Environment(\.dismiss) private var dismiss

    private var nsImage: NSImage? {
        NSImage(contentsOfFile: image.path)
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .center, spacing: 12) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(image.fileName)
                        .font(.headline)
                    Text(image.path)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                        .textSelection(.enabled)
                }

                Spacer(minLength: 12)

                Button("Done") {
                    dismiss()
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)

            Divider()

            ScrollView {
                Group {
                    if let nsImage {
                        Image(nsImage: nsImage)
                            .resizable()
                            .scaledToFit()
                            .frame(maxWidth: .infinity)
                            .padding(16)
                    } else {
                        VStack(spacing: 12) {
                            Image(systemName: "photo")
                                .font(.system(size: 40))
                                .foregroundStyle(.secondary)
                            Text("Unable to load image")
                                .foregroundStyle(.secondary)
                        }
                        .frame(maxWidth: .infinity, minHeight: 320)
                    }
                }
            }
        }
        .frame(width: 760, height: 560)
    }
}

@MainActor
private final class AgentComposerTextView: NSTextView {
    var imagePastePathProvider: (() -> String?)?
    var onActivateSurface: (() -> Void)?

    override func becomeFirstResponder() -> Bool {
        let accepted = super.becomeFirstResponder()
        if accepted {
            onActivateSurface?()
        }
        return accepted
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if modifiers == [.command], event.charactersIgnoringModifiers == "v",
           let insertedPath = imagePastePathProvider?() {
            insertText(insertedPath, replacementRange: selectedRange())
            return true
        }
        return super.performKeyEquivalent(with: event)
    }

    override func paste(_ sender: Any?) {
        if let insertedPath = imagePastePathProvider?() {
            insertText(insertedPath, replacementRange: selectedRange())
        } else {
            super.paste(sender)
        }
    }
}

private struct AgentComposerNativeTextView: NSViewRepresentable {
    @Binding var text: String
    @Binding var measuredHeight: CGFloat
    @Binding var isFocused: Bool

    let isEditable: Bool
    let showsOverlayNavigation: Bool
    let shouldCancelOnEscape: Bool
    let onSubmit: () -> Void
    let onFollowAndSubmit: () -> Void
    let onCancelOperation: () -> Void
    let onMoveUp: () -> Void
    let onMoveDown: () -> Void
    let onDismissOverlay: () -> Void
    let onCycleThinkingLevel: () -> Void
    let onActivateSurface: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(
            text: $text,
            measuredHeight: $measuredHeight,
            isFocused: $isFocused,
            onSubmit: onSubmit,
            onFollowAndSubmit: onFollowAndSubmit,
            onCancelOperation: onCancelOperation,
            onMoveUp: onMoveUp,
            onMoveDown: onMoveDown,
            onDismissOverlay: onDismissOverlay,
            onCycleThinkingLevel: onCycleThinkingLevel,
            onActivateSurface: onActivateSurface,
            showsOverlayNavigation: showsOverlayNavigation,
            shouldCancelOnEscape: shouldCancelOnEscape
        )
    }

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView(frame: .zero)
        let textView = AgentComposerTextView(frame: .zero)
        scrollView.documentView = textView

        scrollView.drawsBackground = false
        scrollView.borderType = .noBorder
        scrollView.hasHorizontalScroller = false
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.scrollerStyle = .overlay

        textView.delegate = context.coordinator
        textView.font = .systemFont(ofSize: 14)
        textView.isRichText = false
        textView.importsGraphics = false
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.isAutomaticSpellingCorrectionEnabled = false
        textView.isAutomaticTextCompletionEnabled = false
        textView.isAutomaticLinkDetectionEnabled = false
        textView.isAutomaticDataDetectionEnabled = false
        textView.isGrammarCheckingEnabled = false
        textView.isContinuousSpellCheckingEnabled = false
        textView.drawsBackground = false
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.minSize = .zero
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.textContainerInset = NSSize(width: AgentLayout.composerTextLeadingInset, height: AgentLayout.composerTextTopInset)
        textView.textContainer?.lineFragmentPadding = 0
        textView.textContainer?.widthTracksTextView = true
        textView.textContainer?.heightTracksTextView = false
        textView.string = text
        textView.isEditable = isEditable
        textView.isSelectable = true
        textView.onActivateSurface = { [weak coordinator = context.coordinator] in
            coordinator?.activateSurface()
        }
        textView.imagePastePathProvider = { [weak coordinator = context.coordinator] in
            coordinator?.composerImagePastePath()
        }

        context.coordinator.scrollView = scrollView
        context.coordinator.textView = textView
        context.coordinator.showsOverlayNavigation = showsOverlayNavigation
        context.coordinator.shouldCancelOnEscape = shouldCancelOnEscape
        context.coordinator.updateMeasuredHeightIfNeeded()

        DispatchQueue.main.async {
            context.coordinator.requestFocusIfNeeded()
        }

        return scrollView
    }

    func updateNSView(_ nsView: NSScrollView, context: Context) {
        guard let textView = nsView.documentView as? NSTextView else { return }

        context.coordinator.scrollView = nsView
        context.coordinator.textView = textView
        context.coordinator.showsOverlayNavigation = showsOverlayNavigation
        context.coordinator.shouldCancelOnEscape = shouldCancelOnEscape

        if textView.string != text, !textView.hasMarkedText() {
            textView.string = text
        }

        if textView.isEditable != isEditable {
            textView.isEditable = isEditable
        }
        if let composerTextView = textView as? AgentComposerTextView {
            composerTextView.onActivateSurface = { [weak coordinator = context.coordinator] in
                coordinator?.activateSurface()
            }
            composerTextView.imagePastePathProvider = { [weak coordinator = context.coordinator] in
                coordinator?.composerImagePastePath()
            }
        }

        context.coordinator.updateMeasuredHeightIfNeeded()
        context.coordinator.requestFocusIfNeeded()
    }

    @MainActor
    final class Coordinator: NSObject, NSTextViewDelegate {
        @Binding var text: String
        @Binding var measuredHeight: CGFloat
        @Binding var isFocused: Bool

        let onSubmit: () -> Void
        let onFollowAndSubmit: () -> Void
        let onCancelOperation: () -> Void
        let onMoveUp: () -> Void
        let onMoveDown: () -> Void
        let onDismissOverlay: () -> Void
        let onCycleThinkingLevel: () -> Void
        let onActivateSurface: () -> Void

        weak var textView: NSTextView?
        weak var scrollView: NSScrollView?
        var showsOverlayNavigation: Bool
        var shouldCancelOnEscape: Bool
        var lastMeasuredText = ""
        var lastMeasuredWidth: CGFloat = 0

        init(
            text: Binding<String>,
            measuredHeight: Binding<CGFloat>,
            isFocused: Binding<Bool>,
            onSubmit: @escaping () -> Void,
            onFollowAndSubmit: @escaping () -> Void,
            onCancelOperation: @escaping () -> Void,
            onMoveUp: @escaping () -> Void,
            onMoveDown: @escaping () -> Void,
            onDismissOverlay: @escaping () -> Void,
            onCycleThinkingLevel: @escaping () -> Void,
            onActivateSurface: @escaping () -> Void,
            showsOverlayNavigation: Bool,
            shouldCancelOnEscape: Bool
        ) {
            _text = text
            _measuredHeight = measuredHeight
            _isFocused = isFocused
            self.onSubmit = onSubmit
            self.onFollowAndSubmit = onFollowAndSubmit
            self.onCancelOperation = onCancelOperation
            self.onMoveUp = onMoveUp
            self.onMoveDown = onMoveDown
            self.onDismissOverlay = onDismissOverlay
            self.onCycleThinkingLevel = onCycleThinkingLevel
            self.onActivateSurface = onActivateSurface
            self.showsOverlayNavigation = showsOverlayNavigation
            self.shouldCancelOnEscape = shouldCancelOnEscape
            super.init()
        }

        func textDidChange(_ notification: Notification) {
            guard let textView = notification.object as? NSTextView else { return }
            guard !textView.hasMarkedText() else { return }
            text = textView.string
            updateMeasuredHeightIfNeeded()
        }

        func textDidBeginEditing(_ notification: Notification) {
            activateSurface()
            guard !isFocused else { return }
            isFocused = true
        }

        func textDidEndEditing(_ notification: Notification) {
            guard isFocused else { return }
            isFocused = false
        }

        func textView(_ textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if showsOverlayNavigation {
                if commandSelector == #selector(NSTextView.moveUp(_:)) {
                    onMoveUp()
                    return true
                }
                if commandSelector == #selector(NSTextView.moveDown(_:)) {
                    onMoveDown()
                    return true
                }
            }

            if commandSelector == #selector(NSTextView.cancelOperation(_:)) {
                if shouldCancelOnEscape {
                    onCancelOperation()
                } else {
                    onDismissOverlay()
                }
                return true
            }

            if commandSelector == #selector(NSResponder.insertBacktab(_:)) ||
                (commandSelector == #selector(NSTextView.insertTab(_:)) && NSEvent.modifierFlags.contains(.shift)) {
                onCycleThinkingLevel()
                return true
            }

            if commandSelector == #selector(NSTextView.insertNewline(_:)) {
                if NSEvent.modifierFlags.contains(.shift) {
                    textView.insertNewlineIgnoringFieldEditor(nil)
                } else if NSEvent.modifierFlags.contains(.command) {
                    onFollowAndSubmit()
                } else {
                    onSubmit()
                }
                return true
            }

            return false
        }

        func activateSurface() {
            onActivateSurface()
        }

        func composerImagePastePath() -> String? {
            let pasteboard = NSPasteboard.general

            if let data = pasteboard.data(forType: .png) {
                return writeTemporaryClipboardImage(data: data, fileExtension: "png")
            }

            if let data = pasteboard.data(forType: .tiff),
               let image = NSImage(data: data),
               let tiffData = image.tiffRepresentation,
               let bitmap = NSBitmapImageRep(data: tiffData),
               let pngData = bitmap.representation(using: .png, properties: [:]) {
                return writeTemporaryClipboardImage(data: pngData, fileExtension: "png")
            }

            if let urls = pasteboard.readObjects(forClasses: [NSURL.self], options: nil) as? [URL],
               let imageURL = urls.first(where: { Self.isImageFileURL($0) }) {
                return imageURL.path
            }

            return nil
        }

        private func writeTemporaryClipboardImage(data: Data, fileExtension: String) -> String? {
            let temporaryURL = FileManager.default.temporaryDirectory
                .appendingPathComponent("pi-clipboard-\(UUID().uuidString)")
                .appendingPathExtension(fileExtension)
            do {
                try data.write(to: temporaryURL, options: [.atomic])
                return temporaryURL.path
            } catch {
                return nil
            }
        }

        private static func isImageFileURL(_ url: URL) -> Bool {
            let imageExtensions: Set<String> = ["png", "jpg", "jpeg", "gif", "webp", "heic", "heif", "tiff", "tif", "bmp"]
            return imageExtensions.contains(url.pathExtension.lowercased())
        }

        func requestFocusIfNeeded() {
            guard isFocused,
                  let textView,
                  let window = textView.window,
                  window.firstResponder !== textView else {
                return
            }
            window.makeFirstResponder(textView)
        }

        func updateMeasuredHeightIfNeeded() {
            guard let textView,
                  let scrollView,
                  let textContainer = textView.textContainer,
                  let layoutManager = textView.layoutManager else {
                return
            }

            let width = scrollView.contentSize.width
            let currentText = textView.string
            guard abs(width - lastMeasuredWidth) > 0.5 || currentText != lastMeasuredText else { return }

            lastMeasuredWidth = width
            lastMeasuredText = currentText

            textContainer.containerSize = NSSize(width: max(width, 1), height: .greatestFiniteMagnitude)
            layoutManager.ensureLayout(for: textContainer)

            let usedRect = layoutManager.usedRect(for: textContainer)
            let contentHeight = ceil(usedRect.height + (textView.textContainerInset.height * 2))
            let lineHeight = textView.layoutManager?.defaultLineHeight(for: textView.font ?? .systemFont(ofSize: 14)) ?? 17
            let maxHeight = ceil((lineHeight * 8) + (textView.textContainerInset.height * 2))
            let clampedHeight = max(contentHeight, AgentLayout.composerMinHeight)
            let visibleHeight = min(clampedHeight, maxHeight)

            scrollView.hasVerticalScroller = contentHeight > maxHeight + 0.5
            if abs(measuredHeight - visibleHeight) > 0.5 {
                measuredHeight = visibleHeight
            }
        }
    }
}

private struct AgentPromptInfoBar: View {
    let footerInfo: EditorAgentFooterInfo?
    let fallbackSubtitle: String
    let fallbackContextUsageText: String?
    let isFollowingAgent: Bool
    let followStatusText: String?
    let onToggleFollow: () -> Void
    let onNewSession: () -> Void
    let recentSessions: [EditorAgentRecentSession]
    let onResumeSession: (EditorAgentRecentSession) -> Void
    let onRefreshRecent: () -> Void

    var body: some View {
        HStack(alignment: .bottom, spacing: 10) {
            AgentSessionMenuButton(
                onNewSession: onNewSession,
                recentSessions: recentSessions,
                onResumeSession: onResumeSession,
                onRefreshRecent: onRefreshRecent
            )

            AgentFollowToggleButton(
                isFollowing: isFollowingAgent,
                statusText: followStatusText,
                action: onToggleFollow
            )

            VStack(alignment: .leading, spacing: 3) {
                Text(promptInfoPathLine)
                    .font(.system(size: 10.5, weight: .medium, design: .monospaced))
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
                    .truncationMode(.middle)

                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    AgentPromptStatsLine(
                        footerInfo: footerInfo,
                        fallbackContextUsageText: fallbackContextUsageText
                    )
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .lineLimit(1)

                    Text(promptInfoModelLine)
                        .font(.system(size: 11, weight: .medium, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .fixedSize(horizontal: true, vertical: false)
                }
            }
        }
        .onAppear { onRefreshRecent() }
    }

    private var promptInfoPathLine: String {
        guard let footerInfo else { return "Session · \(fallbackSubtitle)" }
        var line = shortenAgentDisplayPath(footerInfo.cwd) ?? footerInfo.cwd
        if let gitBranch = footerInfo.gitBranch?.trimmingCharacters(in: .whitespacesAndNewlines), !gitBranch.isEmpty {
            line += " (\(gitBranch))"
        }
        if let sessionName = footerInfo.sessionName?.trimmingCharacters(in: .whitespacesAndNewlines), !sessionName.isEmpty {
            line += " • \(sessionName)"
        }
        return line
    }

    private var promptInfoModelLine: String {
        guard let footerInfo else { return fallbackSubtitle }
        let modelID = footerInfo.modelID ?? footerInfo.modelName ?? fallbackSubtitle
        let providerPrefix = footerInfo.availableProviderCount > 1
            ? footerInfo.modelProvider.map { "(\($0)) " } ?? ""
            : ""
        if footerInfo.modelSupportsReasoning {
            let thinkingText = footerInfo.thinkingLevel == "off" ? "thinking off" : footerInfo.thinkingLevel
            return providerPrefix + modelID + " • " + thinkingText
        }
        return providerPrefix + modelID
    }
}

private struct AgentFollowToggleButton: View {
    let isFollowing: Bool
    let statusText: String?
    let action: () -> Void
    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Image(systemName: "scope")
                        .font(.system(size: 11, weight: .semibold))
                    Text(isFollowing ? "Following" : "Follow")
                        .font(.system(size: 11, weight: .semibold))
                }
                .foregroundStyle(isFollowing ? Color.accentColor : .secondary)

                if let statusText, !statusText.isEmpty {
                    Text(statusText)
                        .font(.system(size: 10.5, weight: .medium, design: .monospaced))
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .frame(maxWidth: 160, alignment: .leading)
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .background {
                Capsule().fill((isFollowing ? Color.accentColor : Color.primary).opacity(isFollowing ? 0.14 : (isHovered ? 0.08 : 0.05)))
            }
            .overlay {
                Capsule().strokeBorder((isFollowing ? Color.accentColor : Color.primary).opacity(isFollowing ? 0.28 : 0.08), lineWidth: 0.5)
            }
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .help(isFollowing ? "Stop following the agent" : "Follow the agent as it reads and edits files")
        .onHover { isHovered = $0 }
    }
}

private struct AgentSessionMenuButton: View {
    let onNewSession: () -> Void
    let recentSessions: [EditorAgentRecentSession]
    let onResumeSession: (EditorAgentRecentSession) -> Void
    let onRefreshRecent: () -> Void

    var body: some View {
        Menu {
            Button("New Session", action: onNewSession)

            if !recentSessions.isEmpty {
                Divider()
                Section("Resume") {
                    ForEach(recentSessions.prefix(8)) { session in
                        Button(session.title) { onResumeSession(session) }
                    }
                }
            }
        } label: {
            HStack(spacing: 5) {
                Image(systemName: "brain.head.profile")
                    .font(.system(size: 11, weight: .semibold))
                Image(systemName: "chevron.up.chevron.down")
                    .font(.system(size: 8, weight: .semibold))
            }
            .foregroundStyle(.secondary)
            .padding(.horizontal, 9)
            .padding(.vertical, 7)
            .background {
                Capsule().fill(Color.primary.opacity(0.05))
            }
            .overlay {
                Capsule().strokeBorder(Color.primary.opacity(0.08), lineWidth: 0.5)
            }
            .contentShape(Capsule())
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
        .onAppear { onRefreshRecent() }
    }
}

private struct AgentPromptStatsLine: View {
    let footerInfo: EditorAgentFooterInfo?
    let fallbackContextUsageText: String?

    var body: some View {
        if let footerInfo {
            HStack(spacing: 6) {
                if footerInfo.totalInput > 0 {
                    Text("↑\(agentFormatTokenCount(footerInfo.totalInput))")
                }
                if footerInfo.totalOutput > 0 {
                    Text("↓\(agentFormatTokenCount(footerInfo.totalOutput))")
                }
                if footerInfo.totalCacheRead > 0 {
                    Text("R\(agentFormatTokenCount(footerInfo.totalCacheRead))")
                }
                if footerInfo.totalCacheWrite > 0 {
                    Text("W\(agentFormatTokenCount(footerInfo.totalCacheWrite))")
                }
                if footerInfo.totalCost > 0 || footerInfo.usingSubscription {
                    Text("$\(String(format: "%.3f", footerInfo.totalCost))\(footerInfo.usingSubscription ? " (sub)" : "")")
                }
                if let contextText = agentPromptContextUsageDisplay(footerInfo) {
                    Text(contextText)
                        .foregroundStyle(agentPromptContextUsageColor(footerInfo))
                }
            }
            .font(.system(size: 11, weight: .medium, design: .monospaced))
            .foregroundStyle(.secondary)
            .monospacedDigit()
        } else if let fallbackContextUsageText {
            Text(fallbackContextUsageText)
                .font(.system(size: 11, weight: .medium, design: .monospaced))
                .foregroundStyle(.secondary)
                .monospacedDigit()
        } else {
            Text("Ready")
                .font(.system(size: 11, weight: .medium, design: .monospaced))
                .foregroundStyle(.secondary)
        }
    }
}

// MARK: - Command suggestions

private struct AgentCommandSuggestionsView: View {
    let commands: [EditorAgentCommand]
    let selectedIndex: Int
    let onPick: (EditorAgentCommand) -> Void

    private let rowHeight: CGFloat = 36

    private var listHeight: CGFloat {
        min(max(CGFloat(commands.count) * rowHeight, rowHeight), 240)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 6) {
                Image(systemName: "command")
                    .font(.system(size: 10, weight: .semibold))
                Text("Commands")
                    .font(.system(size: 11, weight: .semibold))
                Spacer()
                if !commands.isEmpty {
                    Text("\(min(selectedIndex + 1, commands.count))/\(commands.count)")
                        .font(.system(size: 10, weight: .medium))
                        .monospacedDigit()
                }
            }
            .foregroundStyle(.secondary)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)

            Divider().opacity(0.3)

            ScrollViewReader { proxy in
                ScrollView(.vertical, showsIndicators: false) {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(Array(commands.enumerated()), id: \.element.id) { index, command in
                            AgentCommandRow(
                                command: command,
                                isSelected: index == selectedIndex,
                                onTap: { onPick(command) }
                            )
                            .id(command.id)
                        }
                    }
                }
                .frame(height: listHeight)
                .onAppear {
                    guard let command = commands[safe: selectedIndex] else { return }
                    proxy.scrollTo(command.id, anchor: .center)
                }
                .onChange(of: selectedIndex) { _, newValue in
                    guard let command = commands[safe: newValue] else { return }
                    withAnimation(.easeOut(duration: 0.12)) {
                        proxy.scrollTo(command.id, anchor: .center)
                    }
                }
            }

            Divider().opacity(0.3)

            HStack(spacing: 10) {
                Label("↩ insert", systemImage: "return")
                Label("↑↓ move", systemImage: "arrow.up.arrow.down")
            }
            .font(.system(size: 10, weight: .medium))
            .foregroundStyle(.tertiary)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
        }
        .background {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(.ultraThinMaterial)
        }
        .overlay {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.1), lineWidth: 0.5)
        }
    }
}

private struct AgentCommandRow: View {
    let command: EditorAgentCommand
    let isSelected: Bool
    let onTap: () -> Void
    @State private var isHovered = false

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 10) {
                Text("/\(command.name)")
                    .font(.system(size: 12, weight: .semibold, design: .monospaced))
                    .foregroundStyle(.primary)
                    .layoutPriority(1)

                if !command.description.isEmpty {
                    Text(command.description)
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }

                Spacer(minLength: 4)

                Text(agentCommandSourceLabel(command.source))
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background {
                        Capsule().fill(Color.primary.opacity(0.06))
                    }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background {
                Rectangle()
                    .fill(isSelected ? Color.primary.opacity(0.08) : (isHovered ? Color.primary.opacity(0.06) : Color.clear))
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

private struct AgentResumeSuggestionsView: View {
    let sessions: [EditorAgentRecentSession]
    let selectedIndex: Int
    let onPick: (EditorAgentRecentSession) -> Void

    private let rowHeight: CGFloat = 54

    private var listHeight: CGFloat {
        min(max(CGFloat(max(sessions.count, 1)) * rowHeight, rowHeight), 240)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                Image(systemName: "clock.arrow.circlepath")
                    .font(.system(size: 10, weight: .semibold))
                Text("Resume session")
                    .font(.system(size: 11, weight: .medium))
                Spacer(minLength: 6)
                if !sessions.isEmpty {
                    Text("\(min(selectedIndex + 1, sessions.count))/\(sessions.count)")
                        .font(.system(size: 10, weight: .medium))
                        .foregroundStyle(.secondary)
                        .monospacedDigit()
                }
            }
            .foregroundStyle(.secondary)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)

            Divider().opacity(0.3)

            ScrollViewReader { proxy in
                ScrollView(.vertical, showsIndicators: false) {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        if sessions.isEmpty {
                            Text("No matching sessions")
                                .font(.system(size: 12))
                                .foregroundStyle(.secondary)
                                .padding(.horizontal, 12)
                                .padding(.vertical, 12)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        } else {
                            ForEach(Array(sessions.enumerated()), id: \.element.id) { index, session in
                                Button {
                                    onPick(session)
                                } label: {
                                    VStack(alignment: .leading, spacing: 3) {
                                        HStack(spacing: 8) {
                                            Text(session.title)
                                                .font(.system(size: 12, weight: .medium))
                                                .foregroundStyle(.primary)
                                                .lineLimit(1)
                                            Spacer(minLength: 4)
                                            Text(session.modified)
                                                .font(.system(size: 10))
                                                .foregroundStyle(.tertiary)
                                                .lineLimit(1)
                                        }
                                        if !session.firstMessage.isEmpty, session.firstMessage != session.title {
                                            Text(session.firstMessage)
                                                .font(.system(size: 11))
                                                .foregroundStyle(.secondary)
                                                .lineLimit(1)
                                        }
                                    }
                                    .padding(.horizontal, 12)
                                    .padding(.vertical, 8)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .background {
                                        Rectangle()
                                            .fill(index == selectedIndex ? Color.primary.opacity(0.08) : Color.clear)
                                    }
                                    .contentShape(Rectangle())
                                }
                                .buttonStyle(.plain)
                                .id(session.id)
                            }
                        }
                    }
                }
                .frame(height: listHeight)
                .onAppear {
                    guard let session = sessions[safe: selectedIndex] else { return }
                    proxy.scrollTo(session.id, anchor: .center)
                }
                .onChange(of: selectedIndex) { _, newValue in
                    guard let session = sessions[safe: newValue] else { return }
                    withAnimation(.easeOut(duration: 0.12)) {
                        proxy.scrollTo(session.id, anchor: .center)
                    }
                }
            }
        }
        .background {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(.ultraThinMaterial)
        }
        .overlay {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.1), lineWidth: 0.5)
        }
    }
}

private struct AgentModelSuggestionsView: View {
    let models: [EditorAgentModel]
    let selectedIndex: Int
    let onPick: (EditorAgentModel) -> Void

    private let rowHeight: CGFloat = 34

    private var listHeight: CGFloat {
        min(max(CGFloat(max(models.count, 1)) * rowHeight, rowHeight), 260)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                Image(systemName: "cpu")
                    .font(.system(size: 10, weight: .semibold))
                Text("Only showing models with configured API keys")
                    .font(.system(size: 11, weight: .medium))
                Spacer(minLength: 6)
                if !models.isEmpty {
                    Text("\(min(selectedIndex + 1, models.count))/\(models.count)")
                        .font(.system(size: 10, weight: .medium))
                        .foregroundStyle(.secondary)
                        .monospacedDigit()
                }
            }
            .foregroundStyle(.secondary)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)

            Divider().opacity(0.3)

            ScrollViewReader { proxy in
                ScrollView(.vertical, showsIndicators: false) {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        if models.isEmpty {
                            Text("No matching models")
                                .font(.system(size: 12))
                                .foregroundStyle(.secondary)
                                .padding(.horizontal, 12)
                                .padding(.vertical, 12)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        } else {
                            ForEach(Array(models.enumerated()), id: \.element.reference) { index, model in
                                AgentModelRow(
                                    model: model,
                                    isSelected: index == selectedIndex,
                                    onTap: { onPick(model) }
                                )
                                .id(model.reference)
                            }
                        }
                    }
                }
                .frame(height: listHeight)
                .onAppear {
                    guard let model = models[safe: selectedIndex] else { return }
                    proxy.scrollTo(model.reference, anchor: .center)
                }
                .onChange(of: selectedIndex) { _, newValue in
                    guard let model = models[safe: newValue] else { return }
                    withAnimation(.easeOut(duration: 0.12)) {
                        proxy.scrollTo(model.reference, anchor: .center)
                    }
                }
            }

            if let selected = models[safe: selectedIndex] {
                Divider().opacity(0.3)
                Text(selected.name)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .background {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(.ultraThinMaterial)
        }
        .overlay {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.1), lineWidth: 0.5)
        }
    }
}

private struct AgentModelRow: View {
    let model: EditorAgentModel
    let isSelected: Bool
    let onTap: () -> Void
    @State private var isHovered = false

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 8) {
                Text(isSelected ? "→" : " ")
                    .font(.system(size: 12, weight: .semibold, design: .monospaced))
                    .foregroundStyle(isSelected ? Color.accentColor : .clear)
                    .frame(width: 12, alignment: .leading)

                Text(model.displayName)
                    .font(.system(size: 12.5, weight: isSelected ? .semibold : .regular, design: .monospaced))
                    .foregroundStyle(isSelected ? Color.accentColor : .primary)

                Text("[\(model.provider)]")
                    .font(.system(size: 12, weight: .medium, design: .monospaced))
                    .foregroundStyle(.secondary)

                if model.isCurrent {
                    Image(systemName: "checkmark")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(.green)
                }

                Spacer(minLength: 6)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background {
                Rectangle()
                    .fill(isSelected ? Color.primary.opacity(0.08) : (isHovered ? Color.primary.opacity(0.05) : Color.clear))
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

// MARK: - Error banner

private struct AgentErrorBanner: View {
    let message: String

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.red)

            Text(message)
                .font(.system(size: 11))
                .foregroundStyle(.primary)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background {
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(Color.red.opacity(0.08))
        }
        .overlay {
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .strokeBorder(Color.red.opacity(0.24), lineWidth: 0.5)
        }
    }
}

// MARK: - Top scrim

private struct AgentPaneTopScrim: View {
    let height: CGFloat
    let backgroundColor: NSColor

    var body: some View {
        Rectangle()
            .fill(Color(nsColor: backgroundColor))
            .frame(height: max(height, 0))
            .frame(maxWidth: .infinity, alignment: .top)
            .allowsHitTesting(false)
    }
}

// MARK: - Matching helpers

private func exactModelMatch(for query: String, models: [EditorAgentModel]) -> EditorAgentModel? {
    let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else { return nil }
    let normalized = trimmed.lowercased()

    let canonicalMatches = models.filter { $0.reference.lowercased() == normalized }
    if canonicalMatches.count == 1 {
        return canonicalMatches[0]
    }

    if let slashIndex = trimmed.firstIndex(of: "/") {
        let provider = String(trimmed[..<slashIndex]).trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let modelID = String(trimmed[trimmed.index(after: slashIndex)...]).trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let providerMatches = models.filter {
            $0.provider.lowercased() == provider && $0.id.lowercased() == modelID
        }
        if providerMatches.count == 1 {
            return providerMatches[0]
        }
    }

    let idMatches = models.filter { $0.id.lowercased() == normalized }
    return idMatches.count == 1 ? idMatches[0] : nil
}

private func fuzzyFilterCommands(_ commands: [EditorAgentCommand], query: String) -> [EditorAgentCommand] {
    let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else { return commands }

    return commands
        .compactMap { command in
            agentFuzzyBestScore(query: trimmed, candidates: [
                command.name,
                "\(command.name) \(command.description)",
                command.description,
            ]).map { (command, $0) }
        }
        .sorted { lhs, rhs in
            if lhs.0.isBuiltin != rhs.0.isBuiltin { return lhs.0.isBuiltin }
            if lhs.1 != rhs.1 { return lhs.1 < rhs.1 }
            return lhs.0.name.localizedCaseInsensitiveCompare(rhs.0.name) == .orderedAscending
        }
        .map(\.0)
}

private func fuzzyFilterModels(_ models: [EditorAgentModel], query: String) -> [EditorAgentModel] {
    let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else {
        return models.sorted { lhs, rhs in
            if lhs.isCurrent != rhs.isCurrent { return lhs.isCurrent }
            let providerCompare = lhs.provider.localizedCaseInsensitiveCompare(rhs.provider)
            if providerCompare != .orderedSame { return providerCompare == .orderedAscending }
            return lhs.id.localizedCaseInsensitiveCompare(rhs.id) == .orderedAscending
        }
    }

    return models
        .compactMap { model in
            agentFuzzyBestScore(query: trimmed, candidates: [
                model.reference,
                "\(model.id) \(model.provider)",
                "\(model.id) \(model.provider) \(model.name)",
                model.name,
            ]).map { (model, $0) }
        }
        .sorted { lhs, rhs in
            if lhs.0.isCurrent != rhs.0.isCurrent { return lhs.0.isCurrent }
            if lhs.1 != rhs.1 { return lhs.1 < rhs.1 }
            let providerCompare = lhs.0.provider.localizedCaseInsensitiveCompare(rhs.0.provider)
            if providerCompare != .orderedSame { return providerCompare == .orderedAscending }
            return lhs.0.id.localizedCaseInsensitiveCompare(rhs.0.id) == .orderedAscending
        }
        .map(\.0)
}

private func exactRecentSessionMatch(for query: String, sessions: [EditorAgentRecentSession]) -> EditorAgentRecentSession? {
    let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else { return nil }
    let normalized = trimmed.lowercased()

    let pathMatches = sessions.filter { $0.path.lowercased() == normalized }
    if pathMatches.count == 1 {
        return pathMatches[0]
    }

    let titleMatches = sessions.filter { $0.title.lowercased() == normalized }
    if titleMatches.count == 1 {
        return titleMatches[0]
    }

    let firstMessageMatches = sessions.filter { $0.firstMessage.lowercased() == normalized }
    return firstMessageMatches.count == 1 ? firstMessageMatches[0] : nil
}

private func fuzzyFilterSessions(_ sessions: [EditorAgentRecentSession], query: String) -> [EditorAgentRecentSession] {
    let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else { return sessions }

    return sessions
        .compactMap { session in
            agentFuzzyBestScore(query: trimmed, candidates: [
                session.title,
                session.firstMessage,
                session.path,
            ]).map { (session, $0) }
        }
        .sorted { lhs, rhs in
            if lhs.1 != rhs.1 { return lhs.1 < rhs.1 }
            return lhs.0.modified.localizedCaseInsensitiveCompare(rhs.0.modified) == .orderedDescending
        }
        .map(\.0)
}

private func agentCommandSourceLabel(_ source: String) -> String {
    switch source {
    case "builtin":
        return "pi"
    case "prompt":
        return "prompt"
    case "extension":
        return "ext"
    case "skill":
        return "skill"
    default:
        return source
    }
}

private func agentFormatTokenCount(_ count: Int) -> String {
    if count < 1_000 { return "\(count)" }
    if count < 10_000 { return String(format: "%.1fk", Double(count) / 1_000) }
    if count < 1_000_000 { return "\(Int(round(Double(count) / 1_000)))k" }
    if count < 10_000_000 { return String(format: "%.1fM", Double(count) / 1_000_000) }
    return "\(Int(round(Double(count) / 1_000_000)))M"
}

private func agentPromptContextUsageDisplay(_ footerInfo: EditorAgentFooterInfo) -> String? {
    guard let contextWindow = footerInfo.contextWindow, contextWindow > 0 else { return nil }
    let autoSuffix = footerInfo.autoCompactEnabled ? " (auto)" : ""
    if let percent = footerInfo.contextPercent {
        return "\(String(format: "%.1f", percent))%%/\(agentFormatTokenCount(contextWindow))\(autoSuffix)"
    }
    return "?/\(agentFormatTokenCount(contextWindow))\(autoSuffix)"
}

private func agentPromptContextUsageColor(_ footerInfo: EditorAgentFooterInfo) -> Color {
    guard let percent = footerInfo.contextPercent else { return .secondary }
    if percent > 90 {
        return .red
    }
    if percent > 70 {
        return .orange
    }
    return .secondary
}

private func agentFuzzyBestScore(query: String, candidates: [String]) -> Double? {
    candidates.compactMap { agentFuzzyScore(query: query, candidate: $0) }.min()
}

private func agentFuzzyScore(query: String, candidate: String) -> Double? {
    let tokens = query
        .trimmingCharacters(in: .whitespacesAndNewlines)
        .split(whereSeparator: { $0.isWhitespace })
        .map(String.init)
        .filter { !$0.isEmpty }
    guard !tokens.isEmpty else { return 0 }

    var total: Double = 0
    for token in tokens {
        guard let score = agentFuzzySingleTokenScore(query: token, candidate: candidate) else {
            return nil
        }
        total += score
    }
    return total
}

private func agentFuzzySingleTokenScore(query: String, candidate: String) -> Double? {
    let needle = query.lowercased()
    let haystack = candidate.lowercased()
    guard !needle.isEmpty else { return 0 }
    guard needle.count <= haystack.count else { return nil }

    func score(_ normalizedQuery: String) -> Double? {
        var queryIndex = normalizedQuery.startIndex
        var lastMatchIndex: Int?
        var consecutiveMatches = 0
        var total: Double = 0

        for (index, scalar) in haystack.enumerated() {
            guard queryIndex < normalizedQuery.endIndex else { break }
            if scalar == normalizedQuery[queryIndex] {
                let isWordBoundary: Bool
                if index == 0 {
                    isWordBoundary = true
                } else {
                    let previous = haystack[haystack.index(haystack.startIndex, offsetBy: index - 1)]
                    isWordBoundary = previous == " " || previous == "-" || previous == "_" || previous == "." || previous == "/" || previous == ":"
                }

                if let lastMatchIndex, lastMatchIndex == index - 1 {
                    consecutiveMatches += 1
                    total -= Double(consecutiveMatches * 5)
                } else {
                    consecutiveMatches = 0
                    if let lastMatchIndex {
                        total += Double((index - lastMatchIndex - 1) * 2)
                    }
                }

                if isWordBoundary {
                    total -= 10
                }

                total += Double(index) * 0.1
                lastMatchIndex = index
                queryIndex = normalizedQuery.index(after: queryIndex)
            }
        }

        return queryIndex == normalizedQuery.endIndex ? total : nil
    }

    if let direct = score(needle) {
        return direct
    }

    let alphaNumeric = needle.range(of: "^([a-z]+)([0-9]+)$", options: .regularExpression)
    let numericAlpha = needle.range(of: "^([0-9]+)([a-z]+)$", options: .regularExpression)

    if let range = alphaNumeric {
        let matched = String(needle[range])
        let letters = matched.prefix { $0.isLetter }
        let digits = matched.drop(while: { $0.isLetter })
        return score(String(digits) + String(letters)).map { $0 + 5 }
    }
    if let range = numericAlpha {
        let matched = String(needle[range])
        let digits = matched.prefix { $0.isNumber }
        let letters = matched.drop(while: { $0.isNumber })
        return score(String(letters) + String(digits)).map { $0 + 5 }
    }

    return nil
}

private extension Collection {
    subscript(safe index: Index) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}

// MARK: - Color helpers

private extension NSColor {
    var agentIsLightColor: Bool {
        guard let color = usingColorSpace(.sRGB) else { return false }
        let luminance = (0.299 * color.redComponent) + (0.587 * color.greenComponent) + (0.114 * color.blueComponent)
        return luminance > 0.7
    }
}
