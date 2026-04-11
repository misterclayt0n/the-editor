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
    private(set) var inputPrompt: EditorInputPromptState = .empty
    private(set) var hoverDocs: EditorDocsPanelState = .empty
    private(set) var completionDocs: EditorDocsPanelState = .empty
    private(set) var signatureHelp: EditorDocsPanelState = .empty
    private(set) var filePicker: EditorFilePickerState = .empty
    private(set) var bufferTabs: EditorBufferTabsState = .empty
    private(set) var openItems: EditorPaneOpenItemsState = .empty
    private(set) var fileTree: EditorFileTreeState = .empty
    @Published private(set) var showsResizeOverlay = false
    @Published private(set) var isAgentFollowEnabled = true

    private struct TerminalPresentationState {
        var title: String?
        var workingDirectory: String?
    }

    private var baseOpenItems: EditorPaneOpenItemsState = .empty
    private var terminalPresentationBySurfaceID: [UInt: TerminalPresentationState] = [:]

    private var surfaceConfiguration: EditorSurfaceConfiguration?
    private var markedText: String = ""
    private var backgroundPollCancellable: AnyCancellable?
    private var filePickerListVisibleRows: Int = 1
    private var filePickerPreviewVisibleRows: Int = 1
    private var fileTreeVisibleRows: Int = 1
    private var pendingFileTreeToggleStartedAt: CFAbsoluteTime?
    private var pendingFileTreeToggleTargetVisibility: Bool?
    private var fileTreeToggleResizeTask: Task<Void, Never>?
    private var resizeOverlayHideTask: Task<Void, Never>?
    private var interactiveResizeReasons: Set<String> = []
    private var pendingSurfaceConfiguration: EditorSurfaceConfiguration?
    private var surfaceConfigureFlushTask: Task<Void, Never>?
    private var lastSurfaceConfigureAt: CFAbsoluteTime = 0
    private var lastAgentFollowDebugSignature: String?

    private let interactiveResizeMinInterval: CFTimeInterval = 1.0 / 30.0
    private let fileTreeToggleResizeDuration: Duration = .milliseconds(360)

    init(initialPath: String?) {
        self.handle = EditorFFIBridge.createHandle(initialPath: initialPath).map(EditorHandleBox.init(raw:))
        _ = EditorFFIBridge.setEmbeddedTerminalEnabled(handle?.raw, enabled: GhosttyTerminalRegistry.isAvailable)
        isAgentFollowEnabled = EditorFFIBridge.agentFollowEnabled(handle?.raw)
        startBackgroundPolling()
        refreshSnapshot()
    }

    deinit {
        resizeOverlayHideTask?.cancel()
        surfaceConfigureFlushTask?.cancel()
        fileTreeToggleResizeTask?.cancel()
        EditorFFIBridge.destroyHandle(handle?.raw)
    }

    var isInteractiveResizeActive: Bool {
        !interactiveResizeReasons.isEmpty
    }

    @discardableResult
    func configureSurface(size: CGSize, backingScale: CGFloat, fontMetrics: EditorFontMetrics) -> Bool {
        let previousConfiguration = surfaceConfiguration
        let configuration = fontMetrics.surfaceConfiguration(viewSize: size, backingScale: backingScale)
        guard configuration != surfaceConfiguration else { return false }
        let sizeText = String(format: "%.1fx%.1f", size.width, size.height)
        let scaleText = String(format: "%.2f", backingScale)
        let oldPxText = previousConfiguration.map { "\($0.widthPx)x\($0.heightPx)" } ?? "nil"
        let newPxText = "\(configuration.widthPx)x\(configuration.heightPx)"
        let cellPxText = "\(configuration.metrics.cellWidthPx)x\(configuration.metrics.cellHeightPx)"
        let reasons = interactiveResizeReasons.sorted()
        let reasonText = reasons.joined(separator: ",")
        let now = CFAbsoluteTimeGetCurrent()
        let elapsed = now - lastSurfaceConfigureAt
        let deferUntilEndReasons: Set<String> = ["sidebar", "fileTreeToggle"]
        let deferUntilEndActive = !interactiveResizeReasons.isDisjoint(with: deferUntilEndReasons)

        if deferUntilEndActive {
            pendingSurfaceConfiguration = configuration
            surfaceConfigureFlushTask?.cancel()
            surfaceConfigureFlushTask = nil
            let elapsedMsText = String(format: "%.2f", elapsed * 1000)
            scrollPerfLog(
                "controller.configureSurface deferred-until-end reasons=\(reasonText) size=\(sizeText) backingScale=\(scaleText) oldPx=\(oldPxText) newPx=\(newPxText) cellPx=\(cellPxText) elapsedMs=\(elapsedMsText)"
            )
            return false
        }

        if isInteractiveResizeActive && elapsed < interactiveResizeMinInterval {
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
        scrollPerfLog("controller.interactiveResize begin reason=\(reason) active=\(interactiveResizeReasons.sorted())")
    }

    func endInteractiveResize(reason: String) {
        let removed = interactiveResizeReasons.remove(reason) != nil
        guard removed else { return }
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
        pendingSurfaceConfiguration = nil
        surfaceConfigureFlushTask?.cancel()
        surfaceConfigureFlushTask = nil
        surfaceConfiguration = configuration
        lastSurfaceConfigureAt = CFAbsoluteTimeGetCurrent()
        scrollPerfLog(
            "controller.configureSurface apply source=\(source) reasons=\(reasonText) size=\(sizeText) backingScale=\(scaleText) oldPx=\(oldPxText) newPx=\(newPxText) cellPx=\(cellPxText)"
        )
        guard EditorFFIBridge.configureSurface(handle?.raw, configuration: configuration) else { return false }
        refreshSnapshot()
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

    func closeOpenItem(_ item: EditorPaneOpenItemRow) {
        guard EditorFFIBridge.closeOpenItem(handle?.raw, paneID: item.paneID, kind: item.kind, itemID: item.itemID) else { return }
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

    func setAgentFollowEnabled(_ enabled: Bool) {
        guard enabled != isAgentFollowEnabled else { return }
        guard EditorFFIBridge.setAgentFollowEnabled(handle?.raw, enabled: enabled) else { return }
        isAgentFollowEnabled = enabled
        refreshSnapshot()
    }

    func toggleAgentFollowEnabled() {
        setAgentFollowEnabled(!isAgentFollowEnabled)
    }

    func closeTerminalInActivePane() {
        guard EditorFFIBridge.closeTerminalInActivePane(handle?.raw) else { return }
        refreshSnapshot()
    }

    func registerTerminalSurface(_ clientSurfaceID: UInt, preferredWorkingDirectory: String?) {
        var state = terminalPresentationBySurfaceID[clientSurfaceID] ?? TerminalPresentationState()
        if state.workingDirectory?.isEmpty != false,
           let preferredWorkingDirectory,
           !preferredWorkingDirectory.isEmpty {
            state.workingDirectory = preferredWorkingDirectory
        }
        terminalPresentationBySurfaceID[clientSurfaceID] = state
        applyTerminalPresentationIfNeeded()
    }

    func updateTerminalTitle(_ title: String, for clientSurfaceID: UInt) {
        var state = terminalPresentationBySurfaceID[clientSurfaceID] ?? TerminalPresentationState()
        let trimmed = title.trimmingCharacters(in: .whitespacesAndNewlines)
        state.title = trimmed.isEmpty ? nil : trimmed
        terminalPresentationBySurfaceID[clientSurfaceID] = state
        applyTerminalPresentationIfNeeded()
    }

    func updateTerminalWorkingDirectory(_ workingDirectory: String, for clientSurfaceID: UInt) {
        var state = terminalPresentationBySurfaceID[clientSurfaceID] ?? TerminalPresentationState()
        let trimmed = workingDirectory.trimmingCharacters(in: .whitespacesAndNewlines)
        state.workingDirectory = trimmed.isEmpty ? nil : trimmed
        terminalPresentationBySurfaceID[clientSurfaceID] = state
        applyTerminalPresentationIfNeeded()
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

    func handleKey(_ event: the_editor_key_event_t) {
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
        pendingFileTreeToggleStartedAt = started
        pendingFileTreeToggleTargetVisibility = targetVisibility
        startFileTreeToggleResizeTracking(targetVisibility: targetVisibility, source: "toggle-request")
        scrollPerfLog(
            "fileTree.toggle requested visible=\(fileTree.isVisible)→\(targetVisibility) rows=\(fileTree.rows.count) selected=\(String(describing: fileTree.selectedIndex)) scroll=\(fileTree.scrollOffset)"
        )
        let ffiStarted = CFAbsoluteTimeGetCurrent()
        guard EditorFFIBridge.toggleFileTree(handle?.raw) else {
            pendingFileTreeToggleStartedAt = nil
            pendingFileTreeToggleTargetVisibility = nil
            endInteractiveResize(reason: "fileTreeToggle")
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

    func beginLiveResize() {
        beginInteractiveResize(reason: "window")
        resizeOverlayHideTask?.cancel()
        if !showsResizeOverlay {
            showsResizeOverlay = true
        }
    }

    func endLiveResize() {
        endInteractiveResize(reason: "window")
        resizeOverlayHideTask?.cancel()
        resizeOverlayHideTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(650))
            guard let self else { return }
            self.showsResizeOverlay = false
        }
    }

    func refreshSnapshot() {
        let wasInputPromptOpen = inputPrompt.isOpen
        let started = CFAbsoluteTimeGetCurrent()
        guard let snapshot = EditorFFIBridge.makeSnapshot(handle?.raw) else { return }
        let snapshotMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
        let nextChrome = EditorChromeModel(
            document: snapshot.document,
            statusBar: snapshot.statusBar,
            backgroundColor: snapshot.info.backgroundColor?.color ?? .windowBackgroundColor
        )
        let nextAgentFollowEnabled = EditorFFIBridge.agentFollowEnabled(handle?.raw)
        commandPaletteDebugLog("refresh query=\(String(reflecting: snapshot.commandPalette.query)) selected=\(String(describing: snapshot.commandPalette.selectedIndex)) items=\(snapshot.commandPalette.items.count) isOpen=\(snapshot.commandPalette.isOpen)")
        let sceneStarted = CFAbsoluteTimeGetCurrent()
        let marked = markedTextOverlay(from: snapshot)
        let nextScene = EditorRenderScene.from(snapshot: snapshot, markedText: marked)
        let sceneMs = (CFAbsoluteTimeGetCurrent() - sceneStarted) * 1000

        let publishStarted = CFAbsoluteTimeGetCurrent()
        objectWillChange.send()
        currentMode = snapshot.info.mode
        chrome = nextChrome
        isAgentFollowEnabled = nextAgentFollowEnabled
        pendingKeys = snapshot.pendingKeys
        commandPalette = snapshot.commandPalette
        completionMenu = snapshot.completionMenu
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
            if previousFileTree.isVisible != snapshot.fileTree.isVisible {
                if pendingFileTreeToggleStartedAt == nil {
                    pendingFileTreeToggleStartedAt = CFAbsoluteTimeGetCurrent()
                }
                pendingFileTreeToggleTargetVisibility = snapshot.fileTree.isVisible
                startFileTreeToggleResizeTracking(targetVisibility: snapshot.fileTree.isVisible, source: "snapshot-transition")
            }
            let toggleDeltaMs = pendingFileTreeToggleStartedAt.map { (CFAbsoluteTimeGetCurrent() - $0) * 1000 }
            let deltaText = toggleDeltaMs.map { String(format: "%.2f", $0) } ?? "-"
            scrollPerfLog(
                "controller.fileTree transition visible=\(previousFileTree.isVisible)→\(snapshot.fileTree.isVisible) rows=\(previousFileTree.rows.count)→\(snapshot.fileTree.rows.count) selected=\(String(describing: snapshot.fileTree.selectedIndex)) scroll=\(snapshot.fileTree.scrollOffset) toggleDeltaMs=\(deltaText) target=\(String(describing: pendingFileTreeToggleTargetVisibility))"
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
        let totalMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
        themePerfLog(
            "controller.refresh themeGen=\(snapshot.info.themeGeneration) snapshotMs=\(String(format: "%.2f", snapshotMs)) sceneMs=\(String(format: "%.2f", sceneMs)) totalMs=\(String(format: "%.2f", totalMs))"
        )
        scrollPerfLog(
            "controller.refresh damage=\(snapshot.info.damageReason) full=\(snapshot.info.damageIsFull) scrollGen=\(snapshot.info.scrollGeneration) textGen=\(snapshot.info.textGeneration) decoGen=\(snapshot.info.decorationGeneration) themeGen=\(snapshot.info.themeGeneration) lines=\(snapshot.lines.count) cursors=\(snapshot.cursors.count) overlays=\(snapshot.overlays.count) snapshotMs=\(String(format: "%.2f", snapshotMs)) sceneMs=\(String(format: "%.2f", sceneMs)) publishMs=\(String(format: "%.2f", publishMs)) delegateMs=\(String(format: "%.2f", delegateMs)) totalMs=\(String(format: "%.2f", totalMs))"
        )
        let hollowCursors = snapshot.cursors.filter { $0.kind == .hollow }
        let hoverSelections = snapshot.selections.filter { $0.kind == .hover }
        let highlightOverlays = snapshot.overlays.filter { $0.kind == .rect && $0.rectKind == .highlight }
        let cursorSummary = hollowCursors.prefix(3).map { "(\($0.row),\($0.col))" }.joined(separator: ",")
        let overlaySummary = highlightOverlays.prefix(3).map { "y=\($0.y) h=\($0.height)" }.joined(separator: ",")
        let agentFollowSignature = "damage=\(snapshot.info.damageReason.rawValue)|hollow=\(hollowCursors.count)|hover=\(hoverSelections.count)|highlight=\(highlightOverlays.count)|cursor=\(cursorSummary)|overlay=\(overlaySummary)"
        if agentFollowSignature != lastAgentFollowDebugSignature {
            lastAgentFollowDebugSignature = agentFollowSignature
            agentFollowDebugLog(
                "controller.refresh \(agentFollowSignature) totalCursors=\(snapshot.cursors.count) totalSelections=\(snapshot.selections.count) totalOverlays=\(snapshot.overlays.count)"
            )
        }
        if snapshot.completionMenu.isOpen || snapshot.completionDocs.isOpen {
            completionPerfLog(
                "controller.refresh menuOpen=\(snapshot.completionMenu.isOpen) docsOpen=\(snapshot.completionDocs.isOpen) items=\(snapshot.completionMenu.items.count) selected=\(String(describing: snapshot.completionMenu.selectedIndex)) scroll=\(snapshot.completionMenu.scrollOffset) snapshotMs=\(String(format: "%.2f", snapshotMs)) sceneMs=\(String(format: "%.2f", sceneMs)) totalMs=\(String(format: "%.2f", totalMs))"
            )
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

    private func startFileTreeToggleResizeTracking(targetVisibility: Bool, source: String) {
        beginInteractiveResize(reason: "fileTreeToggle")
        fileTreeToggleResizeTask?.cancel()
        scrollPerfLog(
            "controller.fileTreeToggleResize begin source=\(source) target=\(targetVisibility) active=\(interactiveResizeReasons.sorted())"
        )
        fileTreeToggleResizeTask = Task { @MainActor [weak self] in
            let duration = self?.fileTreeToggleResizeDuration ?? .milliseconds(360)
            do {
                try await Task.sleep(for: duration)
            } catch {
                return
            }
            guard let self, !Task.isCancelled else { return }
            let target = self.pendingFileTreeToggleTargetVisibility
            self.pendingFileTreeToggleStartedAt = nil
            self.pendingFileTreeToggleTargetVisibility = nil
            scrollPerfLog(
                "controller.fileTreeToggleResize end target=\(String(describing: target)) visible=\(self.fileTree.isVisible) active=\(self.interactiveResizeReasons.sorted())"
            )
            self.endInteractiveResize(reason: "fileTreeToggle")
            self.fileTreeToggleResizeTask = nil
        }
    }

    private func applyTerminalPresentationIfNeeded() {
        let decorated = decorateOpenItems(baseOpenItems)
        guard decorated != openItems else { return }
        objectWillChange.send()
        openItems = decorated
    }

    private func pruneTerminalPresentation(validSurfaceIDs: Set<UInt>) {
        terminalPresentationBySurfaceID = terminalPresentationBySurfaceID.filter { validSurfaceIDs.contains($0.key) }
    }

    private func decorateOpenItems(_ state: EditorPaneOpenItemsState) -> EditorPaneOpenItemsState {
        EditorPaneOpenItemsState(
            isVisible: state.isVisible,
            groups: state.groups.map { group in
                EditorPaneOpenItemGroup(
                    paneID: group.paneID,
                    isActivePane: group.isActivePane,
                    activeIndex: group.activeIndex,
                    items: group.items.map(decorateOpenItem)
                )
            }
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

    private func markedTextOverlay(from snapshot: EditorSnapshot) -> EditorMarkedText? {
        guard !markedText.isEmpty else { return nil }
        guard let cursor = snapshot.cursors.first else { return nil }
        return EditorMarkedText(text: markedText, row: cursor.row, col: cursor.col)
    }
}
