import AppKit
import Combine
import Foundation
import SwiftUI
import TheEditorFFI

@MainActor
protocol EditorSurfaceControllerDelegate: AnyObject {
    func editorController(_ controller: EditorSurfaceController, didUpdateScene scene: EditorRenderScene)
}

private struct EditorHandleBox: @unchecked Sendable {
    let raw: OpaquePointer
}

private struct EventMonitorBox: @unchecked Sendable {
    let raw: Any
}

struct EditorActiveTerminalStatus: Equatable {
    let title: String?
    let workingDirectory: String?
}

struct EditorInlineAssistModelSelection: Equatable {
    let provider: String
    let modelID: String

    var reference: String {
        "\(provider)/\(modelID)"
    }
}

struct EditorInlineAssistState: Equatable, Identifiable {
    enum Phase: Equatable {
        case editing
        case generating
    }

    let id: UUID
    let selectionRange: NSRange
    let selectionCharRange: Range<Int>
    let selectionText: String
    let filePath: String?
    let sourceLabel: String
    let lineRange: ClosedRange<Int>?
    let language: String?
    var prompt: String
    var phase: Phase
    var errorMessage: String?

    init(
        id: UUID = UUID(),
        selectionRange: NSRange,
        selectionCharRange: Range<Int>,
        selectionText: String,
        filePath: String?,
        sourceLabel: String,
        lineRange: ClosedRange<Int>?,
        language: String?,
        prompt: String = "",
        phase: Phase = .editing,
        errorMessage: String? = nil
    ) {
        self.id = id
        self.selectionRange = selectionRange
        self.selectionCharRange = selectionCharRange
        self.selectionText = selectionText
        self.filePath = filePath
        self.sourceLabel = sourceLabel
        self.lineRange = lineRange
        self.language = language
        self.prompt = prompt
        self.phase = phase
        self.errorMessage = errorMessage
    }

    var titleText: String {
        if let lineRange {
            if lineRange.lowerBound == lineRange.upperBound {
                return "\(sourceLabel):\(lineRange.lowerBound)"
            }
            return "\(sourceLabel):\(lineRange.lowerBound)-\(lineRange.upperBound)"
        }
        return sourceLabel
    }
}

private struct EditorAgentSelectionContext {
    let text: String
    let sourceLabel: String
    let filePath: String?
    let utf16Range: NSRange
    let charRange: Range<Int>
    let lineRange: ClosedRange<Int>?
    let language: String?
}

@MainActor
final class EditorSurfaceController: ObservableObject {
    weak var delegate: EditorSurfaceControllerDelegate?
    weak var editorFirstResponder: NSView?

    fileprivate var handle: EditorHandleBox?
    private(set) var scene: EditorRenderScene?
    private(set) var chrome = EditorChromeModel.empty
    private(set) var currentMode: EditorMode = .normal
    private(set) var pendingKeys: EditorPendingKeyState?
    private(set) var commandPalette: EditorCommandPaletteState = .empty
    private(set) var completionMenu: EditorCompletionMenuState = .empty
    private(set) var inlineCompletion: EditorInlineCompletionState = .empty
    private(set) var inputPrompt: EditorInputPromptState = .empty
    private(set) var hoverDocs: EditorDocsPanelState = .empty
    private(set) var completionDocs: EditorDocsPanelState = .empty
    private(set) var signatureHelp: EditorDocsPanelState = .empty
    private(set) var filePicker: EditorFilePickerState = .empty
    private(set) var bufferTabs: EditorBufferTabsState = .empty
    private(set) var openItems: EditorPaneOpenItemsState = .empty
    private(set) var fileTree: EditorFileTreeState = .empty
    @Published private(set) var sidebarMode: EditorSidebarMode
    @Published private(set) var showsResizeOverlay = false
    @Published private(set) var bufferFontSize: CGFloat = EditorSurfaceController.defaultBufferFontSize
    @Published private(set) var agentControlledPaneID: UInt?
    @Published private(set) var inlineAssistState: EditorInlineAssistState?
    @Published private(set) var inlineAssistModels: [EditorAgentModel] = []
    @Published private(set) var inlineAssistSelectedModel: EditorInlineAssistModelSelection?
    @Published private(set) var inlineAssistThinkingLevel: String = "medium"
    @Published private(set) var inlineAssistFocusRequestToken: UInt64 = 0

    static let defaultBufferFontSize: CGFloat = 14
    private static let minBufferFontSize: CGFloat = 6
    private static let maxBufferFontSize: CGFloat = 100
    private static let bufferFontStep: CGFloat = 1

    private struct TerminalPresentationState {
        var title: String?
        var workingDirectory: String?
    }

    private var baseOpenItems: EditorPaneOpenItemsState = .empty
    private var terminalPresentationBySurfaceID: [UInt: TerminalPresentationState] = [:]

    private var surfaceConfiguration: EditorSurfaceConfiguration?
    private var surfaceConfigurationSignature: EditorSurfaceConfigurationSignature?
    private var markedText: String = ""
    private var backgroundPollCancellable: AnyCancellable?
    private var closeSurfaceEventMonitor: EventMonitorBox?
    private var filePickerListVisibleRows: Int = 1
    private var filePickerPreviewVisibleRows: Int = 1
    private var fileTreeVisibleRows: Int = 1
    private var resizeOverlayHideTask: Task<Void, Never>?
    private var interactiveResizeReasons: Set<String> = []
    private var pendingSurfaceConfiguration: EditorSurfaceConfiguration?
    private var surfaceConfigureFlushTask: Task<Void, Never>?
    private var inlineAssistTask: Task<Void, Never>?
    private var inlineAssistModelsTask: Task<Void, Never>?
    private var inlineAssistRequestGeneration: UInt64 = 0
    private var lastSurfaceConfigureAt: CFAbsoluteTime = 0

    private let interactiveResizeMinInterval: CFTimeInterval = 1.0 / 30.0
    private let supportedInlineAssistThinkingLevels = ["off", "minimal", "low", "medium", "high"]
    private static let sidebarModeDefaultsKey = "swift.sidebar.mode"

    lazy var agentSessionSupervisor = EditorAgentSessionSupervisor(controller: self)
    lazy var agentSidebarCoordinator = EditorAgentSidebarCoordinator(controller: self)

    init(initialPath: String?) {
        self.sidebarMode = EditorSidebarMode(
            rawValue: UserDefaults.standard.string(forKey: Self.sidebarModeDefaultsKey) ?? EditorSidebarMode.files.rawValue
        ) ?? .files
        self.handle = EditorFFIBridge.createHandle(initialPath: initialPath).map(EditorHandleBox.init(raw:))
        _ = EditorFFIBridge.setEmbeddedTerminalEnabled(handle?.raw, enabled: GhosttyTerminalRegistry.isAvailable)
        installCloseSurfaceKeyMonitor()
        startBackgroundPolling()
        refreshSnapshot()
    }

    deinit {
        resizeOverlayHideTask?.cancel()
        surfaceConfigureFlushTask?.cancel()
        inlineAssistTask?.cancel()
        inlineAssistModelsTask?.cancel()
        if let closeSurfaceEventMonitor {
            NSEvent.removeMonitor(closeSurfaceEventMonitor.raw)
        }
        EditorFFIBridge.destroyHandle(handle?.raw)
    }

    private func describeSelections(_ selections: [EditorSnapshotSelection]) -> String {
        guard !selections.isEmpty else { return "[]" }
        let items = selections.prefix(4).map { selection in
            "kind=\(selection.kind.rawValue) rect=(\(selection.x),\(selection.y),\(selection.width),\(selection.height))"
        }
        let suffix = selections.count > items.count ? " +\(selections.count - items.count) more" : ""
        return "[\(items.joined(separator: " | "))]\(suffix)"
    }

    private func describeCursors(_ cursors: [EditorSnapshotCursor]) -> String {
        guard !cursors.isEmpty else { return "[]" }
        let items = cursors.prefix(3).map { cursor in
            "kind=\(cursor.kind.rawValue) pos=(\(cursor.col),\(cursor.row))"
        }
        let suffix = cursors.count > items.count ? " +\(cursors.count - items.count) more" : ""
        return "[\(items.joined(separator: " | "))]\(suffix)"
    }

    private func logSelectionSnapshot(_ label: String, snapshot: EditorSnapshot) {
        selectionDebugLog(
            "controller.\(label) damage=\(snapshot.info.damageReason) full=\(snapshot.info.damageIsFull) scroll=(\(snapshot.info.scrollRow),\(snapshot.info.scrollCol)) scrollGen=\(snapshot.info.scrollGeneration) selections=\(describeSelections(snapshot.selections)) cursors=\(describeCursors(snapshot.cursors))"
        )
    }

    private func logSceneSelectionState(_ label: String, scene: EditorRenderScene) {
        selectionDebugLog(
            "controller.\(label) scroll=(\(scene.info.scrollRow),\(scene.info.scrollCol)) scrollGen=\(scene.info.scrollGeneration) selections=\(describeSelections(scene.selections)) cursors=\(describeCursors(scene.cursors))"
        )
    }

    var isInteractiveResizeActive: Bool {
        !interactiveResizeReasons.isEmpty
    }

    @discardableResult
    func configureSurface(size: CGSize, backingScale: CGFloat, fontMetrics: EditorFontMetrics) -> Bool {
        sidebarPerfIncrement("surface.configure.request")
        let previousConfiguration = surfaceConfiguration
        let previousSignature = surfaceConfigurationSignature
        let configuration = fontMetrics.surfaceConfiguration(viewSize: size, backingScale: backingScale)
        let signature = configuration.meaningfulSignature
        if signature == previousSignature {
            surfaceConfiguration = configuration
            sidebarPerfIncrement("surface.configure.unchanged")
            return false
        }
        let sizeText = String(format: "%.1fx%.1f", size.width, size.height)
        let scaleText = String(format: "%.2f", backingScale)
        let oldPxText = previousConfiguration.map { "\($0.widthPx)x\($0.heightPx)" } ?? "nil"
        let newPxText = "\(configuration.widthPx)x\(configuration.heightPx)"
        let cellPxText = "\(configuration.metrics.cellWidthPx)x\(configuration.metrics.cellHeightPx)"
        let reasons = interactiveResizeReasons.sorted()
        let reasonText = reasons.joined(separator: ",")
        let now = CFAbsoluteTimeGetCurrent()
        let elapsed = now - lastSurfaceConfigureAt

        if isInteractiveResizeActive && elapsed < interactiveResizeMinInterval {
            sidebarPerfIncrement("surface.configure.deferred")
            pendingSurfaceConfiguration = configuration
            schedulePendingSurfaceConfigurationFlush(after: interactiveResizeMinInterval - elapsed)
            let elapsedMsText = String(format: "%.2f", elapsed * 1000)
            scrollPerfLog(
                "controller.configureSurface deferred reasons=\(reasonText) size=\(sizeText) backingScale=\(scaleText) oldPx=\(oldPxText) newPx=\(newPxText) cellPx=\(cellPxText) elapsedMs=\(elapsedMsText)"
            )
            return false
        }

        return applySurfaceConfiguration(
            configuration,
            sizeText: sizeText,
            scaleText: scaleText,
            oldPxText: oldPxText,
            newPxText: newPxText,
            cellPxText: cellPxText,
            reasonText: reasonText,
            source: "immediate"
        )
    }

    func beginInteractiveResize(reason: String) {
        let inserted = interactiveResizeReasons.insert(reason).inserted
        guard inserted else { return }
        sidebarPerfIncrement("interactiveResize.begin")
        scrollPerfLog("controller.interactiveResize begin reason=\(reason) active=\(interactiveResizeReasons.sorted())")
    }

    func endInteractiveResize(reason: String) {
        let removed = interactiveResizeReasons.remove(reason) != nil
        guard removed else { return }
        sidebarPerfIncrement("interactiveResize.end")
        scrollPerfLog("controller.interactiveResize end reason=\(reason) active=\(interactiveResizeReasons.sorted())")
        if !isInteractiveResizeActive {
            flushPendingSurfaceConfiguration(source: "interactive-end")
        }
    }

    @discardableResult
    private func applySurfaceConfiguration(
        _ configuration: EditorSurfaceConfiguration,
        sizeText: String,
        scaleText: String,
        oldPxText: String,
        newPxText: String,
        cellPxText: String,
        reasonText: String,
        source: String
    ) -> Bool {
        sidebarPerfIncrement("surface.configure.apply")
        pendingSurfaceConfiguration = nil
        surfaceConfigureFlushTask?.cancel()
        surfaceConfigureFlushTask = nil
        surfaceConfiguration = configuration
        surfaceConfigurationSignature = configuration.meaningfulSignature
        lastSurfaceConfigureAt = CFAbsoluteTimeGetCurrent()
        scrollPerfLog(
            "controller.configureSurface apply source=\(source) reasons=\(reasonText) size=\(sizeText) backingScale=\(scaleText) oldPx=\(oldPxText) newPx=\(newPxText) cellPx=\(cellPxText)"
        )
        let ffiConfigured = measureSidebarSignpostedInterval("SurfaceConfigureFFI", counterKey: "surface.configure.ffi.ms") {
            EditorFFIBridge.configureSurface(handle?.raw, configuration: configuration)
        }
        guard ffiConfigured else { return false }
        measureSidebarSignpostedInterval("SurfaceConfigureRefreshSnapshot", counterKey: "surface.configure.refreshSnapshot.ms") {
            refreshSnapshot()
        }
        return true
    }

    private func schedulePendingSurfaceConfigurationFlush(after delay: CFTimeInterval) {
        guard delay.isFinite, delay > 0 else {
            flushPendingSurfaceConfiguration(source: "scheduled-immediate")
            return
        }
        surfaceConfigureFlushTask?.cancel()
        surfaceConfigureFlushTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(delay))
            guard let self else { return }
            self.flushPendingSurfaceConfiguration(source: "scheduled")
        }
    }

    private func flushPendingSurfaceConfiguration(source: String) {
        guard let configuration = pendingSurfaceConfiguration else { return }
        let oldPxText = surfaceConfiguration.map { "\($0.widthPx)x\($0.heightPx)" } ?? "nil"
        let newPxText = "\(configuration.widthPx)x\(configuration.heightPx)"
        let cellPxText = "\(configuration.metrics.cellWidthPx)x\(configuration.metrics.cellHeightPx)"
        let reasonText = interactiveResizeReasons.sorted().joined(separator: ",")
        let sizeText = "deferred"
        let scaleText = "deferred"
        _ = applySurfaceConfiguration(
            configuration,
            sizeText: sizeText,
            scaleText: scaleText,
            oldPxText: oldPxText,
            newPxText: newPxText,
            cellPxText: cellPxText,
            reasonText: reasonText,
            source: source
        )
    }

    func setScrollRow(_ row: Int) {
        guard EditorFFIBridge.setScrollRow(handle?.raw, row: UInt32(max(row, 0))) else { return }
        refreshSnapshot()
    }

    func setScrollCol(_ col: Int) {
        guard EditorFFIBridge.setScrollCol(handle?.raw, col: UInt32(max(col, 0))) else { return }
        refreshSnapshot()
    }

    func increaseBufferFontSize() {
        adjustBufferFontSize(by: Self.bufferFontStep)
    }

    func decreaseBufferFontSize() {
        adjustBufferFontSize(by: -Self.bufferFontStep)
    }

    func resetBufferFontSize() {
        setBufferFontSize(Self.defaultBufferFontSize)
    }

    private func adjustBufferFontSize(by delta: CGFloat) {
        setBufferFontSize(bufferFontSize + delta)
    }

    private func setBufferFontSize(_ pointSize: CGFloat) {
        let clampedPointSize = min(max(pointSize, Self.minBufferFontSize), Self.maxBufferFontSize)
        guard abs(clampedPointSize - bufferFontSize) > 0.001 else { return }
        bufferFontSize = clampedPointSize
    }

    func setActivePane(_ paneID: UInt) {
        var changed = EditorFFIBridge.setFileTreeActive(handle?.raw, active: false)
        changed = EditorFFIBridge.setActivePane(handle?.raw, paneID: paneID) || changed
        guard changed else { return }
        refreshSnapshot()
    }

    func resizeSplit(_ splitID: UInt, x: Int, y: Int) {
        guard EditorFFIBridge.resizeSplit(handle?.raw, splitID: splitID, x: x, y: y) else { return }
        refreshSnapshot()
    }

    func clickBufferPosition(
        paneID: UInt,
        logicalCol: Int,
        logicalRow: Int,
        modifiers: UInt8,
        clickCount: Int
    ) {
        selectionDebugLog(
            "controller.clickBufferPosition pane=\(paneID) logical=(\(logicalCol),\(logicalRow)) modifiers=\(modifiers) clickCount=\(clickCount)"
        )
        guard EditorFFIBridge.clickBufferPosition(
            handle?.raw,
            paneID: paneID,
            logicalCol: logicalCol,
            logicalRow: logicalRow,
            modifiers: modifiers,
            clickCount: clickCount
        ) else { return }
        refreshSnapshot()
    }

    func activateBufferTab(_ bufferID: UInt) {
        guard EditorFFIBridge.activateBufferTab(handle?.raw, bufferID: bufferID) else { return }
        refreshSnapshot()
    }

    func closeBufferTab(_ bufferID: UInt) {
        guard EditorFFIBridge.closeBufferTab(handle?.raw, bufferID: bufferID) else { return }
        refreshSnapshot()
    }

    func activateOpenItem(_ item: EditorPaneOpenItemRow) {
        guard EditorFFIBridge.activateOpenItem(handle?.raw, paneID: item.paneID, kind: item.kind, itemID: item.itemID) else { return }
        refreshSnapshot()
    }

    func setAgentControlledPane(_ paneID: UInt?) {
        guard agentControlledPaneID != paneID else { return }
        agentControlledPaneID = paneID
    }

    @discardableResult
    func revealAgentFollowLocation(
        path: String,
        lineStart: Int?,
        lineEnd: Int?,
        agentItemID: UInt,
        preferredPaneID: UInt?
    ) -> UInt? {
        let targetPaneID = resolvedAgentFollowPaneID(preferredPaneID)
        if let targetPaneID,
           openItems.groups.contains(where: { $0.paneID == targetPaneID }) {
            setActivePane(targetPaneID)
        }

        guard EditorFFIBridge.followPath(handle?.raw, path: path, startLine: lineStart, endLine: lineEnd) else {
            if targetPaneID != nil {
                activateAgentUIIfNeeded(agentItemID: agentItemID)
            }
            return preferredPaneID
        }

        refreshSnapshot()
        let resolvedPaneID = activeOpenItem?.paneID ?? resolvedAgentFollowPaneID(preferredPaneID)
        if let resolvedPaneID {
            setAgentControlledPane(resolvedPaneID)
        }
        activateAgentUIIfNeeded(agentItemID: agentItemID)
        return resolvedPaneID
    }

    @discardableResult
    func setAgentFollowPreview(path: String, text: String, lineStart: Int?, lineEnd: Int?) -> Bool {
        if let agentControlledPaneID,
           openItems.groups.contains(where: { $0.paneID == agentControlledPaneID }) {
            setActivePane(agentControlledPaneID)
        }
        guard EditorFFIBridge.followPreviewContents(handle?.raw, path: path, text: text, startLine: lineStart, endLine: lineEnd) else {
            return false
        }
        refreshSnapshot()
        if let paneID = activeOpenItem?.paneID ?? openItems.groups.first(where: { $0.isActivePane })?.paneID {
            setAgentControlledPane(paneID)
        }
        return true
    }

    func animateAgentFollowTextUpdate(path: String, originalText: String, updatedText: String, lineStart: Int?, lineEnd: Int?) async {
        let frames = agentFollowAnimationFrames(from: originalText, to: updatedText)
        guard !frames.isEmpty else {
            _ = setAgentFollowPreview(path: path, text: updatedText, lineStart: lineStart, lineEnd: lineEnd)
            return
        }

        for frame in frames {
            _ = setAgentFollowPreview(path: path, text: frame, lineStart: lineStart, lineEnd: lineEnd)
            try? await Task.sleep(for: .milliseconds(18))
        }
        _ = setAgentFollowPreview(path: path, text: updatedText, lineStart: lineStart, lineEnd: lineEnd)
    }

    private func resolvedAgentFollowPaneID(_ preferredPaneID: UInt?) -> UInt? {
        if let preferredPaneID,
           openItems.groups.contains(where: { group in
               group.paneID == preferredPaneID && group.items.contains(where: { $0.kind == .buffer })
           }) {
            return preferredPaneID
        }

        if let activeGroup = openItems.groups.first(where: { $0.isActivePane }),
           activeGroup.items.contains(where: { $0.kind == .buffer }) {
            return activeGroup.paneID
        }

        return openItems.groups.first(where: { group in
            group.items.contains(where: { $0.kind == .buffer })
        })?.paneID
    }

    @discardableResult
    func activatePaneLocalOpenItem(at index: Int) -> Bool {
        guard index >= 0 else { return false }
        guard let item = openItems.groups.first(where: { $0.isActivePane })?.items[safe: index] else {
            return false
        }
        if item.isActive {
            if item.kind == .buffer {
                focusEditor()
            }
            return true
        }
        activateOpenItem(item)
        if item.kind == .buffer {
            focusEditor()
        }
        return true
    }

    func closeOpenItem(_ item: EditorPaneOpenItemRow) {
        guard EditorFFIBridge.closeOpenItem(handle?.raw, paneID: item.paneID, kind: item.kind, itemID: item.itemID) else { return }
        refreshSnapshot()
        if item.kind == .buffer {
            focusEditor()
        }
    }

    func moveOpenItem(_ item: EditorPaneOpenItemRow, toPaneID targetPaneID: UInt, atIndex targetIndex: Int) {
        guard EditorFFIBridge.moveOpenItem(
            handle?.raw,
            sourcePaneID: item.paneID,
            kind: item.kind,
            itemID: item.itemID,
            targetPaneID: targetPaneID,
            targetIndex: targetIndex
        ) else {
            return
        }
        refreshSnapshot()
        if item.kind == .buffer {
            focusEditor()
        }
    }

    func splitOpenItem(_ item: EditorPaneOpenItemRow, ontoPaneID targetPaneID: UInt, direction: EditorPaneDropDirection) {
        guard EditorFFIBridge.splitOpenItem(
            handle?.raw,
            sourcePaneID: item.paneID,
            kind: item.kind,
            itemID: item.itemID,
            targetPaneID: targetPaneID,
            direction: direction
        ) else {
            return
        }
        refreshSnapshot()
        if item.kind == .buffer {
            focusEditor()
        }
    }

    func closeTerminalSurface(_ clientSurfaceID: UInt) {
        guard let paneID = openItems.groups
            .flatMap(\.items)
            .first(where: { $0.kind == .terminal && $0.clientSurfaceID == clientSurfaceID })?
            .paneID
        else {
            return
        }
        guard EditorFFIBridge.closeOpenItem(handle?.raw, paneID: paneID, kind: .terminal, itemID: clientSurfaceID) else { return }
        refreshSnapshot()
    }

    func openTerminalInActivePane() {
        guard EditorFFIBridge.openTerminalInActivePane(handle?.raw) else { return }
        refreshSnapshot()
    }

    func openAgentInActivePane() {
        showSidebar(mode: .agent)
        agentSidebarCoordinator.store.startIfNeeded()
        agentSidebarCoordinator.store.requestComposerFocus()
    }

    func closeAgentPane(_ paneID: UInt? = nil) {
        if paneID == nil, fileTree.isVisible, sidebarMode == .agent {
            toggleFileTree()
            focusEditor()
            return
        }

        let targetItem: EditorPaneOpenItemRow?
        if let paneID {
            targetItem = openItems.groups.first(where: { $0.paneID == paneID })?.items.first(where: { $0.kind == .agent })
        } else {
            targetItem = openItems.groups.flatMap(\.items).first(where: { $0.kind == .agent })
        }
        guard let targetItem,
              EditorFFIBridge.closeOpenItem(handle?.raw, paneID: targetItem.paneID, kind: targetItem.kind, itemID: targetItem.itemID)
        else {
            return
        }
        refreshSnapshot()
        focusEditor()
    }

    func splitActivePaneVertical() {
        guard EditorFFIBridge.splitActivePaneVertical(handle?.raw) else { return }
        refreshSnapshot()
        if activeOpenItemKind != .terminal {
            focusEditor()
        }
    }

    func splitActivePaneHorizontal() {
        guard EditorFFIBridge.splitActivePaneHorizontal(handle?.raw) else { return }
        refreshSnapshot()
        if activeOpenItemKind != .terminal {
            focusEditor()
        }
    }

    var activeOpenItem: EditorPaneOpenItemRow? {
        openItems.groups.first(where: { $0.isActivePane })?
            .items.first(where: { $0.isActive })
    }

    /// Kind of the active tab in the active pane's open-items strip (buffer vs terminal).
    private var activeOpenItemKind: EditorOpenItemKind? {
        activeOpenItem?.kind
    }

    var activeTerminalStatus: EditorActiveTerminalStatus? {
        guard let item = activeOpenItem,
              item.kind == .terminal,
              let clientSurfaceID = item.clientSurfaceID
        else {
            return nil
        }

        let state = terminalPresentationBySurfaceID[clientSurfaceID]
        let workingDirectory = state?.workingDirectory.flatMap(formatTerminalDisplayPath)
        let title = normalizedTerminalStatusTitle(item.title, workingDirectory: workingDirectory)
        return EditorActiveTerminalStatus(title: title, workingDirectory: workingDirectory)
    }

    func closeActivePaneItem() {
        guard EditorFFIBridge.closeActivePaneItem(handle?.raw) else { return }
        refreshSnapshot()
        focusEditor()
    }

    func quitApplication() {
        NSApp.terminate(nil)
    }

    private func installCloseSurfaceKeyMonitor() {
        closeSurfaceEventMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self else {
                return event
            }
            if self.shouldHandleDismissInlineAssistShortcut(event) {
                self.dismissInlineAssist()
                return nil
            }
            if self.shouldHandleInlineAssistShortcut(event) {
                self.beginInlineAssist()
                return nil
            }
            if let paneLocalIndex = self.paneLocalOpenItemShortcutIndex(for: event),
               self.activatePaneLocalOpenItem(at: paneLocalIndex) {
                return nil
            }
            if self.shouldHandleCloseSurfaceShortcut(event) {
                self.closeActivePaneItem()
                return nil
            }
            if self.shouldHandleQuitShortcut(event) {
                self.quitApplication()
                return nil
            }
            return event
        }.map(EventMonitorBox.init(raw:))
    }

    private func paneLocalOpenItemShortcutIndex(for event: NSEvent) -> Int? {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        guard flags == [.command] else { return nil }
        guard let characters = event.charactersIgnoringModifiers, characters.count == 1 else { return nil }
        switch characters {
        case "1", "2", "3", "4", "5", "6", "7", "8", "9":
            return Int(characters)! - 1
        case "0":
            return 9
        default:
            return nil
        }
    }

    private func shouldHandleInlineAssistShortcut(_ event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        guard flags == [.control] else { return false }
        guard event.keyCode == 36 || event.keyCode == 76 else { return false }
        return inlineAssistState != nil || canStartInlineAssist
    }

    private func shouldHandleDismissInlineAssistShortcut(_ event: NSEvent) -> Bool {
        guard let inlineAssistState, inlineAssistState.phase == .editing else { return false }
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        guard flags.isEmpty else { return false }
        return event.keyCode == 53
    }

    private func shouldHandleCloseSurfaceShortcut(_ event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        guard flags == [.command] else { return false }
        guard event.charactersIgnoringModifiers?.lowercased() == "w" else { return false }
        return true
    }

    private func shouldHandleQuitShortcut(_ event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        guard flags == [.command] else { return false }
        guard event.charactersIgnoringModifiers?.lowercased() == "q" else { return false }
        return true
    }

    func closeTerminalInActivePane() {
        guard EditorFFIBridge.closeTerminalInActivePane(handle?.raw) else { return }
        refreshSnapshot()
    }

    func registerTerminalSurface(_ clientSurfaceID: UInt, preferredWorkingDirectory: String?) {
        objectWillChange.send()
        var state = terminalPresentationBySurfaceID[clientSurfaceID] ?? TerminalPresentationState()
        if state.workingDirectory?.isEmpty != false,
           let preferredWorkingDirectory,
           !preferredWorkingDirectory.isEmpty {
            state.workingDirectory = preferredWorkingDirectory
        }
        terminalPresentationBySurfaceID[clientSurfaceID] = state
        refreshOpenItemsDecoration()
    }

    func updateTerminalTitle(_ title: String, for clientSurfaceID: UInt) {
        objectWillChange.send()
        var state = terminalPresentationBySurfaceID[clientSurfaceID] ?? TerminalPresentationState()
        let trimmed = title.trimmingCharacters(in: .whitespacesAndNewlines)
        state.title = trimmed.isEmpty ? nil : trimmed
        terminalPresentationBySurfaceID[clientSurfaceID] = state
        refreshOpenItemsDecoration()
    }

    func updateTerminalWorkingDirectory(_ workingDirectory: String, for clientSurfaceID: UInt) {
        objectWillChange.send()
        var state = terminalPresentationBySurfaceID[clientSurfaceID] ?? TerminalPresentationState()
        let trimmed = workingDirectory.trimmingCharacters(in: .whitespacesAndNewlines)
        state.workingDirectory = trimmed.isEmpty ? nil : trimmed
        terminalPresentationBySurfaceID[clientSurfaceID] = state
        refreshOpenItemsDecoration()
    }

    func dragBufferSelection(
        paneID: UInt,
        dragOriginCol: Int,
        dragOriginRow: Int,
        logicalCol: Int,
        logicalRow: Int,
        modifiers: UInt8,
        clickCount: Int
    ) {
        selectionDebugLog(
            "controller.dragBufferSelection pane=\(paneID) origin=(\(dragOriginCol),\(dragOriginRow)) logical=(\(logicalCol),\(logicalRow)) modifiers=\(modifiers) clickCount=\(clickCount)"
        )
        guard EditorFFIBridge.dragBufferSelection(
            handle?.raw,
            paneID: paneID,
            dragOriginCol: dragOriginCol,
            dragOriginRow: dragOriginRow,
            logicalCol: logicalCol,
            logicalRow: logicalRow,
            modifiers: modifiers,
            clickCount: clickCount
        ) else { return }
        refreshSnapshot()
    }

    func scroll(byRows rowDelta: Int, cols colDelta: Int) {
        guard (rowDelta != 0 || colDelta != 0), let scene else { return }
        let started = CFAbsoluteTimeGetCurrent()
        logSceneSelectionState("scroll.before", scene: scene)
        let targetRow = UInt32(max(scene.info.scrollRow + rowDelta, 0))
        let targetCol = UInt32(max(scene.info.scrollCol + colDelta, 0))
        var changed = false
        if rowDelta != 0 {
            changed = EditorFFIBridge.setScrollRow(handle?.raw, row: targetRow) || changed
        }
        if colDelta != 0 {
            changed = EditorFFIBridge.setScrollCol(handle?.raw, col: targetCol) || changed
        }
        if changed {
            refreshSnapshot()
        }
        let totalMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
        scrollPerfLog(
            "controller.scroll rowDelta=\(rowDelta) colDelta=\(colDelta) targetRow=\(targetRow) targetCol=\(targetCol) changed=\(changed) totalMs=\(String(format: "%.2f", totalMs))"
        )
    }

    func scrollRows(by delta: Int) {
        scroll(byRows: delta, cols: 0)
    }

    func shouldHandleEditorKeyboardInput(from responder: NSView? = nil) -> Bool {
        guard activeOpenItemKind != .terminal, activeOpenItemKind != .agent else { return false }
        guard let editorFirstResponder else { return false }
        if let responder, responder !== editorFirstResponder {
            return false
        }
        return editorFirstResponder.window?.firstResponder === editorFirstResponder
    }

    func handleKey(_ event: the_editor_key_event_t) {
        guard shouldHandleEditorKeyboardInput() else { return }
        if event.kind == THE_EDITOR_KEY_ESCAPE.rawValue, hoverDocs.isOpen || signatureHelp.isOpen {
            closeDocsPanels()
            markedText = ""
            return
        }
        guard EditorFFIBridge.sendKey(handle?.raw, event: event) else { return }
        if event.kind == THE_EDITOR_KEY_ESCAPE.rawValue {
            markedText = ""
        }
        refreshSnapshot()
    }

    func acceptInlineCompletion() {
        guard inlineCompletion.visible else { return }
        handleKey(the_editor_key_event_t(kind: THE_EDITOR_KEY_TAB.rawValue, codepoint: 0, modifiers: 0))
    }

    func dismissInlineCompletion() {
        guard inlineCompletion.visible else { return }
        handleKey(the_editor_key_event_t(kind: THE_EDITOR_KEY_ESCAPE.rawValue, codepoint: 0, modifiers: 0))
    }

    func toggleCommandPalette() {
        commandPaletteDebugLog("toggle before isOpen=\(commandPalette.isOpen) query=\(String(reflecting: commandPalette.query))")
        guard EditorFFIBridge.toggleCommandPalette(handle?.raw) else { return }
        refreshSnapshot()
    }

    func closeCommandPalette() {
        guard EditorFFIBridge.closeCommandPalette(handle?.raw) else { return }
        refreshSnapshot()
    }

    func openSearchPrompt() {
        guard !inputPrompt.isOpen || inputPrompt.kind != .search else { return }
        guard EditorFFIBridge.openSearchPrompt(handle?.raw) else { return }
        refreshSnapshot()
    }

    func closeInputPrompt() {
        guard EditorFFIBridge.closeInputPrompt(handle?.raw) else { return }
        refreshSnapshot()
        focusEditor()
    }

    func closeCompletionMenu() {
        guard EditorFFIBridge.closeCompletionMenu(handle?.raw) else { return }
        refreshSnapshot()
        focusEditor()
    }

    func selectCompletionMenuIndex(_ index: Int) {
        guard EditorFFIBridge.selectCompletionMenuIndex(handle?.raw, index: index) else { return }
        refreshSnapshot()
    }

    func setCompletionMenuScroll(_ offset: Int) {
        guard EditorFFIBridge.setCompletionMenuScroll(handle?.raw, offset: offset) else { return }
        refreshSnapshot()
    }

    func submitCompletionMenu() {
        guard EditorFFIBridge.submitCompletionMenu(handle?.raw) else { return }
        refreshSnapshot()
        focusEditor()
    }

    /// Moves the completion selection like Tab / Shift+Tab (wraps at ends).
    func stepCompletionMenuSelection(forward: Bool) {
        guard completionMenu.isOpen, !completionMenu.items.isEmpty else { return }
        let count = completionMenu.items.count
        let current = completionMenu.selectedIndex ?? 0
        let next: Int
        if forward {
            next = (current + 1) % count
        } else {
            next = (current + count - 1) % count
        }
        selectCompletionMenuIndex(next)
    }

    func closeDocsPanels() {
        guard EditorFFIBridge.closeDocsPanels(handle?.raw) else { return }
        refreshSnapshot()
        focusEditor()
    }

    func setInputPromptQuery(_ query: String) {
        guard query != inputPrompt.query else { return }
        guard EditorFFIBridge.setInputPromptQuery(handle?.raw, query: query) else { return }
        refreshSnapshot()
    }

    func submitInputPrompt() {
        guard EditorFFIBridge.submitInputPrompt(handle?.raw) else { return }
        refreshSnapshot()
        if !inputPrompt.isOpen {
            focusEditor()
        }
    }

    func stepInputPromptNext() {
        guard EditorFFIBridge.stepNextInputPrompt(handle?.raw) else { return }
        refreshSnapshot()
    }

    func stepInputPromptPrevious() {
        guard EditorFFIBridge.stepPreviousInputPrompt(handle?.raw) else { return }
        refreshSnapshot()
    }

    func configureFilePicker(listVisibleRows: Int, previewVisibleRows: Int) {
        filePickerListVisibleRows = max(listVisibleRows, 1)
        filePickerPreviewVisibleRows = max(previewVisibleRows, 1)
        guard EditorFFIBridge.configureFilePicker(handle?.raw, listVisibleRows: filePickerListVisibleRows, previewVisibleRows: filePickerPreviewVisibleRows) else {
            return
        }
        refreshSnapshot()
    }

    func closeFilePicker() {
        guard EditorFFIBridge.closeFilePicker(handle?.raw) else { return }
        refreshSnapshot()
    }

    func setFilePickerQuery(_ query: String) {
        guard query != filePicker.query else { return }
        guard EditorFFIBridge.setFilePickerQuery(handle?.raw, query: query) else { return }
        refreshSnapshot()
    }

    func cycleFilePickerSearchMode() {
        guard EditorFFIBridge.cycleFilePickerSearchMode(handle?.raw) else { return }
        refreshSnapshot()
    }

    func moveFilePickerSelection(_ direction: MoveCommandDirection) {
        let changed: Bool
        switch direction {
        case .up:
            changed = EditorFFIBridge.selectPreviousFilePickerItem(handle?.raw)
        case .down:
            changed = EditorFFIBridge.selectNextFilePickerItem(handle?.raw)
        case .left, .right:
            changed = false
        @unknown default:
            changed = false
        }
        guard changed else { return }
        refreshSnapshot()
    }

    func setFilePickerListOffset(_ offset: Int) {
        let normalized = max(offset, 0)
        guard normalized != filePicker.visibleItemStart else { return }
        guard EditorFFIBridge.setFilePickerListOffset(handle?.raw, offset: normalized) else { return }
        refreshSnapshot()
    }

    func setFilePickerPreviewOffset(_ offset: Int) {
        let normalized = max(offset, 0)
        guard normalized != filePicker.previewOffset else { return }
        guard EditorFFIBridge.setFilePickerPreviewOffset(handle?.raw, offset: normalized, visibleRows: filePickerPreviewVisibleRows) else { return }
        refreshSnapshot()
    }

    func selectFilePickerIndex(_ index: Int) {
        guard EditorFFIBridge.selectFilePickerIndex(handle?.raw, index: index) else { return }
        refreshSnapshot()
    }

    func submitFilePicker() {
        guard EditorFFIBridge.submitFilePicker(handle?.raw) else { return }
        refreshSnapshot()
    }

    func submitFilePicker(index: Int) {
        guard EditorFFIBridge.selectFilePickerIndex(handle?.raw, index: index) else { return }
        guard EditorFFIBridge.submitFilePicker(handle?.raw) else { return }
        refreshSnapshot()
    }

    func selectFileTreeIndex(_ index: Int) {
        scrollPerfLog(
            "controller.fileTreeSelect requested index=\(index) selected=\(String(describing: fileTree.selectedIndex)) scroll=\(fileTree.scrollOffset) visibleRows=\(fileTreeVisibleRows) rows=\(fileTree.rows.count)"
        )
        guard EditorFFIBridge.selectFileTreeIndex(handle?.raw, index: index) else { return }
        refreshSnapshot()
    }

    func clickFileTreeIndex(_ index: Int) {
        scrollPerfLog(
            "controller.fileTreeClick requested index=\(index) selected=\(String(describing: fileTree.selectedIndex)) scroll=\(fileTree.scrollOffset) visibleRows=\(fileTreeVisibleRows) rows=\(fileTree.rows.count)"
        )
        guard EditorFFIBridge.clickFileTreeIndex(handle?.raw, index: index) else { return }
        refreshSnapshot()
    }

    func activateFileTreeIndex(_ index: Int) {
        scrollPerfLog(
            "controller.fileTreeActivate requested index=\(index) selected=\(String(describing: fileTree.selectedIndex)) scroll=\(fileTree.scrollOffset) visibleRows=\(fileTreeVisibleRows) rows=\(fileTree.rows.count)"
        )
        guard EditorFFIBridge.activateFileTreeIndex(handle?.raw, index: index) else { return }
        refreshSnapshot()
    }

    func setFileTreeVisibleRows(_ rows: Int) {
        let clampedRows = max(rows, 1)
        guard clampedRows != fileTreeVisibleRows else { return }
        scrollPerfLog(
            "controller.fileTreeVisibleRows requested previous=\(fileTreeVisibleRows) next=\(clampedRows) selected=\(String(describing: fileTree.selectedIndex)) scroll=\(fileTree.scrollOffset) rows=\(fileTree.rows.count)"
        )
        guard EditorFFIBridge.setFileTreeVisibleRows(handle?.raw, visibleRows: clampedRows) else { return }
        fileTreeVisibleRows = clampedRows
        refreshSnapshot()
    }

    func syncFileTreeScrollOffset(_ offset: Int) {
        let clampedOffset = max(offset, 0)
        guard clampedOffset != fileTree.scrollOffset else { return }
        scrollPerfLog(
            "controller.fileTreeScrollSync requested previous=\(fileTree.scrollOffset) next=\(clampedOffset) selected=\(String(describing: fileTree.selectedIndex)) rows=\(fileTree.rows.count)"
        )
        _ = EditorFFIBridge.setFileTreeScrollOffset(handle?.raw, scrollOffset: clampedOffset)
    }

    func setFileTreeActive(_ active: Bool) {
        scrollPerfLog(
            "controller.fileTreeActive requested next=\(active) selected=\(String(describing: fileTree.selectedIndex)) scroll=\(fileTree.scrollOffset) rows=\(fileTree.rows.count)"
        )
        guard EditorFFIBridge.setFileTreeActive(handle?.raw, active: active) else { return }
        refreshSnapshot()
    }

    func toggleFileTree() {
        let targetVisibility = !fileTree.isVisible
        let started = CFAbsoluteTimeGetCurrent()
        scrollPerfLog(
            "fileTree.toggle requested visible=\(fileTree.isVisible)→\(targetVisibility) rows=\(fileTree.rows.count) selected=\(String(describing: fileTree.selectedIndex)) scroll=\(fileTree.scrollOffset)"
        )
        let ffiStarted = CFAbsoluteTimeGetCurrent()
        guard EditorFFIBridge.toggleFileTree(handle?.raw) else {
            return
        }
        let ffiMs = (CFAbsoluteTimeGetCurrent() - ffiStarted) * 1000
        let refreshStarted = CFAbsoluteTimeGetCurrent()
        refreshSnapshot()
        let refreshMs = (CFAbsoluteTimeGetCurrent() - refreshStarted) * 1000
        let totalMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
        let ffiMsText = String(format: "%.2f", ffiMs)
        let refreshMsText = String(format: "%.2f", refreshMs)
        let totalMsText = String(format: "%.2f", totalMs)
        scrollPerfLog(
            "fileTree.toggle completed ffiMs=\(ffiMsText) refreshMs=\(refreshMsText) totalMs=\(totalMsText) visibleNow=\(fileTree.isVisible) rowsNow=\(fileTree.rows.count)"
        )
    }

    func setSidebarMode(_ mode: EditorSidebarMode) {
        guard sidebarMode != mode else {
            setFileTreeActive(mode == .files)
            return
        }
        sidebarMode = mode
        UserDefaults.standard.set(mode.rawValue, forKey: Self.sidebarModeDefaultsKey)
        setFileTreeActive(mode == .files)
    }

    func showSidebar(mode: EditorSidebarMode) {
        setSidebarMode(mode)
        if !fileTree.isVisible {
            toggleFileTree()
        }
    }

    func activateAgentUIIfNeeded(agentItemID: UInt) {
        _ = agentItemID
        showSidebar(mode: .agent)
        setFileTreeActive(false)
    }

    func setCommandPaletteQuery(_ query: String) {
        commandPaletteDebugLog("setQuery incoming=\(String(reflecting: query)) current=\(String(reflecting: commandPalette.query))")
        guard query != commandPalette.query else { return }
        guard EditorFFIBridge.setCommandPaletteQuery(handle?.raw, query: query) else { return }
        refreshSnapshot()
    }

    func moveCommandPaletteSelection(_ direction: MoveCommandDirection) {
        let changed: Bool
        switch direction {
        case .up:
            changed = EditorFFIBridge.selectPreviousCommandPaletteItem(handle?.raw)
        case .down:
            changed = EditorFFIBridge.selectNextCommandPaletteItem(handle?.raw)
        case .left, .right:
            changed = false
        @unknown default:
            changed = false
        }
        guard changed else { return }
        refreshSnapshot()
    }

    func submitCommandPalette() {
        commandPaletteDebugLog("submit query=\(String(reflecting: commandPalette.query)) selected=\(String(describing: commandPalette.selectedIndex)) items=\(commandPalette.items.count)")
        guard EditorFFIBridge.submitCommandPalette(handle?.raw) else { return }
        refreshSnapshot()
    }

    func submitCommandPalette(visibleIndex: Int) {
        guard EditorFFIBridge.selectCommandPaletteVisibleIndex(handle?.raw, index: visibleIndex) else { return }
        guard EditorFFIBridge.submitCommandPalette(handle?.raw) else { return }
        refreshSnapshot()
    }

    func focusEditor() {
        guard let editorFirstResponder else { return }
        editorFirstResponder.window?.makeFirstResponder(editorFirstResponder)
    }

    func insertText(_ text: String) {
        guard shouldHandleEditorKeyboardInput() else { return }
        guard !text.isEmpty else { return }
        guard EditorFFIBridge.insertText(handle?.raw, text: text) else { return }
        markedText = ""
        refreshSnapshot()
    }

    func updateMarkedText(_ text: String) {
        markedText = text
        refreshSnapshot()
    }

    func clearMarkedText() {
        if markedText.isEmpty { return }
        markedText = ""
        refreshSnapshot()
    }

    func primarySelectionUTF16Range() -> NSRange {
        EditorFFIBridge.primarySelectionUTF16Range(handle?.raw)
    }

    func primarySelectionText() -> String {
        EditorFFIBridge.primarySelectionText(handle?.raw)
    }

    func primarySelectionLineRange() -> ClosedRange<Int>? {
        EditorFFIBridge.primarySelectionLineRange(handle?.raw)
    }

    var primarySelectionDisplayRect: CGRect? {
        guard let scene,
              let activeOpenItem,
              activeOpenItem.kind == .buffer
        else {
            return nil
        }

        let rects = scene.selections.compactMap { selection -> CGRect? in
            guard selection.kind == .primary,
                  let pane = scene.paneContainingCell(col: selection.x, row: selection.y),
                  pane.paneID == activeOpenItem.paneID
            else {
                return nil
            }
            let rect = scene.displayRect(
                x: selection.x,
                y: selection.y,
                width: selection.width,
                height: selection.height,
                paneID: pane.paneID
            )
            let clippedRect = rect.intersection(scene.paneContentRect(for: pane))
            guard clippedRect.width > 0, clippedRect.height > 0 else {
                return nil
            }
            return clippedRect
        }

        guard let firstRect = rects.first else { return nil }
        return rects.dropFirst().reduce(firstRect) { partialResult, rect in
            partialResult.union(rect)
        }
    }

    var inlineAssistPaneContentRect: CGRect? {
        guard let scene,
              let activeOpenItem,
              activeOpenItem.kind == .buffer,
              let pane = scene.pane(id: activeOpenItem.paneID)
        else {
            return nil
        }
        return scene.paneContentRect(for: pane)
    }

    var canStartInlineAssist: Bool {
        guard activeOpenItem?.kind == .buffer,
              !commandPalette.isOpen,
              !filePicker.isOpen,
              !inputPrompt.isOpen,
              !completionMenu.isOpen
        else {
            return false
        }
        let selectionRange = primarySelectionUTF16Range()
        return selectionRange.length > 0 && primarySelectionDisplayRect != nil
    }

    func beginInlineAssist(prefilledPrompt: String? = nil) {
        if let inlineAssistState {
            agentDebugLog("inlineAssist.focusExisting phase=\(String(describing: inlineAssistState.phase))")
            if inlineAssistState.phase == .editing {
                focusInlineAssistPrompt()
            }
            return
        }
        guard canStartInlineAssist,
              let selectionContext = currentAgentSelectionContext()
        else {
            NSSound.beep()
            return
        }

        inlineAssistTask?.cancel()
        inlineAssistRequestGeneration &+= 1
        agentDebugLog("inlineAssist.begin generation=\(inlineAssistRequestGeneration) file=\(selectionContext.filePath ?? "-") lines=\(selectionContext.lineRange.map { "\($0.lowerBound)-\($0.upperBound)" } ?? "-") selectionChars=\(selectionContext.text.count) prefilledPromptChars=\((prefilledPrompt ?? "").count)")
        inlineAssistState = EditorInlineAssistState(
            selectionRange: selectionContext.utf16Range,
            selectionCharRange: selectionContext.charRange,
            selectionText: selectionContext.text,
            filePath: selectionContext.filePath,
            sourceLabel: selectionContext.sourceLabel,
            lineRange: selectionContext.lineRange,
            language: selectionContext.language,
            prompt: prefilledPrompt ?? ""
        )
        loadInlineAssistModelsIfNeeded(preferred: agentSidebarCoordinator.preferredInlineModelSelection())
    }

    func dismissInlineAssist() {
        agentDebugLog("inlineAssist.dismiss state=\(inlineAssistState.map { String(describing: $0.phase) } ?? "nil") generation=\(inlineAssistRequestGeneration)")
        inlineAssistTask?.cancel()
        inlineAssistTask = nil
        inlineAssistModelsTask?.cancel()
        inlineAssistModelsTask = nil
        inlineAssistRequestGeneration &+= 1
        inlineAssistState = nil
        inlineAssistModels = []
        inlineAssistSelectedModel = nil
        focusEditor()
    }

    func updateInlineAssistPrompt(_ prompt: String) {
        guard var inlineAssistState else { return }
        guard inlineAssistState.prompt != prompt else { return }
        inlineAssistState.prompt = prompt
        inlineAssistState.errorMessage = nil
        self.inlineAssistState = inlineAssistState
    }

    func retryInlineAssist() {
        submitInlineAssist()
    }

    func setInlineAssistModel(provider: String, modelID: String) {
        agentDebugLog("inlineAssist.model provider=\(provider) model=\(modelID)")
        inlineAssistSelectedModel = EditorInlineAssistModelSelection(provider: provider, modelID: modelID)
        normalizeInlineAssistThinkingLevel()
    }

    func setInlineAssistThinkingLevel(_ level: String) {
        let availableLevels = availableInlineAssistThinkingLevels()
        guard availableLevels.contains(level) else { return }
        inlineAssistThinkingLevel = level
    }

    func submitInlineAssist() {
        guard var inlineAssistState else { return }
        let prompt = inlineAssistState.prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !prompt.isEmpty else {
            inlineAssistState.errorMessage = "Enter a rewrite instruction first."
            self.inlineAssistState = inlineAssistState
            return
        }
        guard let selectionContext = validatedInlineAssistSelectionContext(for: inlineAssistState) else {
            inlineAssistState.errorMessage = "The selection changed. Reopen inline assist on the text you want to rewrite."
            inlineAssistState.phase = .editing
            self.inlineAssistState = inlineAssistState
            return
        }
        guard inlineAssistSelectedModel != nil else {
            inlineAssistState.errorMessage = inlineAssistModels.isEmpty
                ? "No inline rewrite models are available."
                : "Choose a model for inline rewrite."
            inlineAssistState.phase = .editing
            self.inlineAssistState = inlineAssistState
            return
        }

        inlineAssistState.phase = .generating
        inlineAssistState.errorMessage = nil
        self.inlineAssistState = inlineAssistState
        focusEditor()

        inlineAssistTask?.cancel()
        inlineAssistRequestGeneration &+= 1
        let requestGeneration = inlineAssistRequestGeneration
        let selectedModel = inlineAssistSelectedModel
        let modelSourceSessionPath = selectedModel.flatMap {
            self.agentSidebarCoordinator.sessionPathForInlineModel(provider: $0.provider, modelID: $0.modelID)
        }
        agentDebugLog("inlineAssist.submit.begin generation=\(requestGeneration) stateID=\(inlineAssistState.id.uuidString) file=\(selectionContext.filePath ?? "-") lines=\(selectionContext.lineRange.map { "\($0.lowerBound)-\($0.upperBound)" } ?? "-") selectionChars=\(selectionContext.text.count) promptChars=\(prompt.count) model=\(selectedModel.map { "\($0.provider)/\($0.modelID)" } ?? "nil") modelSourceSessionPath=\(modelSourceSessionPath ?? "nil")")
        Task { [weak self] in
            try? await Task.sleep(for: .seconds(15))
            await MainActor.run {
                guard let self,
                      self.inlineAssistRequestGeneration == requestGeneration,
                      let currentState = self.inlineAssistState,
                      currentState.id == inlineAssistState.id,
                      currentState.phase == .generating
                else {
                    return
                }
                agentDebugLog("inlineAssist.submit.stillRunning generation=\(requestGeneration) stateID=\(currentState.id.uuidString) promptChars=\(currentState.prompt.count) selectedModel=\(self.inlineAssistSelectedModel.map { "\($0.provider)/\($0.modelID)" } ?? "nil")")
            }
        }
        inlineAssistTask = Task { [weak self] in
            guard let self else { return }
            do {
                let response = try await self.agentSessionSupervisor.inlineRewrite(
                    cwd: self.editorWorkingDirectory,
                    filePath: selectionContext.filePath,
                    sourceLabel: selectionContext.sourceLabel,
                    lineStart: selectionContext.lineRange?.lowerBound,
                    lineEnd: selectionContext.lineRange?.upperBound,
                    language: selectionContext.language,
                    selectionText: selectionContext.text,
                    prompt: prompt,
                    provider: selectedModel?.provider,
                    modelID: selectedModel?.modelID,
                    modelSourceSessionPath: modelSourceSessionPath,
                    thinkingLevel: inlineAssistThinkingLevel
                )
                guard !Task.isCancelled else { return }
                let rewrittenText = (response["text"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
                await MainActor.run {
                    guard self.inlineAssistRequestGeneration == requestGeneration,
                          var currentState = self.inlineAssistState,
                          currentState.id == inlineAssistState.id
                    else {
                        return
                    }
                    if rewrittenText.isEmpty {
                        currentState.phase = .editing
                        currentState.errorMessage = "Inline rewrite returned no text."
                        agentDebugLog("inlineAssist.submit.empty generation=\(requestGeneration) stateID=\(currentState.id.uuidString)")
                        self.inlineAssistState = currentState
                    } else if self.applyInlineAssistRewriteDirect(rewrittenText, for: currentState) {
                        agentDebugLog("inlineAssist.submit.success generation=\(requestGeneration) stateID=\(currentState.id.uuidString) resultChars=\(rewrittenText.count)")
                        self.dismissInlineAssist()
                    } else {
                        currentState.phase = .editing
                        currentState.errorMessage = "Couldn’t apply inline rewrite. Generate again."
                        agentDebugLog("inlineAssist.submit.applyFailed generation=\(requestGeneration) stateID=\(currentState.id.uuidString) resultChars=\(rewrittenText.count)")
                        self.inlineAssistState = currentState
                    }
                    self.inlineAssistTask = nil
                }
            } catch {
                if Task.isCancelled {
                    agentDebugLog("inlineAssist.submit.cancelled generation=\(requestGeneration)")
                    return
                }
                await MainActor.run {
                    guard self.inlineAssistRequestGeneration == requestGeneration,
                          var currentState = self.inlineAssistState,
                          currentState.id == inlineAssistState.id
                    else {
                        return
                    }
                    currentState.phase = .editing
                    currentState.errorMessage = error.localizedDescription
                    agentDebugLog("inlineAssist.submit.error generation=\(requestGeneration) stateID=\(currentState.id.uuidString) error=\(error.localizedDescription)")
                    self.inlineAssistState = currentState
                    self.inlineAssistTask = nil
                }
            }
        }
    }

    func addPrimarySelectionToAgent() {
        guard let selectionContext = currentAgentSelectionContext() else {
            return
        }

        let store = agentSidebarCoordinator.storeForSelectionRouting()
        showSidebar(mode: .agent)
        store.startIfNeeded()
        store.activateAgentSurfaceIfNeeded()
        store.appendSelectionAttachment(
            EditorAgentSelectionAttachment(
                sourceLabel: selectionContext.sourceLabel,
                lineRange: selectionContext.lineRange,
                text: selectionContext.text,
                language: selectionContext.language
            )
        )
    }

    func renderMarkdown(_ markdown: String) -> EditorRenderedMarkdown {
        EditorFFIBridge.renderMarkdown(handle?.raw, markdown: markdown)
    }

    func beginLiveResize() {
        sidebarPerfIncrement("window.liveResize.begin")
        beginInteractiveResize(reason: "window")
        resizeOverlayHideTask?.cancel()
        if !showsResizeOverlay {
            showsResizeOverlay = true
        }
    }

    func endLiveResize() {
        sidebarPerfIncrement("window.liveResize.end")
        endInteractiveResize(reason: "window")
        resizeOverlayHideTask?.cancel()
        resizeOverlayHideTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(650))
            guard let self else { return }
            self.showsResizeOverlay = false
        }
    }

    func refreshSnapshot() {
        sidebarPerfIncrement("controller.refreshSnapshot.request")
        let wasInputPromptOpen = inputPrompt.isOpen
        let started = CFAbsoluteTimeGetCurrent()
        guard var snapshot = EditorFFIBridge.makeSnapshot(handle?.raw) else { return }
        if purgeLegacyAgentOpenItemsIfNeeded(snapshot.openItems), let refreshedSnapshot = EditorFFIBridge.makeSnapshot(handle?.raw) {
            snapshot = refreshedSnapshot
        }
        let shouldQuitApplication = EditorFFIBridge.takeQuitRequested(handle?.raw)
        let shouldAddSelectionToAgent = EditorFFIBridge.takeAddSelectionToAgentRequested(handle?.raw)
        let snapshotMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
        let nextChrome = EditorChromeModel(
            document: snapshot.document,
            statusBar: snapshot.statusBar,
            backgroundColor: snapshot.info.backgroundColor?.color ?? .windowBackgroundColor
        )
        logSelectionSnapshot("refresh", snapshot: snapshot)
        commandPaletteDebugLog("refresh query=\(String(reflecting: snapshot.commandPalette.query)) selected=\(String(describing: snapshot.commandPalette.selectedIndex)) items=\(snapshot.commandPalette.items.count) isOpen=\(snapshot.commandPalette.isOpen)")
        let sceneStarted = CFAbsoluteTimeGetCurrent()
        let marked = markedTextOverlay(from: snapshot)
        let nextScene = EditorRenderScene.from(snapshot: snapshot, markedText: marked)
        let sceneMs = (CFAbsoluteTimeGetCurrent() - sceneStarted) * 1000

        let publishStarted = CFAbsoluteTimeGetCurrent()
        objectWillChange.send()
        currentMode = snapshot.info.mode
        chrome = nextChrome
        pendingKeys = snapshot.pendingKeys
        commandPalette = snapshot.commandPalette
        completionMenu = snapshot.completionMenu
        inlineCompletion = snapshot.inlineCompletion
        inputPrompt = snapshot.inputPrompt
        hoverDocs = snapshot.hoverDocs
        completionDocs = snapshot.completionDocs
        signatureHelp = snapshot.signatureHelp
        filePicker = snapshot.filePicker
        bufferTabs = snapshot.bufferTabs
        baseOpenItems = snapshot.openItems
        pruneTerminalPresentation(validSurfaceIDs: Set(snapshot.openItems.groups.flatMap { group in
            group.items.compactMap { item in
                guard item.kind == .terminal else { return nil }
                return item.clientSurfaceID
            }
        }))
        openItems = decorateOpenItems(snapshot.openItems)
        let previousFileTree = fileTree
        fileTree = snapshot.fileTree
        let decoratedFileTreeRows = snapshot.fileTree.rows.filter { $0.vcsKind != nil || $0.diagnosticSeverity != nil }.count
        let currentFileTreeSummary = snapshot.fileTree.rows.first(where: { $0.isCurrentFile }).map {
            "current=\($0.path) vcs=\($0.vcsKind?.rawValue ?? 0) diag=\($0.diagnosticSeverity?.rawValue ?? 0)"
        } ?? "current=-"
        scrollPerfLog(
            "controller.fileTree visible=\(snapshot.fileTree.isVisible) rows=\(snapshot.fileTree.rows.count) selected=\(String(describing: snapshot.fileTree.selectedIndex)) scroll=\(snapshot.fileTree.scrollOffset) visibleRows=\(fileTreeVisibleRows) decoratedRows=\(decoratedFileTreeRows) \(currentFileTreeSummary)"
        )
        if previousFileTree.isVisible != snapshot.fileTree.isVisible || previousFileTree.rows.count != snapshot.fileTree.rows.count {
            scrollPerfLog(
                "controller.fileTree transition visible=\(previousFileTree.isVisible)→\(snapshot.fileTree.isVisible) rows=\(previousFileTree.rows.count)→\(snapshot.fileTree.rows.count) selected=\(String(describing: snapshot.fileTree.selectedIndex)) scroll=\(snapshot.fileTree.scrollOffset)"
            )
        }
        scene = nextScene
        let publishMs = (CFAbsoluteTimeGetCurrent() - publishStarted) * 1000

        let delegateStarted = CFAbsoluteTimeGetCurrent()
        delegate?.editorController(self, didUpdateScene: nextScene)
        let delegateMs = (CFAbsoluteTimeGetCurrent() - delegateStarted) * 1000
        if wasInputPromptOpen && !snapshot.inputPrompt.isOpen {
            DispatchQueue.main.async { [weak self] in
                self?.focusEditor()
            }
        }
        validateInlineAssistAfterSnapshot()
        let totalMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
        sidebarPerfRecordDuration("controller.refreshSnapshot.total.ms", ms: totalMs)
        sidebarPerfRecordDuration("controller.refreshSnapshot.snapshot.ms", ms: snapshotMs)
        sidebarPerfRecordDuration("controller.refreshSnapshot.scene.ms", ms: sceneMs)
        sidebarPerfRecordDuration("controller.refreshSnapshot.publish.ms", ms: publishMs)
        sidebarPerfRecordDuration("controller.refreshSnapshot.delegate.ms", ms: delegateMs)
        themePerfLog(
            "controller.refresh themeGen=\(snapshot.info.themeGeneration) snapshotMs=\(String(format: "%.2f", snapshotMs)) sceneMs=\(String(format: "%.2f", sceneMs)) totalMs=\(String(format: "%.2f", totalMs))"
        )
        scrollPerfLog(
            "controller.refresh damage=\(snapshot.info.damageReason) full=\(snapshot.info.damageIsFull) scrollGen=\(snapshot.info.scrollGeneration) textGen=\(snapshot.info.textGeneration) decoGen=\(snapshot.info.decorationGeneration) themeGen=\(snapshot.info.themeGeneration) lines=\(snapshot.lines.count) cursors=\(snapshot.cursors.count) overlays=\(snapshot.overlays.count) snapshotMs=\(String(format: "%.2f", snapshotMs)) sceneMs=\(String(format: "%.2f", sceneMs)) publishMs=\(String(format: "%.2f", publishMs)) delegateMs=\(String(format: "%.2f", delegateMs)) totalMs=\(String(format: "%.2f", totalMs))"
        )
        if snapshot.completionMenu.isOpen || snapshot.completionDocs.isOpen {
            completionPerfLog(
                "controller.refresh menuOpen=\(snapshot.completionMenu.isOpen) docsOpen=\(snapshot.completionDocs.isOpen) items=\(snapshot.completionMenu.items.count) selected=\(String(describing: snapshot.completionMenu.selectedIndex)) scroll=\(snapshot.completionMenu.scrollOffset) snapshotMs=\(String(format: "%.2f", snapshotMs)) sceneMs=\(String(format: "%.2f", sceneMs)) totalMs=\(String(format: "%.2f", totalMs))"
            )
        }
        if shouldAddSelectionToAgent {
            DispatchQueue.main.async { [weak self] in
                self?.addPrimarySelectionToAgent()
            }
        }
        if shouldQuitApplication {
            quitApplication()
        }
    }

    var editorWorkingDirectory: String {
        if let root = fileTree.root, !root.isEmpty {
            return root
        }
        if let absolutePath = chrome.document.absolutePath, !absolutePath.isEmpty {
            return URL(fileURLWithPath: absolutePath).deletingLastPathComponent().path
        }
        return FileManager.default.currentDirectoryPath
    }

    private func currentAgentSelectionContext() -> EditorAgentSelectionContext? {
        guard activeOpenItem?.kind == .buffer else { return nil }
        let selectionRange = primarySelectionUTF16Range()
        guard selectionRange.length > 0 else { return nil }
        guard let selectionCharRange = EditorFFIBridge.primarySelectionCharRange(handle?.raw),
              !selectionCharRange.isEmpty else { return nil }

        let text = primarySelectionText()
        guard !text.isEmpty else { return nil }

        let document = chrome.document
        let absolutePath = document.absolutePath
        let sourceLabel = document.relativePath ?? absolutePath ?? document.name
        let path = absolutePath ?? document.relativePath
        return EditorAgentSelectionContext(
            text: text,
            sourceLabel: sourceLabel,
            filePath: absolutePath,
            utf16Range: selectionRange,
            charRange: selectionCharRange,
            lineRange: primarySelectionLineRange(),
            language: selectionFenceLanguage(for: path, fallbackLanguageName: document.languageName)
        )
    }

    private func validatedInlineAssistSelectionContext(for state: EditorInlineAssistState) -> EditorAgentSelectionContext? {
        guard let selectionContext = currentAgentSelectionContext() else { return nil }
        guard selectionContext.utf16Range == state.selectionRange,
              selectionContext.charRange == state.selectionCharRange,
              selectionContext.text == state.selectionText,
              selectionContext.filePath == state.filePath
        else {
            return nil
        }
        return selectionContext
    }

    private func applyInlineAssistRewriteDirect(_ text: String, for state: EditorInlineAssistState) -> Bool {
        guard EditorFFIBridge.replaceTextInCharRange(
            handle?.raw,
            filePath: state.filePath,
            start: state.selectionCharRange.lowerBound,
            end: state.selectionCharRange.upperBound,
            expectedText: state.selectionText,
            text: text
        ) else {
            return false
        }
        markedText = ""
        refreshSnapshot()
        return true
    }

    private func selectionFenceLanguage(for path: String?, fallbackLanguageName: String?) -> String? {
        if let path {
            let ext = URL(fileURLWithPath: path).pathExtension.lowercased()
            if !ext.isEmpty {
                return ext
            }
        }
        guard let fallbackLanguageName else { return nil }
        let normalized = fallbackLanguageName
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
            .replacingOccurrences(of: " ", with: "-")
        return normalized.isEmpty ? nil : normalized
    }

    private func validateInlineAssistAfterSnapshot() {
        guard let inlineAssistState else { return }
        if inlineAssistState.phase == .generating {
            guard activeOpenItem?.kind == .buffer else { return }
            return
        }
        guard activeOpenItem?.kind == .buffer,
              validatedInlineAssistSelectionContext(for: inlineAssistState) != nil,
              primarySelectionDisplayRect != nil
        else {
            dismissInlineAssist()
            return
        }
    }

    private func focusInlineAssistPrompt() {
        inlineAssistFocusRequestToken &+= 1
    }

    private func loadInlineAssistModelsIfNeeded(preferred: (provider: String, modelID: String)?) {
        let registryModels = agentSidebarCoordinator.availableInlineModels()
        agentDebugLog("inlineAssist.models.begin registryCount=\(registryModels.count) preferred=\(preferred.map { "\($0.provider)/\($0.modelID)" } ?? "nil")")
        if !registryModels.isEmpty {
            applyInlineAssistModels(registryModels, preferred: preferred)
        }

        inlineAssistModelsTask?.cancel()
        inlineAssistModelsTask = Task { [weak self] in
            guard let self else { return }
            do {
                let response = try await self.agentSessionSupervisor.listInlineRewriteModels(cwd: self.editorWorkingDirectory)
                guard !Task.isCancelled else { return }
                let helperModels = EditorAgentPanelStore.parseModels(response["result"])
                await MainActor.run {
                    guard self.inlineAssistState != nil else { return }
                    let mergedModels = self.mergeInlineAssistModels(primary: self.agentSidebarCoordinator.availableInlineModels(), secondary: helperModels)
                    agentDebugLog("inlineAssist.models.loaded helperCount=\(helperModels.count) mergedCount=\(mergedModels.count)")
                    self.applyInlineAssistModels(mergedModels, preferred: preferred)
                    self.focusInlineAssistPrompt()
                }
            } catch {
                guard !Task.isCancelled else { return }
                await MainActor.run {
                    guard var inlineAssistState = self.inlineAssistState else { return }
                    agentDebugLog("inlineAssist.models.error existingCount=\(self.inlineAssistModels.count) error=\(error.localizedDescription)")
                    if self.inlineAssistModels.isEmpty {
                        self.inlineAssistSelectedModel = nil
                        inlineAssistState.errorMessage = error.localizedDescription
                        self.inlineAssistState = inlineAssistState
                    }
                }
            }
        }
    }

    private func mergeInlineAssistModels(primary: [EditorAgentModel], secondary: [EditorAgentModel]) -> [EditorAgentModel] {
        var seen: Set<String> = []
        var merged: [EditorAgentModel] = []
        for model in primary + secondary {
            let key = model.reference.lowercased()
            guard seen.insert(key).inserted else { continue }
            merged.append(model)
        }
        return merged.sorted { lhs, rhs in
            if lhs.isCurrent != rhs.isCurrent { return lhs.isCurrent }
            let providerCompare = lhs.provider.localizedCaseInsensitiveCompare(rhs.provider)
            if providerCompare != .orderedSame { return providerCompare == .orderedAscending }
            return lhs.id.localizedCaseInsensitiveCompare(rhs.id) == .orderedAscending
        }
    }

    func availableInlineAssistThinkingLevels() -> [String] {
        selectedInlineAssistModel()?.supportsReasoning == true ? supportedInlineAssistThinkingLevels : ["off"]
    }

    private func selectedInlineAssistModel() -> EditorAgentModel? {
        guard let selected = inlineAssistSelectedModel else { return nil }
        return inlineAssistModels.first(where: { $0.provider == selected.provider && $0.id == selected.modelID })
    }

    private func normalizeInlineAssistThinkingLevel() {
        let availableLevels = availableInlineAssistThinkingLevels()
        if availableLevels.contains(inlineAssistThinkingLevel) {
            return
        }
        inlineAssistThinkingLevel = availableLevels.contains("medium") ? "medium" : (availableLevels.first ?? "off")
    }

    private func applyInlineAssistModels(_ models: [EditorAgentModel], preferred: (provider: String, modelID: String)?) {
        guard var currentState = inlineAssistState else { return }
        agentDebugLog("inlineAssist.models.apply count=\(models.count) preferred=\(preferred.map { "\($0.provider)/\($0.modelID)" } ?? "nil") current=\(inlineAssistSelectedModel.map { "\($0.provider)/\($0.modelID)" } ?? "nil")")
        inlineAssistModels = models
        if let preferred,
           models.contains(where: { $0.provider == preferred.provider && $0.id == preferred.modelID }) {
            inlineAssistSelectedModel = EditorInlineAssistModelSelection(provider: preferred.provider, modelID: preferred.modelID)
            currentState.errorMessage = nil
            normalizeInlineAssistThinkingLevel()
            inlineAssistState = currentState
        } else if let current = inlineAssistSelectedModel,
                  models.contains(where: { $0.provider == current.provider && $0.id == current.modelID }) {
            currentState.errorMessage = nil
            inlineAssistSelectedModel = current
            normalizeInlineAssistThinkingLevel()
            inlineAssistState = currentState
        } else if let firstModel = models.first {
            inlineAssistSelectedModel = EditorInlineAssistModelSelection(provider: firstModel.provider, modelID: firstModel.id)
            currentState.errorMessage = nil
            normalizeInlineAssistThinkingLevel()
            inlineAssistState = currentState
        } else {
            inlineAssistSelectedModel = nil
            inlineAssistThinkingLevel = "off"
            currentState.errorMessage = "No inline rewrite models are configured. Add a Kimi model via opencode-go or a Composer model via Cursor."
            inlineAssistState = currentState
        }
    }

    private func startBackgroundPolling() {
        guard backgroundPollCancellable == nil else { return }
        backgroundPollCancellable = Timer.publish(every: 1.0 / 60.0, on: .main, in: .common)
            .autoconnect()
            .sink { [weak self] _ in
                guard let self else { return }
                let pollStarted = CFAbsoluteTimeGetCurrent()
                let changed = EditorFFIBridge.pollBackgroundTasks(self.handle?.raw)
                let pollMs = (CFAbsoluteTimeGetCurrent() - pollStarted) * 1000
                scrollPerfLog("controller.pollBackground changed=\(changed) pollMs=\(String(format: "%.2f", pollMs))")
                guard changed else { return }
                let refreshStarted = CFAbsoluteTimeGetCurrent()
                self.refreshSnapshot()
                let refreshMs = (CFAbsoluteTimeGetCurrent() - refreshStarted) * 1000
                scrollPerfLog("controller.pollBackground refreshMs=\(String(format: "%.2f", refreshMs))")
            }
    }

    private func refreshOpenItemsDecoration() {
        let decorated = decorateOpenItems(baseOpenItems)
        guard decorated != openItems else { return }
        objectWillChange.send()
        openItems = decorated
    }

    private func purgeLegacyAgentOpenItemsIfNeeded(_ state: EditorPaneOpenItemsState) -> Bool {
        let legacyAgentItems = state.groups.flatMap(\.items).filter { $0.kind == .agent }
        guard !legacyAgentItems.isEmpty else { return false }

        var didCloseAny = false
        for item in legacyAgentItems {
            if EditorFFIBridge.closeOpenItem(handle?.raw, paneID: item.paneID, kind: item.kind, itemID: item.itemID) {
                didCloseAny = true
            }
        }
        return didCloseAny
    }

    private func pruneTerminalPresentation(validSurfaceIDs: Set<UInt>) {
        terminalPresentationBySurfaceID = terminalPresentationBySurfaceID.filter { validSurfaceIDs.contains($0.key) }
    }

    private func decorateOpenItems(_ state: EditorPaneOpenItemsState) -> EditorPaneOpenItemsState {
        let groups = state.groups.compactMap { group -> EditorPaneOpenItemGroup? in
            let items = group.items
                .filter { $0.kind != .agent }
                .map(decorateOpenItem)
            guard !items.isEmpty else { return nil }
            let activeIndex = items.firstIndex(where: { $0.isActive })
            return EditorPaneOpenItemGroup(
                paneID: group.paneID,
                isActivePane: group.isActivePane,
                activeIndex: activeIndex,
                items: items
            )
        }
        return EditorPaneOpenItemsState(
            isVisible: state.isVisible && !groups.isEmpty,
            groups: groups
        )
    }

    private func decorateOpenItem(_ item: EditorPaneOpenItemRow) -> EditorPaneOpenItemRow {
        guard item.kind == .terminal,
              let clientSurfaceID = item.clientSurfaceID
        else {
            return item
        }
        let state = terminalPresentationBySurfaceID[clientSurfaceID]
        let fallbackTitle = state?.workingDirectory
            .flatMap(formatTerminalDisplayPath)
            ?? "Terminal"
        let title = state?.title ?? fallbackTitle
        return EditorPaneOpenItemRow(
            paneID: item.paneID,
            kind: item.kind,
            itemID: item.itemID,
            bufferID: item.bufferID,
            clientSurfaceID: item.clientSurfaceID,
            title: title,
            subtitle: item.subtitle,
            filePath: item.filePath,
            iconName: item.iconName,
            isActive: item.isActive,
            isModified: item.isModified,
            vcsKind: item.vcsKind,
            diagnosticSeverity: item.diagnosticSeverity
        )
    }

    private func formatTerminalDisplayPath(_ path: String) -> String {
        let trimmed = path.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return "Terminal" }
        return (trimmed as NSString).abbreviatingWithTildeInPath
    }

    private func normalizedTerminalStatusTitle(_ title: String?, workingDirectory: String?) -> String? {
        guard let rawTitle = title?.trimmingCharacters(in: .whitespacesAndNewlines), !rawTitle.isEmpty else {
            return nil
        }
        let normalizedTitle = rawTitle.lowercased()
        if normalizedTitle == "terminal" || normalizedTitle.hasPrefix("terminal ") {
            return nil
        }
        if let workingDirectory {
            let directoryName = URL(fileURLWithPath: workingDirectory).lastPathComponent
            if rawTitle == workingDirectory || (!directoryName.isEmpty && rawTitle == directoryName) {
                return nil
            }
        }
        return rawTitle
    }

    private func markedTextOverlay(from snapshot: EditorSnapshot) -> EditorMarkedText? {
        guard !markedText.isEmpty else { return nil }
        guard let cursor = snapshot.cursors.first else { return nil }
        return EditorMarkedText(text: markedText, row: cursor.row, col: cursor.col)
    }
}

private func agentFollowAnimationFrames(from originalText: String, to updatedText: String) -> [String] {
    guard originalText != updatedText else { return [] }

    let originalChars = Array(originalText)
    let updatedChars = Array(updatedText)
    var prefixCount = 0
    while prefixCount < originalChars.count,
          prefixCount < updatedChars.count,
          originalChars[prefixCount] == updatedChars[prefixCount] {
        prefixCount += 1
    }

    var suffixCount = 0
    while suffixCount < (originalChars.count - prefixCount),
          suffixCount < (updatedChars.count - prefixCount),
          originalChars[originalChars.count - 1 - suffixCount] == updatedChars[updatedChars.count - 1 - suffixCount] {
        suffixCount += 1
    }

    let prefix = String(originalChars[..<prefixCount])
    let originalMiddle = Array(originalChars[prefixCount..<(originalChars.count - suffixCount)])
    let updatedMiddle = Array(updatedChars[prefixCount..<(updatedChars.count - suffixCount)])
    let suffix = String(updatedChars[(updatedChars.count - suffixCount)...])

    let changeMagnitude = originalMiddle.count + updatedMiddle.count
    if changeMagnitude > 2_000 {
        return [updatedText]
    }

    let maxDeleteFrames = 10
    let maxInsertFrames = 14
    let deleteFrames = originalMiddle.isEmpty ? 0 : min(maxDeleteFrames, max(1, Int(ceil(Double(originalMiddle.count) / 24.0))))
    let insertFrames = updatedMiddle.isEmpty ? 0 : min(maxInsertFrames, max(1, Int(ceil(Double(updatedMiddle.count) / 20.0))))

    var frames: [String] = []
    frames.reserveCapacity(deleteFrames + insertFrames)

    if deleteFrames > 0 {
        for step in 1...deleteFrames {
            let remainingCount = max(originalMiddle.count - Int(round(Double(step) * Double(originalMiddle.count) / Double(deleteFrames))), 0)
            let middle = String(originalMiddle.prefix(remainingCount))
            frames.append(prefix + middle + suffix)
        }
    }

    if insertFrames > 0 {
        for step in 1...insertFrames {
            let insertedCount = min(Int(round(Double(step) * Double(updatedMiddle.count) / Double(insertFrames))), updatedMiddle.count)
            let middle = String(updatedMiddle.prefix(insertedCount))
            frames.append(prefix + middle + suffix)
        }
    }

    if frames.last != updatedText {
        frames.append(updatedText)
    }

    return frames
}

private extension Array {
    subscript(safe index: Int) -> Element? {
        guard indices.contains(index) else { return nil }
        return self[index]
    }
}
