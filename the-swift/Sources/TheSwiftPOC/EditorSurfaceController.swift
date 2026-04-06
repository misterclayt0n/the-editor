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
    @Published private(set) var scene: EditorRenderScene?
    @Published private(set) var chrome = EditorChromeModel.empty
    @Published private(set) var currentMode: EditorMode = .normal
    @Published private(set) var commandPalette: EditorCommandPaletteState = .empty
    @Published private(set) var completionMenu: EditorCompletionMenuState = .empty
    @Published private(set) var inputPrompt: EditorInputPromptState = .empty
    @Published private(set) var hoverDocs: EditorDocsPanelState = .empty
    @Published private(set) var completionDocs: EditorDocsPanelState = .empty
    @Published private(set) var signatureHelp: EditorDocsPanelState = .empty
    @Published private(set) var filePicker: EditorFilePickerState = .empty
    @Published private(set) var showsResizeOverlay = false

    private var surfaceConfiguration: EditorSurfaceConfiguration?
    private var markedText: String = ""
    private var backgroundPollCancellable: AnyCancellable?
    private var filePickerListVisibleRows: Int = 1
    private var filePickerPreviewVisibleRows: Int = 1
    private var resizeOverlayHideTask: Task<Void, Never>?

    init(initialPath: String?) {
        self.handle = EditorFFIBridge.createHandle(initialPath: initialPath).map(EditorHandleBox.init(raw:))
        startBackgroundPolling()
        refreshSnapshot()
    }

    deinit {
        resizeOverlayHideTask?.cancel()
        EditorFFIBridge.destroyHandle(handle?.raw)
    }

    @discardableResult
    func configureSurface(size: CGSize, backingScale: CGFloat, fontMetrics: EditorFontMetrics) -> Bool {
        let configuration = fontMetrics.surfaceConfiguration(viewSize: size, backingScale: backingScale)
        guard configuration != surfaceConfiguration else { return false }
        surfaceConfiguration = configuration
        guard EditorFFIBridge.configureSurface(handle?.raw, configuration: configuration) else { return false }
        refreshSnapshot()
        return true
    }

    func setScrollRow(_ row: Int) {
        guard EditorFFIBridge.setScrollRow(handle?.raw, row: UInt32(max(row, 0))) else { return }
        refreshSnapshot()
    }

    func setScrollCol(_ col: Int) {
        guard EditorFFIBridge.setScrollCol(handle?.raw, col: UInt32(max(col, 0))) else { return }
        refreshSnapshot()
    }

    func scroll(byRows rowDelta: Int, cols colDelta: Int) {
        guard (rowDelta != 0 || colDelta != 0), let scene else { return }
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
        resizeOverlayHideTask?.cancel()
        if !showsResizeOverlay {
            showsResizeOverlay = true
        }
    }

    func endLiveResize() {
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
        currentMode = snapshot.info.mode
        chrome = EditorChromeModel(
            document: snapshot.document,
            statusBar: snapshot.statusBar,
            backgroundColor: snapshot.info.backgroundColor?.color ?? .windowBackgroundColor
        )
        commandPalette = snapshot.commandPalette
        completionMenu = snapshot.completionMenu
        inputPrompt = snapshot.inputPrompt
        hoverDocs = snapshot.hoverDocs
        completionDocs = snapshot.completionDocs
        signatureHelp = snapshot.signatureHelp
        filePicker = snapshot.filePicker
        commandPaletteDebugLog("refresh query=\(String(reflecting: snapshot.commandPalette.query)) selected=\(String(describing: snapshot.commandPalette.selectedIndex)) items=\(snapshot.commandPalette.items.count) isOpen=\(snapshot.commandPalette.isOpen)")
        let sceneStarted = CFAbsoluteTimeGetCurrent()
        let marked = markedTextOverlay(from: snapshot)
        let scene = EditorRenderScene.from(snapshot: snapshot, markedText: marked)
        let sceneMs = (CFAbsoluteTimeGetCurrent() - sceneStarted) * 1000
        self.scene = scene
        delegate?.editorController(self, didUpdateScene: scene)
        if wasInputPromptOpen && !snapshot.inputPrompt.isOpen {
            DispatchQueue.main.async { [weak self] in
                self?.focusEditor()
            }
        }
        let totalMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
        themePerfLog(
            "controller.refresh themeGen=\(snapshot.info.themeGeneration) snapshotMs=\(String(format: "%.2f", snapshotMs)) sceneMs=\(String(format: "%.2f", sceneMs)) totalMs=\(String(format: "%.2f", totalMs))"
        )
        if snapshot.completionMenu.isOpen || snapshot.completionDocs.isOpen {
            completionPerfLog(
                "controller.refresh menuOpen=\(snapshot.completionMenu.isOpen) docsOpen=\(snapshot.completionDocs.isOpen) items=\(snapshot.completionMenu.items.count) selected=\(String(describing: snapshot.completionMenu.selectedIndex)) scroll=\(snapshot.completionMenu.scrollOffset) snapshotMs=\(String(format: "%.2f", snapshotMs)) sceneMs=\(String(format: "%.2f", sceneMs)) totalMs=\(String(format: "%.2f", totalMs))"
            )
        }
    }

    private func startBackgroundPolling() {
        guard backgroundPollCancellable == nil else { return }
        backgroundPollCancellable = Timer.publish(every: 0.05, on: .main, in: .common)
            .autoconnect()
            .sink { [weak self] _ in
                guard let self else { return }
                guard EditorFFIBridge.pollBackgroundTasks(self.handle?.raw) else { return }
                self.refreshSnapshot()
            }
    }

    private func markedTextOverlay(from snapshot: EditorSnapshot) -> EditorMarkedText? {
        guard !markedText.isEmpty else { return nil }
        guard let cursor = snapshot.cursors.first else { return nil }
        return EditorMarkedText(text: markedText, row: cursor.row, col: cursor.col)
    }
}
