import AppKit
import Foundation
import SwiftUI
import TheEditorFFIBridge

struct SplitSeparatorSnapshot: Identifiable {
    let splitId: UInt64
    let axis: UInt8
    let line: UInt16
    let spanStart: UInt16
    let spanEnd: UInt16

    var id: UInt64 { splitId }
}

struct TerminalPaneSnapshot: Identifiable, Equatable {
    let paneId: UInt64
    let terminalId: UInt64
    let x: UInt16
    let y: UInt16
    let width: UInt16
    let height: UInt16
    let isActive: Bool

    var id: UInt64 { paneId }
}

struct TerminalSurfaceSnapshot: Identifiable, Equatable {
    let terminalId: UInt64
    let paneId: UInt64?
    let isActive: Bool

    var id: UInt64 { terminalId }
    var isAttached: Bool { paneId != nil }
}

struct EditorPaneSurfaceSnapshot: Identifiable, Equatable {
    let paneId: UInt64
    let bufferId: UInt64
    let bufferIndex: Int
    let title: String
    let modified: Bool
    let filePath: String?
    let isActive: Bool

    var id: UInt64 { paneId }
}

struct DocsPopupAnchorSnapshot: Equatable {
    let paneId: UInt64
    let row: UInt16
    let col: UInt16
}

private struct NativeTabOpenRequest: Decodable, Hashable {
    enum Kind: String, Decodable {
        case focusExisting = "focus_existing"
        case openNew = "open_new"
    }

    let kind: Kind
    let bufferId: UInt64
    let filePath: String?
}

final class EditorModel: ObservableObject {
    private struct NativeWindowPresentation: Equatable {
        let title: String
        let subtitle: String
        let representedFilePath: String?
        let isDocumentEdited: Bool
    }

    private enum PaneSurfaceKind {
        case editor
        case terminal
    }

    private enum PaneFocusDirection: UInt8 {
        case left = 0
        case right = 1
        case up = 2
        case down = 3
    }

    private enum PaneSplitAxis: UInt8 {
        // Vertical creates a right-hand pane (like :vsplit).
        case vertical = 0
        // Horizontal creates a pane below (like :hsplit).
        case horizontal = 1
    }

    private struct PaneSurfaceSnapshot: Equatable {
        let paneId: UInt64
        let kind: PaneSurfaceKind
    }

    private let runtime: SharedEditorRuntime
    private let app: TheEditorFFIBridge.App
    let runtimeInstanceId: UInt64
    let editorId: EditorId
    @Published var plan: RenderPlan
    @Published var framePlan: RenderFramePlan
    @Published var splitSeparators: [SplitSeparatorSnapshot] = []
    @Published var terminalPanes: [TerminalPaneSnapshot] = []
    @Published var terminalSurfaces: [TerminalSurfaceSnapshot] = []
    @Published var editorSurfacePanes: [EditorPaneSurfaceSnapshot] = []
    @Published var isActivePaneTerminal: Bool = false
    @Published var uiTree: UiTreeSnapshot = .empty
    @Published var docsPopupAnchor: DocsPopupAnchorSnapshot? = nil
    @Published var bufferTabsSnapshot: BufferTabsSnapshot? = nil
    @Published var navigationTitle: String = "untitled"
    @Published private(set) var isHostWindowFocused: Bool = false
    @Published private(set) var bufferFontSize: CGFloat
    private var viewport: Rect
    private var effectiveViewport: Rect
    private(set) var cellSize: CGSize
    private(set) var bufferFont: Font
    private(set) var bufferNSFont: NSFont
    private let initialFilePath: String?
    private var boundBufferId: UInt64? = nil
    private var pendingInitialBoundBufferId: UInt64? = nil
    private var pendingInitialBoundFilePath: String? = nil
    private(set) var mode: EditorMode = .normal
    @Published var pendingKeys: [String] = []
    @Published var pendingKeyHints: PendingKeyHintsSnapshot? = nil
    @Published var filePickerSnapshot: FilePickerSnapshot? = nil
    @Published var fileTreeSnapshot: FileTreeSnapshot = .hidden
    let filePickerPreviewModel = FilePickerPreviewModel()
    private var filePickerTimer: Timer? = nil
    private var backgroundTimer: Timer? = nil
    private var scrollRemainderX: CGFloat = 0
    private var scrollRemainderY: CGFloat = 0
    private var syntaxHighlightStyleCache: [UInt32: Style] = [:]
    private var effectiveThemeName: String = ""
    private var lastUiTreeJson: String? = nil
    private var lastBufferTabsJson: String? = nil
    private var lastPickerQuery: String? = nil
    private var lastPickerMatchedCount: Int = -1
    private var lastPickerTotalCount: Int = -1
    private var lastPickerScanning: Bool = false
    private var lastPickerTitle: String? = nil
    private var lastPickerRoot: String? = nil
    private var lastPickerKind: UInt8 = 0
    private var lastFileTreeRefreshGeneration: UInt64 = 0
    private var lastFileTreeVisible: Bool = false
    private var lastFileTreeRoot: String = ""
    private var lastFileTreeMode: UInt8 = 0
    private var lastFileTreeNodeCount: Int = -1
    private var filePickerPreviewOffsetHint: Int = -1
    private var filePickerPreviewVisibleRows: Int = 24
    private var filePickerPreviewOverscan: Int = 24
    private var topChromeReservedRows: Int = 0
    private weak var hostWindow: NSWindow? = nil
    private var hostWindowNotificationTokens: [NSObjectProtocol] = []
    private var openWindowTabHandler: ((EditorWindowRoute) -> Void)? = nil
    private var nativeTabGatewayRegistered: Bool = false
    private var suppressFocusedWindowBindingUpdate: Bool = false
    private var closeConfirmationAlert: NSAlert? = nil
    private var allowProgrammaticWindowClose: Bool = false
    private var lastNativeWindowPresentation: NativeWindowPresentation? = nil
    private var seededUntitledForUnroutedWindow: Bool = false
    private var paneSurfaceItems: [PaneSurfaceSnapshot] = []
    private var lastFocusedTerminalSurfaceId: UInt64? = nil
    private var lastFocusedEditorPaneId: UInt64? = nil

    init(filePath: String? = nil, bufferId: UInt64? = nil) {
        self.runtime = SharedEditorRuntime()
        self.app = runtime.app
        self.initialFilePath = filePath
        self.boundBufferId = bufferId
        self.pendingInitialBoundBufferId = bufferId
        self.pendingInitialBoundFilePath = EditorModel.normalizedFilePath(filePath)
        self.runtimeInstanceId = runtime.instanceId
        let fontInfo = FontLoader.loadBufferFont(size: FontZoomLimits.defaultBufferPointSize)
        self.bufferFontSize = FontZoomLimits.defaultBufferPointSize
        self.cellSize = fontInfo.cellSize
        self.bufferFont = fontInfo.font
        self.bufferNSFont = fontInfo.nsFont
        self.viewport = Rect(x: 0, y: 0, width: 80, height: 24)
        self.effectiveViewport = self.viewport
        self.editorId = runtime.editorId
        if let filePath, bufferId == nil {
            _ = app.open_file_path(editorId, filePath)
        }
        let initialFramePlan = app.frame_render_plan(editorId)
        self.framePlan = initialFramePlan
        self.plan = initialFramePlan.active_plan()
        self.splitSeparators = []
        self.terminalPanes = []
        self.terminalSurfaces = []
        self.isActivePaneTerminal = false
        self.mode = EditorMode(rawValue: app.mode(editorId)) ?? .normal
        synchronizeEffectiveTheme(force: true)
        updateTerminalPaneSnapshots(from: initialFramePlan)
        updateTerminalSurfaceSnapshots()
        refreshFileTree(force: true)
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "font init: nsFont=\(fontInfo.nsFont.fontName) pointSize=\(fontInfo.nsFont.pointSize) cell=\(Int(cellSize.width))x\(Int(cellSize.height))"
            )
        }
        startBackgroundTimerIfNeeded()
    }

    deinit {
        filePickerTimer?.invalidate()
        backgroundTimer?.invalidate()
        unregisterHostWindowNotifications()
        EditorCommandModelRegistry.shared.unregister(window: hostWindow)
    }

    private static func loadText(filePath: String?) -> String {
        guard let filePath else {
            return "hello from the-lib\nswift demo"
        }

        do {
            return try String(contentsOfFile: filePath, encoding: .utf8)
        } catch {
            let message = "the-swift: failed to read file: \(filePath)\n"
            if let data = message.data(using: .utf8) {
                FileHandle.standardError.write(data)
            }
            return ""
        }
    }

    func updateViewport(pixelSize: CGSize, cellSize: CGSize) {
        let cols = max(1, UInt16(pixelSize.width / cellSize.width))
        let rows = max(1, UInt16(pixelSize.height / cellSize.height))

        if cols == viewport.width && rows == viewport.height {
            return
        }

        viewport = Rect(x: 0, y: 0, width: cols, height: rows)
        refresh()
    }

    func refresh(trigger: String = "manual") {
        let perfEnabled = DiagnosticsDebugLog.editorPerfEnabled
        let perfStart = perfEnabled ? DispatchTime.now().uptimeNanoseconds : 0
        func perfNow() -> UInt64 {
            DispatchTime.now().uptimeNanoseconds
        }
        func perfMs(_ start: UInt64, _ end: UInt64) -> Double {
            guard end >= start else { return 0 }
            return Double(end - start) / 1_000_000.0
        }

        let shouldDriveRuntime = shouldDriveSharedRuntime()
        let runtimePollChanged: Bool
        if shouldDriveRuntime {
            runtimePollChanged = app.poll_background(editorId)
        } else {
            runtimePollChanged = false
        }
        let perfAfterPoll = perfEnabled ? perfNow() : 0
        synchronizeEffectiveTheme()
        let perfAfterTheme = perfEnabled ? perfNow() : 0
        let uiFetch = fetchUiTree()
        let perfAfterUi = perfEnabled ? perfNow() : 0
        if uiFetch.changed {
            uiTree = uiFetch.tree
        }
        let tabsFetch = fetchBufferTabsSnapshot()
        let perfAfterTabs = perfEnabled ? perfNow() : 0
        if tabsFetch.changed {
            bufferTabsSnapshot = tabsFetch.snapshot
        }
        updateBoundBufferIdFromActiveBufferIfNeeded()
        // Buffer tabs in the Swift app are native window tabs, not in-content chrome.
        setTopChromeReservedRows(0)
        updateEffectiveViewport()
        let perfAfterViewport = perfEnabled ? perfNow() : 0

        framePlan = app.frame_render_plan(editorId)
        plan = framePlan.active_plan()
        let popupAnchor = app.docs_popup_anchor(editorId)
        docsPopupAnchor = popupAnchor.has_value
            ? DocsPopupAnchorSnapshot(
                paneId: popupAnchor.pane_id,
                row: popupAnchor.row,
                col: popupAnchor.col
            )
            : nil
        let perfAfterFrame = perfEnabled ? perfNow() : 0
        splitSeparators = fetchSplitSeparators()
        updateTerminalPaneSnapshots(from: framePlan)
        updateTerminalSurfaceSnapshots()
        updateSurfaceOverviewSnapshots(from: framePlan)
        updateEditorSurfaceSnapshots()
        let perfAfterSnapshots = perfEnabled ? perfNow() : 0
        debugSurfaceRailState(trigger: trigger)
        debugDiagnosticsSnapshot(trigger: trigger, plan: plan)

        mode = EditorMode(rawValue: app.mode(editorId)) ?? .normal
        pendingKeys = fetchPendingKeys()
        pendingKeyHints = fetchPendingKeyHints()
        let perfAfterInputState = perfEnabled ? perfNow() : 0
        refreshFilePicker()
        let perfAfterPicker = perfEnabled ? perfNow() : 0
        refreshFileTree()
        let perfAfterTree = perfEnabled ? perfNow() : 0

        // Sync title/subtitle after pane snapshots update so terminal/editor focus
        // state reflects the latest frame.
        if shouldSyncNativeWindowPresentation() {
            syncNativeWindowPresentation()
        }
        let perfAfterWindow = perfEnabled ? perfNow() : 0

        if app.take_should_quit() {
            NSApp.terminate(nil)
        }

        guard perfEnabled else {
            return
        }
        let totalMs = perfMs(perfStart, perfAfterWindow)
        guard DiagnosticsDebugLog.editorPerfShouldLog(durationMs: totalMs) else {
            return
        }

        DiagnosticsDebugLog.editorPerfLog(
            String(
                format: "refresh trigger=%@ total=%.2fms poll=%.2fms theme=%.2fms ui=%.2fms tabs=%.2fms viewport=%.2fms frame=%.2fms snapshots=%.2fms input=%.2fms picker=%.2fms tree=%.2fms window=%.2fms runtime_drive=%d runtime_changed=%d ui_changed=%d tabs_changed=%d overlays=%d panes=%d lines=%d eol=%d underlines=%d terminals=%d active_terminal=%d",
                trigger,
                totalMs,
                perfMs(perfStart, perfAfterPoll),
                perfMs(perfAfterPoll, perfAfterTheme),
                perfMs(perfAfterTheme, perfAfterUi),
                perfMs(perfAfterUi, perfAfterTabs),
                perfMs(perfAfterTabs, perfAfterViewport),
                perfMs(perfAfterViewport, perfAfterFrame),
                perfMs(perfAfterFrame, perfAfterSnapshots),
                perfMs(perfAfterSnapshots, perfAfterInputState),
                perfMs(perfAfterInputState, perfAfterPicker),
                perfMs(perfAfterPicker, perfAfterTree),
                perfMs(perfAfterTree, perfAfterWindow),
                shouldDriveRuntime ? 1 : 0,
                runtimePollChanged ? 1 : 0,
                uiFetch.changed ? 1 : 0,
                tabsFetch.changed ? 1 : 0,
                uiTree.overlays.count,
                Int(framePlan.pane_count()),
                Int(plan.line_count()),
                Int(plan.eol_diagnostic_count()),
                Int(plan.diagnostic_underline_count()),
                terminalPanes.count,
                isActivePaneTerminal ? 1 : 0
            )
        )
    }

    private func updateEffectiveViewport() {
        let reserved = statuslineReservedRows(in: uiTree) + max(0, topChromeReservedRows)
        let height = max(1, Int(viewport.height) - reserved)
        let next = Rect(x: 0, y: 0, width: viewport.width, height: UInt16(height))
        if !rectsEqual(next, effectiveViewport) {
            effectiveViewport = next
            _ = app.set_viewport(editorId, next)
        }
    }

    func setTopChromeReservedRows(_ rows: Int) {
        let next = max(0, rows)
        guard next != topChromeReservedRows else {
            return
        }
        topChromeReservedRows = next
        updateEffectiveViewport()
    }

    func currentTopChromeReservedRows() -> Int {
        topChromeReservedRows
    }

    private func rectsEqual(_ lhs: Rect, _ rhs: Rect) -> Bool {
        lhs.x == rhs.x && lhs.y == rhs.y && lhs.width == rhs.width && lhs.height == rhs.height
    }

    private func statuslineReservedRows(in tree: UiTreeSnapshot) -> Int {
        return 0
    }

    func insert(_ text: String) {
        _ = app.insert(editorId, text)
        refresh()
    }

    func handleKeyEvent(_ keyEvent: KeyEvent) {
        if mode == .command {
            if sendUiKeyEvent(keyEvent) {
                refresh(trigger: "key_ui")
                return
            }
        }

        _ = app.handle_key(editorId, keyEvent)
        refresh(trigger: "key")
    }

    func handleText(_ text: String, modifiers: NSEvent.ModifierFlags) {
        let includeModifiers = !mode.isTextInput && (modifiers.contains(.control) || modifiers.contains(.option))
        let events = KeyEventMapper.mapText(text, modifiers: modifiers, includeModifiers: includeModifiers)
        for event in events {
            if mode == .command {
                if sendUiKeyEvent(event) {
                    continue
                }
            }
            _ = app.handle_key(editorId, event)
        }
        refresh(trigger: "text")
    }

    private func sendUiKeyEvent(_ keyEvent: KeyEvent) -> Bool {
        guard let envelope = UiEventEncoder.keyEventEnvelope(from: keyEvent) else {
            return false
        }
        guard let json = UiEventEncoder.encode(envelope) else {
            return false
        }
        return app.ui_event_json(editorId, json)
    }

    func handleScroll(deltaX: CGFloat, deltaY: CGFloat, precise: Bool) {
        let (rowDelta, colDelta) = scrollDelta(deltaX: deltaX, deltaY: deltaY, precise: precise)
        if rowDelta == 0 && colDelta == 0 {
            return
        }

        let current = plan.scroll()
        let newRow = max(0, Int(current.row) + rowDelta)
        let newCol = max(0, Int(current.col) + colDelta)
        let scroll = Position(row: UInt64(newRow), col: UInt64(newCol))
        _ = app.set_scroll(editorId, scroll)
        refresh(trigger: "scroll")
    }

    func handlePointerEvent(_ event: MouseBridgeEvent) {
        guard app.handle_mouse(
            editorId,
            event.packed,
            event.logicalCol,
            event.logicalRow,
            event.surfaceId
        ) else {
            return
        }
        refresh(trigger: "pointer")
    }

    func handlePointerScroll(deltaX: CGFloat, deltaY: CGFloat, precise: Bool) {
        handleScroll(deltaX: deltaX, deltaY: deltaY, precise: precise)
    }

    func activePaneRect() -> Rect? {
        let activePaneId = framePlan.active_pane_id()
        let paneCount = Int(framePlan.pane_count())
        guard paneCount > 0 else { return nil }
        for index in 0..<paneCount {
            let pane = framePlan.pane_at(UInt(index))
            if pane.pane_id() == activePaneId {
                return pane.rect()
            }
        }
        return nil
    }

    func resizeSplit(splitId: UInt64, pixelPoint: CGPoint) {
        let col = max(0, Int((pixelPoint.x / max(1, cellSize.width)).rounded()))
        let row = max(0, Int((pixelPoint.y / max(1, cellSize.height)).rounded()))
        let x = UInt16(clamping: col)
        let y = UInt16(clamping: row)
        guard app.resize_split(editorId, splitId, x, y) else {
            return
        }
        refresh(trigger: "split_resize_drag")
    }

    func paneDragPreviewTitle(for paneId: UInt64) -> String {
        if let editorSurface = editorSurfacePanes.first(where: { $0.paneId == paneId }) {
            return normalizeFallbackTitle(editorSurface.title)
        }
        if let terminalPane = terminalPanes.first(where: { $0.paneId == paneId }) {
            let metadata = GhosttyRuntime.shared.terminalMetadata(
                runtimeId: runtimeInstanceId,
                for: terminalPane.terminalId
            )
            return terminalPresentationTitle(from: metadata)
        }
        return "pane"
    }

    @discardableResult
    func movePane(sourcePaneId: UInt64, destinationPaneId: UInt64, directionRaw: UInt8) -> Bool {
        guard app.move_pane(editorId, sourcePaneId, destinationPaneId, directionRaw) else {
            return false
        }
        refresh(trigger: "pane_move_drag")
        return true
    }

    @discardableResult
    private func splitActivePane(axis: PaneSplitAxis) -> Bool {
        let wasTerminal = isActivePaneTerminal
        guard app.split_active_pane(editorId, axis.rawValue) else {
            return false
        }
        if wasTerminal {
            _ = app.open_terminal_in_active_pane(editorId)
        }
        refresh(trigger: "pane_split")
        return true
    }

    @discardableResult
    private func focusPane(direction: PaneFocusDirection) -> Bool {
        guard app.jump_active_pane(editorId, direction.rawValue) else {
            return false
        }
        refresh(trigger: "pane_focus")
        return true
    }

    @discardableResult
    func openTerminalInActivePane() -> Bool {
        guard app.open_terminal_in_active_pane(editorId) else {
            return false
        }
        refresh(trigger: "terminal_open")
        return true
    }

    @discardableResult
    func closeTerminalInActivePane() -> Bool {
        guard app.close_terminal_in_active_pane(editorId) else {
            return false
        }
        refresh(trigger: "terminal_close")
        return true
    }

    @discardableResult
    func hideActiveTerminalSurface() -> Bool {
        guard app.hide_active_terminal_surface(editorId) else {
            return false
        }
        refresh(trigger: "terminal_hide")
        return true
    }

    func handleTerminalMetadataUpdate(terminalId: UInt64) {
        guard terminalSurfaces.contains(where: { $0.terminalId == terminalId }) else {
            return
        }
        refresh(trigger: "terminal_metadata")
    }

    @discardableResult
    func toggleSurfaceOverview() -> Bool {
        guard let hostWindow else {
            return false
        }
        hostWindow.toggleTabOverview(nil)
        return true
    }

    @discardableResult
    func toggleGlobalTerminalSwitcher() -> Bool {
        let anchorWindow = hostWindow ?? NSApp.keyWindow ?? NSApp.mainWindow
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "terminal.switcher.anchor runtime=\(runtimeInstanceId) editor=\(editorId.value) host=\(hostWindow?.windowNumber ?? 0) key=\(NSApp.keyWindow?.windowNumber ?? 0) main=\(NSApp.mainWindow?.windowNumber ?? 0) resolved=\(anchorWindow?.windowNumber ?? 0)"
            )
        }
        let entries = EditorCommandModelRegistry.shared.globalTerminalEntries(anchorWindow: anchorWindow)
        let opened = GlobalTerminalSwitcherController.shared.toggle(
            ownerWindow: anchorWindow,
            items: entries
        )
        if DiagnosticsDebugLog.enabled, let anchorWindow {
            DiagnosticsDebugLog.log(
                "terminal.switcher.toggle runtime=\(runtimeInstanceId) editor=\(editorId.value) window=\(anchorWindow.windowNumber) entries=\(entries.count) opened=\(opened ? 1 : 0)"
            )
        }
        return opened
    }

    func closeGlobalTerminalSwitcher() {
        GlobalTerminalSwitcherController.shared.close(ownerWindow: hostWindow)
    }

    @discardableResult
    func submitGlobalTerminalSwitcher(entry: GlobalTerminalSurfaceEntry) -> Bool {
        closeGlobalTerminalSwitcher()
        return EditorCommandModelRegistry.shared.focusTerminalSurface(
            runtimeId: entry.runtimeId,
            terminalId: entry.terminalId
        )
    }

    @discardableResult
    func closeSurface() -> Bool {
        let paneCountBefore = Int(framePlan.pane_count())
        if isActivePaneTerminal {
            if paneCountBefore > 1 {
                guard closeTerminalInActivePane() else {
                    return false
                }
                let didExecuteClose = executeNamedCommand("wclose")
                let paneCountAfter = Int(framePlan.pane_count())
                if paneCountAfter < paneCountBefore || didExecuteClose {
                    return true
                }
                return false
            }
            return closeTerminalInActivePane()
        }

        guard let context = currentBufferCloseContext() else {
            return false
        }

        if context.modified {
            guard let hostWindow else {
                return false
            }
            presentCloseConfirmation(for: context, in: hostWindow) { [weak self, paneCountBefore] in
                _ = self?.closeBufferSurface(context, paneCountBefore: paneCountBefore)
            }
            return true
        }

        return closeBufferSurface(context, paneCountBefore: paneCountBefore)
    }

    @discardableResult
    private func closeBufferSurface(_ context: BufferCloseContext, paneCountBefore: Int) -> Bool {
        guard closeBufferForSurface(context) else {
            return false
        }

        guard paneCountBefore > 1 else {
            return true
        }

        let didExecuteClose = executeNamedCommand("wclose")
        let paneCountAfter = Int(framePlan.pane_count())
        if paneCountAfter < paneCountBefore || didExecuteClose {
            return true
        }
        return false
    }

    @discardableResult
    private func closeBufferForSurface(_ context: BufferCloseContext) -> Bool {
        if bufferTabsSnapshot == nil {
            let tabsFetch = fetchBufferTabsSnapshot()
            if tabsFetch.changed {
                bufferTabsSnapshot = tabsFetch.snapshot
            }
        }

        let bufferCount = bufferTabsSnapshot?.tabs.count ?? 0
        if bufferCount <= 1 {
            let fallbackBufferId = app.open_untitled_buffer(editorId)
            guard fallbackBufferId != 0 else {
                return false
            }
            refresh(trigger: "buffer_close_seed")
        }

        guard app.close_buffer_by_id(editorId, context.bufferId) else {
            refresh(trigger: "buffer_close_retry")
            return false
        }
        if boundBufferId == context.bufferId {
            boundBufferId = nil
        }
        if pendingInitialBoundBufferId == context.bufferId {
            pendingInitialBoundBufferId = nil
        }
        refresh(trigger: "buffer_closed")
        return true
    }

    @discardableResult
    private func closeBufferForWindowClose(_ context: BufferCloseContext) -> Bool {
        guard app.close_buffer_by_id(editorId, context.bufferId) else {
            refresh(trigger: "buffer_close_retry")
            return canCloseWindowWithoutBufferClose(failedBufferId: context.bufferId)
        }
        if boundBufferId == context.bufferId {
            boundBufferId = nil
        }
        if pendingInitialBoundBufferId == context.bufferId {
            pendingInitialBoundBufferId = nil
        }
        refresh(trigger: "buffer_closed")
        return true
    }

    @discardableResult
    private func focusPane(paneId: UInt64, trigger: String) -> Bool {
        guard paneId != 0 else {
            return false
        }
        let activePaneBefore = framePlan.active_pane_id()
        let event = MouseBridgeEvent(
            kind: 0,
            button: 1,
            logicalCol: UInt16.max,
            logicalRow: UInt16.max,
            modifiers: 0,
            clickCount: 1,
            surfaceId: paneId
        )
        guard app.handle_mouse(
            editorId,
            event.packed,
            event.logicalCol,
            event.logicalRow,
            event.surfaceId
        ) else {
            if DiagnosticsDebugLog.enabled {
                DiagnosticsDebugLog.log(
                    "surface_rail.focus trigger=\(trigger) requestedPane=\(paneId) beforeActivePane=\(activePaneBefore) handled=0"
                )
            }
            return false
        }
        refresh(trigger: trigger)
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "surface_rail.focus trigger=\(trigger) requestedPane=\(paneId) beforeActivePane=\(activePaneBefore) afterActivePane=\(framePlan.active_pane_id()) handled=1 responder=\(debugFirstResponderSummary())"
            )
        }
        return true
    }

    @discardableResult
    func executeNamedCommand(_ command: EditorNamedCommand) -> Bool {
        switch command {
        case .openNativeTab:
            return openNativeUntitledTab()
        case .closeSurface:
            return closeSurface()
        case .increaseFontSize:
            return resizeFonts(.increase)
        case .decreaseFontSize:
            return resizeFonts(.decrease)
        case .resetFontSize:
            return resizeFonts(.reset)
        case .splitPaneDown:
            return splitActivePane(axis: .horizontal)
        case .splitPaneRight:
            return splitActivePane(axis: .vertical)
        case .focusPaneLeft:
            return focusPane(direction: .left)
        case .focusPaneRight:
            return focusPane(direction: .right)
        case .focusPaneUp:
            return focusPane(direction: .up)
        case .focusPaneDown:
            return focusPane(direction: .down)
        case .openGlobalTerminalSwitcher:
            return toggleGlobalTerminalSwitcher()
        case .toggleSurfaceOverview:
            return toggleSurfaceOverview()
        default:
            return executeNamedCommand(command.rawValue)
        }
    }

    @discardableResult
    func executeNamedCommand(_ commandName: String) -> Bool {
        guard app.execute_command_named(editorId, commandName) else {
            return false
        }
        refresh(trigger: "named_command:\(commandName)")
        return true
    }

    @discardableResult
    private func resizeFonts(_ action: FontZoomAction) -> Bool {
        let didResizeBuffer = resizeBufferFont(action)
        let didResizeTerminals = GhosttyRuntime.shared.changeFontSize(
            runtimeId: runtimeInstanceId,
            terminalIds: Set(terminalSurfaces.map(\.terminalId)),
            action: action
        )
        return didResizeBuffer || didResizeTerminals
    }

    @discardableResult
    private func resizeBufferFont(_ action: FontZoomAction) -> Bool {
        let nextSize = action.adjustedBufferPointSize(from: bufferFontSize)
        guard abs(nextSize - bufferFontSize) > 0.001 else {
            return false
        }

        let fontInfo = FontLoader.loadBufferFont(size: nextSize)
        bufferFontSize = nextSize
        cellSize = fontInfo.cellSize
        bufferFont = fontInfo.font
        bufferNSFont = fontInfo.nsFont

        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "font resize: nsFont=\(fontInfo.nsFont.fontName) pointSize=\(fontInfo.nsFont.pointSize) cell=\(Int(cellSize.width))x\(Int(cellSize.height))"
            )
        }

        return true
    }

    private func scrollDelta(deltaX: CGFloat, deltaY: CGFloat, precise: Bool) -> (Int, Int) {
        let lineDeltaY: CGFloat
        let lineDeltaX: CGFloat

        if precise {
            lineDeltaY = scrollRemainderY + (deltaY / cellSize.height)
            lineDeltaX = scrollRemainderX + (deltaX / cellSize.width)
        } else {
            lineDeltaY = scrollRemainderY + deltaY
            lineDeltaX = scrollRemainderX + deltaX
        }

        let rows = Int(lineDeltaY.rounded(.towardZero))
        let cols = Int(lineDeltaX.rounded(.towardZero))

        scrollRemainderY = lineDeltaY - CGFloat(rows)
        scrollRemainderX = lineDeltaX - CGFloat(cols)

        // Positive deltaY means scroll up; reduce row index.
        return (-rows, -cols)
    }

    private func fetchSplitSeparators() -> [SplitSeparatorSnapshot] {
        let count = Int(app.split_separator_count(editorId))
        guard count > 0 else { return [] }
        var separators: [SplitSeparatorSnapshot] = []
        separators.reserveCapacity(count)
        for index in 0..<count {
            let separator = app.split_separator_at(editorId, UInt(index))
            separators.append(
                SplitSeparatorSnapshot(
                    splitId: separator.split_id(),
                    axis: separator.axis(),
                    line: separator.line(),
                    spanStart: separator.span_start(),
                    spanEnd: separator.span_end()
                )
            )
        }
        return separators
    }

    private func updateTerminalPaneSnapshots(from framePlan: RenderFramePlan) {
        let count = Int(framePlan.pane_count())
        let activePaneId = framePlan.active_pane_id()
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "editor.term.snapshot.begin runtime=\(runtimeInstanceId) editor=\(editorId.value) pane_count=\(count)"
            )
        }
        guard count > 0 else {
            if !terminalPanes.isEmpty {
                terminalPanes = []
            }
            if isActivePaneTerminal {
                isActivePaneTerminal = false
            }
            if DiagnosticsDebugLog.enabled {
                DiagnosticsDebugLog.log(
                    "editor.term.snapshot.empty runtime=\(runtimeInstanceId) editor=\(editorId.value)"
                )
            }
            return
        }

        var panes: [TerminalPaneSnapshot] = []
        panes.reserveCapacity(count)
        var activeTerminal = false
        for index in 0..<count {
            let pane = framePlan.pane_at(UInt(index))
            let paneKind = pane.pane_kind()
            guard paneKind == 1 else {
                continue
            }

            let terminalId = pane.terminal_id()
            guard terminalId != 0 else {
                continue
            }

            let rect = pane.rect()
            // `active_pane_id` is the authoritative focus source for Swift-side
            // pane rendering and terminal dimming.
            let isActive = pane.pane_id() == activePaneId
            if isActive {
                activeTerminal = true
            }
            panes.append(
                TerminalPaneSnapshot(
                    paneId: pane.pane_id(),
                    terminalId: terminalId,
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                    isActive: isActive
                )
            )
        }

        if panes != terminalPanes {
            terminalPanes = panes
        }
        if activeTerminal != isActivePaneTerminal {
            isActivePaneTerminal = activeTerminal
        }
        if DiagnosticsDebugLog.enabled {
            let summary = panes
                .map { "p\($0.paneId):t\($0.terminalId):a\($0.isActive ? 1 : 0)" }
                .joined(separator: ",")
            DiagnosticsDebugLog.logChanged(
                key: "editor.term.snapshot.\(runtimeInstanceId).\(editorId.value)",
                value: "count=\(panes.count) active_terminal=\(activeTerminal ? 1 : 0) panes=[\(summary)]"
            )
        }

    }

    private func updateTerminalSurfaceSnapshots() {
        let count = Int(app.terminal_surface_count(editorId))
        var snapshots: [TerminalSurfaceSnapshot] = []
        snapshots.reserveCapacity(count)

        for index in 0..<count {
            let snapshot = app.terminal_surface_at(editorId, UInt(index))
            let paneId = snapshot.pane_id()
            snapshots.append(
                TerminalSurfaceSnapshot(
                    terminalId: snapshot.terminal_id(),
                    paneId: paneId == 0 ? nil : paneId,
                    isActive: snapshot.is_active()
                )
            )
        }

        if snapshots != terminalSurfaces {
            terminalSurfaces = snapshots
        }

        if let activeSurface = snapshots.first(where: \.isActive) {
            lastFocusedTerminalSurfaceId = activeSurface.terminalId
        } else if let last = lastFocusedTerminalSurfaceId,
                  !snapshots.contains(where: { $0.terminalId == last }) {
            lastFocusedTerminalSurfaceId = nil
        }

        EditorCommandModelRegistry.shared.reconcileTerminalSurfaces(
            runtimeId: runtimeInstanceId,
            terminalIds: Set(snapshots.map(\.terminalId))
        )

        GhosttyRuntime.shared.reconcileTerminalIds(
            runtimeId: runtimeInstanceId,
            Set(snapshots.map(\.terminalId))
        )

        if DiagnosticsDebugLog.enabled {
            let summary = snapshots
                .map { "t\($0.terminalId):p\($0.paneId ?? 0):a\($0.isActive ? 1 : 0)" }
                .joined(separator: ",")
            DiagnosticsDebugLog.logChanged(
                key: "editor.term.surfaces.\(runtimeInstanceId).\(editorId.value)",
                value: "count=\(snapshots.count) surfaces=[\(summary)]"
            )
        }
    }

    private func updateEditorSurfaceSnapshots() {
        let count = Int(app.editor_surface_count(editorId))
        var snapshots: [EditorPaneSurfaceSnapshot] = []
        snapshots.reserveCapacity(count)

        for index in 0..<count {
            let snapshot = app.editor_surface_at(editorId, UInt(index))
            let rawFilePath = snapshot.file_path().toString().trimmingCharacters(in: .whitespacesAndNewlines)
            let filePath = rawFilePath.isEmpty ? nil : rawFilePath
            snapshots.append(
                EditorPaneSurfaceSnapshot(
                    paneId: snapshot.pane_id(),
                    bufferId: snapshot.buffer_id(),
                    bufferIndex: Int(snapshot.buffer_index()),
                    title: snapshot.title().toString(),
                    modified: snapshot.modified(),
                    filePath: filePath,
                    isActive: snapshot.is_active()
                )
            )
        }

        if snapshots != editorSurfacePanes {
            editorSurfacePanes = snapshots
        }
    }

    private func updateSurfaceOverviewSnapshots(from framePlan: RenderFramePlan) {
        trackLastFocusedPane(from: framePlan)

        let count = Int(framePlan.pane_count())

        var items: [PaneSurfaceSnapshot] = []
        items.reserveCapacity(count)

        for index in 0..<count {
            let pane = framePlan.pane_at(UInt(index))
            let paneId = pane.pane_id()
            if pane.pane_kind() == 1 {
                items.append(PaneSurfaceSnapshot(paneId: paneId, kind: .terminal))
                continue
            }

            items.append(PaneSurfaceSnapshot(paneId: paneId, kind: .editor))
        }

        paneSurfaceItems = items
    }

    private func trackLastFocusedPane(from framePlan: RenderFramePlan) {
        let activePaneId = framePlan.active_pane_id()
        let count = Int(framePlan.pane_count())
        for index in 0..<count {
            let pane = framePlan.pane_at(UInt(index))
            guard pane.pane_id() == activePaneId else {
                continue
            }
            if pane.pane_kind() == 1 {
                if let window = hostWindow,
                   window.isKeyWindow || window.isMainWindow,
                   let terminalId = activeTerminalId(for: pane.pane_id()) {
                    EditorCommandModelRegistry.shared.noteFocusedTerminalSurface(
                        runtimeId: runtimeInstanceId,
                        terminalId: terminalId
                    )
                }
            } else {
                lastFocusedEditorPaneId = activePaneId
                if let window = hostWindow,
                   window.isKeyWindow || window.isMainWindow {
                    EditorCommandModelRegistry.shared.noteFocusedEditorPane(
                        runtimeId: runtimeInstanceId,
                        paneId: activePaneId
                    )
                }
            }
            return
        }
    }

    private func activeTerminalId(for paneId: UInt64) -> UInt64? {
        terminalPanes.first(where: { $0.paneId == paneId && $0.isActive })?.terminalId
    }

    private func preferredTerminalSurfaceId() -> UInt64? {
        if let last = lastFocusedTerminalSurfaceId,
           terminalSurfaces.contains(where: { $0.terminalId == last }) {
            return last
        }

        return terminalSurfaces.sorted { lhs, rhs in
            if lhs.isAttached != rhs.isAttached {
                return lhs.isAttached && !rhs.isAttached
            }
            return lhs.terminalId < rhs.terminalId
        }.first?.terminalId
    }

    private func preferredEditorPaneId(excluding excludedPaneId: UInt64?) -> UInt64? {
        if let last = lastFocusedEditorPaneId,
           paneSurfaceItems.contains(where: { $0.paneId == last && $0.kind == .editor }),
           last != excludedPaneId {
            return last
        }
        return paneSurfaceItems.first(where: {
            $0.kind == .editor && $0.paneId != excludedPaneId
        })?.paneId
    }

    func preferredEditorPaneIdForGlobalToggle() -> UInt64? {
        preferredEditorPaneId(excluding: nil)
    }

    private func focusEditorAfterOpenBufferSelection() {
        let activePaneId = framePlan.active_pane_id()
        if paneSurfaceItems.contains(where: { $0.paneId == activePaneId && $0.kind == .editor }) {
            if let hostWindow {
                let reclaimed = KeyCaptureFocusBridge.shared.reclaim(in: hostWindow)
                if DiagnosticsDebugLog.enabled {
                    DiagnosticsDebugLog.log(
                        "surface_rail.buffer.reclaim targetPane=\(activePaneId) activePane=\(framePlan.active_pane_id()) reclaimed=\(reclaimed ? 1 : 0) responder=\(debugFirstResponderSummary()) mode=direct"
                    )
                }
            }
            return
        }

        let targetPaneId = preferredEditorPaneId(excluding: nil) ?? activePaneId
        if targetPaneId != 0 {
            _ = focusPane(paneId: targetPaneId, trigger: "surface_rail_buffer_focus")
        }
        if let hostWindow {
            let reclaimed = KeyCaptureFocusBridge.shared.reclaim(in: hostWindow)
            if DiagnosticsDebugLog.enabled {
                DiagnosticsDebugLog.log(
                    "surface_rail.buffer.reclaim targetPane=\(targetPaneId) activePane=\(framePlan.active_pane_id()) reclaimed=\(reclaimed ? 1 : 0) responder=\(debugFirstResponderSummary()) mode=focus_pane"
                )
            }
        }
    }

    @discardableResult
    func focusEditorSurface(paneId: UInt64) -> Bool {
        if let hostWindow {
            hostWindow.makeKeyAndOrderFront(nil)
        }
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "surface_rail.editor.request pane=\(paneId) activeBefore=\(framePlan.active_pane_id())"
            )
        }
        let focused = focusPane(paneId: paneId, trigger: "surface_rail_editor_surface")
        if focused, let hostWindow {
            DispatchQueue.main.async {
                let reclaimed = KeyCaptureFocusBridge.shared.reclaim(in: hostWindow)
                if DiagnosticsDebugLog.enabled {
                    DiagnosticsDebugLog.log(
                        "surface_rail.editor.reclaim pane=\(paneId) activeAfter=\(self.framePlan.active_pane_id()) reclaimed=\(reclaimed ? 1 : 0) responder=\(self.debugFirstResponderSummary())"
                    )
                }
            }
        }
        return focused
    }

    @discardableResult
    func focusEditorPaneForGlobalToggle(paneId: UInt64) -> Bool {
        if let hostWindow {
            hostWindow.makeKeyAndOrderFront(nil)
        }
        return focusPane(paneId: paneId, trigger: "global_editor_toggle")
    }

    func globalTerminalSurfaceEntries(isCurrentWindow: Bool) -> [GlobalTerminalSurfaceEntry] {
        guard !terminalSurfaces.isEmpty else {
            return []
        }

        let currentWindowTitle = normalizedTerminalSwitcherWindowTitle()
        let sortedSurfaces = terminalSurfaces.sorted { lhs, rhs in
            if lhs.isActive != rhs.isActive {
                return lhs.isActive && !rhs.isActive
            }
            return lhs.terminalId < rhs.terminalId
        }

        return sortedSurfaces.map { surface in
            let metadata = GhosttyRuntime.shared.terminalMetadata(runtimeId: runtimeInstanceId, for: surface.terminalId)
            let title = terminalPresentationTitle(from: metadata)
            let subtitle = terminalSwitcherSubtitle(for: surface, metadata: metadata)
            return GlobalTerminalSurfaceEntry(
                runtimeId: runtimeInstanceId,
                terminalId: surface.terminalId,
                paneId: surface.paneId,
                title: title,
                subtitle: subtitle,
                windowTitle: currentWindowTitle,
                isActive: surface.isActive,
                isAttached: surface.isAttached,
                isInCurrentWindow: isCurrentWindow
            )
        }
    }

    func surfaceRailSnapshot() -> SurfaceRailSnapshot {
        let fileItems = surfaceRailOpenFileItems()
        let terminalItems = surfaceRailOpenTerminalItems()
        var sections: [SurfaceRailSectionSnapshot] = []

        if !fileItems.isEmpty {
            sections.append(
                SurfaceRailSectionSnapshot(
                    id: "open-files",
                    title: "Files",
                    items: fileItems
                )
            )
        }

        if !terminalItems.isEmpty {
            sections.append(
                SurfaceRailSectionSnapshot(
                    id: "open-terminals",
                    title: "Terminals",
                    items: terminalItems
                )
            )
        }

        return SurfaceRailSnapshot(sections: sections)
    }

    @discardableResult
    func focusTerminalSurface(terminalId: UInt64) -> Bool {
        if let pane = terminalPanes.first(where: { $0.terminalId == terminalId }) {
            if DiagnosticsDebugLog.enabled {
                DiagnosticsDebugLog.log(
                    "terminal.switcher.focus runtime=\(runtimeInstanceId) editor=\(editorId.value) terminal=\(terminalId) mode=attached pane=\(pane.paneId)"
                )
            }
            if let hostWindow {
                hostWindow.makeKeyAndOrderFront(nil)
            }
            return focusPane(paneId: pane.paneId, trigger: "global_terminal_switcher")
        }

        guard app.focus_terminal_surface(editorId, terminalId) else {
            return false
        }
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "terminal.switcher.focus runtime=\(runtimeInstanceId) editor=\(editorId.value) terminal=\(terminalId) mode=reattach"
            )
        }
        if let hostWindow {
            hostWindow.makeKeyAndOrderFront(nil)
        }
        refresh(trigger: "global_terminal_switcher_attach")
        return true
    }

    func selectCommandPalette(index: Int) {
        _ = app.command_palette_select_filtered(editorId, UInt(index))
        refresh()
    }

    func selectOpenBuffer(bufferIndex: Int) {
        guard bufferIndex >= 0 else { return }
        if let hostWindow {
            hostWindow.makeKeyAndOrderFront(nil)
        }
        let selectedTab = targetBufferTab(forBufferIndex: bufferIndex)
        let selectedFilePath = EditorModel.normalizedFilePath(selectedTab?.filePath)
        let alreadyActiveBuffer = activeBufferTabSnapshot()?.bufferIndex == bufferIndex
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "surface_rail.buffer.request bufferIndex=\(bufferIndex) activePaneBefore=\(framePlan.active_pane_id()) activeBufferBefore=\(debugActiveBufferSummary()) alreadyActive=\(alreadyActiveBuffer ? 1 : 0) mode=\(selectedFilePath == nil ? "activate_buffer" : "open_file_path")"
            )
        }
        if alreadyActiveBuffer && !isActivePaneTerminal {
            DispatchQueue.main.async { [weak self] in
                self?.focusEditorAfterOpenBufferSelection()
            }
            return
        }
        let handled: Bool
        if let selectedFilePath {
            handled = app.open_file_path(editorId, selectedFilePath)
        } else {
            handled = app.activate_buffer_tab(editorId, UInt(bufferIndex))
        }
        guard handled else {
            if DiagnosticsDebugLog.enabled {
                DiagnosticsDebugLog.log(
                    "surface_rail.buffer.request bufferIndex=\(bufferIndex) handled=0"
                )
            }
            return
        }
        refresh(trigger: "surface_rail_open_buffer")
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "surface_rail.buffer.request bufferIndex=\(bufferIndex) handled=1 activePaneAfterRefresh=\(framePlan.active_pane_id()) activeBufferAfter=\(debugActiveBufferSummary())"
            )
        }
        DispatchQueue.main.async { [weak self] in
            self?.focusEditorAfterOpenBufferSelection()
        }
    }

    func setHostWindow(_ window: NSWindow?) {
        let previousWindow = hostWindow
        if DiagnosticsDebugLog.enabled, previousWindow !== window {
            DiagnosticsDebugLog.log(
                "editor.window.bind runtime=\(runtimeInstanceId) editor=\(editorId.value) old=\(previousWindow?.windowNumber ?? 0) new=\(window?.windowNumber ?? 0)"
            )
        }
        if previousWindow !== window {
            EditorCommandModelRegistry.shared.unregister(window: previousWindow)
            closeConfirmationAlert = nil
            lastNativeWindowPresentation = nil
            unregisterHostWindowNotifications()
            hostWindow = window
            EditorCommandModelRegistry.shared.register(window: window, model: self)
            registerHostWindowNotifications(for: window)
            seedUntitledBufferForUnroutedWindowIfNeeded()
            refresh(trigger: "window_attach")
        } else {
            hostWindow = window
            EditorCommandModelRegistry.shared.register(window: window, model: self)
        }
        syncHostWindowFocusState()
        if shouldSyncNativeWindowPresentation() {
            syncNativeWindowPresentation()
        }
    }

    func setOpenWindowTabHandler(_ handler: @escaping (EditorWindowRoute) -> Void) {
        openWindowTabHandler = handler
    }

    func currentHostWindow() -> NSWindow? {
        hostWindow
    }

    func selectNativeWindowTab(indexOneBased: Int) -> Bool {
        let handled = SwiftWindowTabsCoordinator.shared.selectNativeTab(indexOneBased: indexOneBased, around: hostWindow)
        if handled {
            DispatchQueue.main.async { [weak self] in
                self?.refresh(trigger: "native_tab_shortcut")
            }
        }
        return handled
    }

    @discardableResult
    func openNativeUntitledTab() -> Bool {
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "editor.native_new_tab.request editor=\(editorId.value) has_handler=\(openWindowTabHandler != nil ? 1 : 0)"
            )
        }
        guard let openWindowTabHandler else {
            let fallbackBuffer = app.open_untitled_buffer(editorId)
            guard fallbackBuffer != 0 else { return false }
            refresh(trigger: "native_new_tab_fallback")
            return true
        }

        SwiftWindowTabsCoordinator.shared.requestOpenUntitledTab(from: hostWindow, openWindow: openWindowTabHandler)
        refresh(trigger: "native_new_tab")
        return true
    }

    func handleWindowShouldClose(_ window: NSWindow) -> Bool {
        if allowProgrammaticWindowClose {
            allowProgrammaticWindowClose = false
            return true
        }

        guard closeConfirmationAlert == nil else {
            return false
        }

        refresh(trigger: "window_should_close")
        guard let context = currentBufferCloseContext() else {
            // No active buffer context: allow native close behavior.
            return true
        }

        guard context.modified else {
            return closeBufferForWindowClose(context)
        }

        presentCloseConfirmation(for: context, in: window) { [weak self, weak window] in
            guard let self, let window else { return }
            guard self.closeBufferForWindowClose(context) else { return }
            self.allowProgrammaticWindowClose = true
            window.performClose(nil)
        }
        return false
    }

    func submitCommandPalette(index: Int?) {
        if let index {
            _ = app.command_palette_submit_filtered(editorId, UInt(index))
        } else {
            let event = KeyEvent(kind: KeyKind.enter.rawValue, codepoint: 0, modifiers: 0)
            _ = app.handle_key(editorId, event)
        }
        refresh()
    }

    func closeCommandPalette() {
        _ = app.command_palette_close(editorId)
        refresh()
    }

    func setSearchQuery(_ query: String) {
        _ = app.search_prompt_set_query(editorId, query)
        _ = app.ensure_cursor_visible(editorId)
        refresh()
    }

    func searchPrev() {
        searchStep(character: "p")
    }

    func searchNext() {
        searchStep(character: "n")
    }

    func closeSearch() {
        _ = app.search_prompt_close(editorId)
        refresh()
    }

    func submitSearch() {
        _ = app.search_prompt_submit(editorId)
        refresh()
    }

    private func searchStep(character: Character) {
        guard let scalar = character.unicodeScalars.first else {
            return
        }
        // Ctrl-N/Ctrl-P is handled by the search prompt to step next/previous
        // match while keeping the prompt open.
        let event = KeyEvent(
            kind: KeyKind.char.rawValue,
            codepoint: scalar.value,
            modifiers: 0b0000_0001
        )
        _ = app.handle_key(editorId, event)
        _ = app.ensure_cursor_visible(editorId)
        refresh()
    }

    func setCommandPaletteQuery(_ query: String) {
        _ = app.command_palette_set_query(editorId, query)
        refresh(trigger: "command_palette_query")
    }

    // MARK: - File picker

    func setFilePickerQuery(_ query: String) {
        _ = app.file_picker_set_query(editorId, query)
        refreshFilePicker()
    }

    func submitFilePicker(index: Int) {
        _ = app.file_picker_submit(editorId, UInt(index))
        refresh(trigger: "file_picker_submit")
    }

    func filePickerSelectIndex(_ index: Int) {
        _ = app.file_picker_select_index(editorId, UInt(index))
        filePickerPreviewOffsetHint = -1
        // Only refresh the preview — don't re-serialize the full item list
        // (up to 10k items) on every arrow key press.
        refreshFilePickerPreview()
    }

    func filePickerPreviewWindowRequest(offset: Int, visibleRows: Int, overscan: Int) {
        filePickerPreviewOffsetHint = offset
        filePickerPreviewVisibleRows = max(1, visibleRows)
        filePickerPreviewOverscan = max(1, overscan)
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "picker.window_request offset=\(offset) visible=\(visibleRows) overscan=\(overscan)"
            )
        }
        refreshFilePickerPreview()
    }

    func closeFilePicker() {
        filePickerTimer?.invalidate()
        filePickerTimer = nil
        _ = app.file_picker_close(editorId)
        lastPickerQuery = nil
        lastPickerMatchedCount = -1
        lastPickerTotalCount = -1
        lastPickerScanning = false
        filePickerPreviewOffsetHint = -1
        filePickerPreviewVisibleRows = 24
        filePickerPreviewOverscan = 24
        filePickerSnapshot = nil
        filePickerPreviewModel.preview = nil
        refresh(trigger: "file_picker_close")
    }

    func refreshFilePicker() {
        let t0 = CFAbsoluteTimeGetCurrent()
        let perfT0 = DispatchTime.now().uptimeNanoseconds
        let data = app.file_picker_snapshot(editorId, 10_000)
        let perfAfterFfi = DispatchTime.now().uptimeNanoseconds

        guard data.active() else {
            stopFilePickerTimerIfNeeded()
            lastPickerQuery = nil
            lastPickerMatchedCount = -1
            lastPickerTitle = nil
            lastPickerRoot = nil
            lastPickerKind = 0
            filePickerSnapshot = nil
            if DiagnosticsDebugLog.pickerPerfEnabled {
                let ffiMs = Double(perfAfterFfi - perfT0) / 1_000_000.0
                DiagnosticsDebugLog.pickerPerfLog(
                    String(format: "list_refresh inactive ffi=%.2fms", ffiMs)
                )
            }
            return
        }

        let perfDecodeStart = DispatchTime.now().uptimeNanoseconds
        let title = data.title().toString()
        let root = data.root().toString()
        let pickerKind = data.picker_kind()
        let pickerIdentityChanged = title != lastPickerTitle
            || root != lastPickerRoot
            || pickerKind != lastPickerKind
        if pickerIdentityChanged || filePickerSnapshot == nil {
            filePickerPreviewOffsetHint = -1
            if DiagnosticsDebugLog.enabled {
                DiagnosticsDebugLog.log(
                    "picker.identity_changed title=\(title) kind=\(pickerKind) root=\(root) reset_offset_hint=1"
                )
            }
        }
        lastPickerTitle = title
        lastPickerRoot = root
        lastPickerKind = pickerKind

        let query = data.query().toString()
        let matchedCount = Int(data.matched_count())
        let totalCount = Int(data.total_count())
        let scanning = data.scanning()

        if scanning {
            startFilePickerTimerIfNeeded(scanning: true)
        } else {
            stopFilePickerTimerIfNeeded()
        }

        // Skip full item rebuild if metadata hasn't changed.
        if query == lastPickerQuery
            && matchedCount == lastPickerMatchedCount
            && totalCount == lastPickerTotalCount
            && scanning == lastPickerScanning {
            // Only poll the preview while matches are still streaming in.
            let perfDecodeEnd = DispatchTime.now().uptimeNanoseconds
            let refreshedPreview = scanning
            if refreshedPreview {
                refreshFilePickerPreview()
            }
            if DiagnosticsDebugLog.pickerPerfEnabled {
                let ffiMs = Double(perfAfterFfi - perfT0) / 1_000_000.0
                let decodeMs = Double(perfDecodeEnd - perfDecodeStart) / 1_000_000.0
                let totalMs = Double(DispatchTime.now().uptimeNanoseconds - perfT0) / 1_000_000.0
                DiagnosticsDebugLog.pickerPerfLog(
                    String(
                        format: "list_refresh skip kind=%d matched=%d total=%d scanning=%d preview_refreshed=%d ffi=%.2fms decode=%.2fms total=%.2fms",
                        Int(pickerKind),
                        matchedCount,
                        totalCount,
                        scanning ? 1 : 0,
                        refreshedPreview ? 1 : 0,
                        ffiMs,
                        decodeMs,
                        totalMs
                    )
                )
            }
            let t1 = CFAbsoluteTimeGetCurrent()
            debugUiLog(String(format: "refreshFilePicker SKIP items=%d elapsed=%.2fms", matchedCount, (t1 - t0) * 1000))
            return
        }

        lastPickerQuery = query
        lastPickerMatchedCount = matchedCount
        lastPickerTotalCount = totalCount
        lastPickerScanning = scanning

        let itemCount = Int(data.item_count())
        var items = [FilePickerItemSnapshot]()
        items.reserveCapacity(itemCount)
        for i in 0..<itemCount {
            let item = data.item_at(UInt(i))
            let miCount = Int(item.match_index_count())
            var matchIndices = [Int]()
            matchIndices.reserveCapacity(miCount)
            for j in 0..<miCount {
                matchIndices.append(Int(item.match_index_at(UInt(j))))
            }
            let iconStr = item.icon().toString()
            items.append(FilePickerItemSnapshot(
                id: i,
                display: item.display().toString(),
                isDir: item.is_dir(),
                icon: iconStr.isEmpty ? nil : iconStr,
                matchIndices: matchIndices,
                rowKind: item.row_kind(),
                severity: item.severity(),
                primary: item.primary().toString(),
                secondary: item.secondary().toString(),
                tertiary: item.tertiary().toString(),
                quaternary: item.quaternary().toString(),
                line: Int(item.line()),
                column: Int(item.column()),
                depth: Int(item.depth())
            ))
        }

        filePickerSnapshot = FilePickerSnapshot(
            active: true,
            title: title,
            pickerKind: pickerKind,
            query: query,
            matchedCount: matchedCount,
            totalCount: totalCount,
            scanning: scanning,
            root: root,
            items: items
        )
        let perfDecodeEnd = DispatchTime.now().uptimeNanoseconds
        refreshFilePickerPreview()

        if DiagnosticsDebugLog.pickerPerfEnabled {
            let ffiMs = Double(perfAfterFfi - perfT0) / 1_000_000.0
            let decodeMs = Double(perfDecodeEnd - perfDecodeStart) / 1_000_000.0
            let totalMs = Double(DispatchTime.now().uptimeNanoseconds - perfT0) / 1_000_000.0
            DiagnosticsDebugLog.pickerPerfLog(
                String(
                    format: "list_refresh full kind=%d items=%d matched=%d total=%d scanning=%d ffi=%.2fms decode=%.2fms total=%.2fms",
                    Int(pickerKind),
                    itemCount,
                    matchedCount,
                    totalCount,
                    scanning ? 1 : 0,
                    ffiMs,
                    decodeMs,
                    totalMs
                )
            )
        }

        let t1 = CFAbsoluteTimeGetCurrent()
        debugUiLog(String(format: "refreshFilePicker items=%d elapsed=%.2fms", itemCount, (t1 - t0) * 1000))
    }

    /// Lightweight refresh — direct FFI, no JSON. Called on selection change.
    func refreshFilePickerPreview() {
        let offset: UInt
        if filePickerPreviewOffsetHint < 0 {
            offset = UInt.max
        } else {
            offset = UInt(max(0, filePickerPreviewOffsetHint))
        }
        let perfT0 = DispatchTime.now().uptimeNanoseconds
        let preview = app.file_picker_preview_window(
            editorId,
            offset,
            UInt(max(1, filePickerPreviewVisibleRows)),
            UInt(max(1, filePickerPreviewOverscan))
        )
        let perfAfterFfi = DispatchTime.now().uptimeNanoseconds
        debugLogFilePickerPreview(preview: preview, requestedOffset: offset)
        let snapshot = FilePickerPreviewSnapshot(preview: preview)
        let perfAfterDecode = DispatchTime.now().uptimeNanoseconds
        filePickerPreviewModel.preview = snapshot
        let perfAfterPublish = DispatchTime.now().uptimeNanoseconds

        if DiagnosticsDebugLog.pickerPerfEnabled {
            let ffiMs = Double(perfAfterFfi - perfT0) / 1_000_000.0
            let decodeMs = Double(perfAfterDecode - perfAfterFfi) / 1_000_000.0
            let publishMs = Double(perfAfterPublish - perfAfterDecode) / 1_000_000.0
            let totalMs = Double(perfAfterPublish - perfT0) / 1_000_000.0
            let requested = offset == UInt.max ? "focus" : String(offset)
            DiagnosticsDebugLog.pickerPerfLog(
                String(
                    format: "preview_refresh req=%@ kind=%d visible=%d overscan=%d returned_offset=%d window_start=%d lines=%d total=%d ffi=%.2fms decode=%.2fms publish=%.2fms total=%.2fms",
                    requested,
                    Int(snapshot.kind),
                    max(1, filePickerPreviewVisibleRows),
                    max(1, filePickerPreviewOverscan),
                    snapshot.offset,
                    snapshot.windowStart,
                    snapshot.lines.count,
                    snapshot.totalLines,
                    ffiMs,
                    decodeMs,
                    publishMs,
                    totalMs
                )
            )
        }
    }

    // MARK: - File tree

    func fileTreeSetExpanded(path: String, expanded: Bool) {
        guard !path.isEmpty else {
            return
        }
        guard app.file_tree_set_expanded(editorId, path, expanded) else {
            return
        }
        refreshFileTree(force: true)
    }

    func fileTreeSelectPath(path: String) {
        guard !path.isEmpty else {
            return
        }
        guard app.file_tree_select_path(editorId, path) else {
            return
        }
        refreshFileTree(force: true)
    }

    func fileTreeOpenSelected() {
        guard app.file_tree_open_selected(editorId) else {
            return
        }
        refresh(trigger: "file_tree_open_selected")
    }

    func refreshFileTree(force: Bool = false) {
        let data = app.file_tree_snapshot(editorId, 10_000)
        let visible = data.visible()
        let mode = data.mode()
        let root = data.root().toString()
        let generation = data.refresh_generation()
        let nodeCount = Int(data.node_count())

        if !force
            && generation == lastFileTreeRefreshGeneration
            && visible == lastFileTreeVisible
            && mode == lastFileTreeMode
            && root == lastFileTreeRoot
            && nodeCount == lastFileTreeNodeCount {
            return
        }

        let selectedPathRaw = data.selected_path().toString()
        let selectedPath = selectedPathRaw.isEmpty ? nil : selectedPathRaw
        var nodes: [FileTreeNodeSnapshot] = []
        nodes.reserveCapacity(nodeCount)
        for index in 0..<nodeCount {
            let node = data.node_at(UInt(index))
            nodes.append(
                FileTreeNodeSnapshot(
                    id: node.id().toString(),
                    path: node.path().toString(),
                    name: node.name().toString(),
                    depth: Int(node.depth()),
                    isDirectory: node.kind() == 1,
                    expanded: node.expanded(),
                    selected: node.selected(),
                    hasUnloadedChildren: node.has_unloaded_children()
                )
            )
        }

        fileTreeSnapshot = FileTreeSnapshot(
            visible: visible,
            mode: mode,
            root: root,
            selectedPath: selectedPath,
            refreshGeneration: generation,
            nodes: nodes
        )
        lastFileTreeRefreshGeneration = generation
        lastFileTreeVisible = visible
        lastFileTreeMode = mode
        lastFileTreeRoot = root
        lastFileTreeNodeCount = nodeCount
    }

    private func startFilePickerTimerIfNeeded(scanning: Bool) {
        guard filePickerTimer == nil, scanning else { return }
        filePickerTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30.0, repeats: true) { [weak self] _ in
            self?.refreshFilePicker()
        }
    }

    private func stopFilePickerTimerIfNeeded() {
        filePickerTimer?.invalidate()
        filePickerTimer = nil
    }

    private func startBackgroundTimerIfNeeded() {
        guard backgroundTimer == nil else { return }
        backgroundTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30.0, repeats: true) { [weak self] _ in
            guard let self else { return }
            guard self.shouldDriveSharedRuntime() else { return }
            if self.app.poll_background(self.editorId) {
                self.refresh(trigger: "background")
            }
        }
    }

    private struct UiTreeFetchResult {
        let tree: UiTreeSnapshot
        let changed: Bool
    }

    private struct BufferTabsFetchResult {
        let snapshot: BufferTabsSnapshot?
        let changed: Bool
    }

    private func fetchUiTree() -> UiTreeFetchResult {
        let json = app.ui_tree_json(editorId).toString()
        if let lastUiTreeJson, lastUiTreeJson == json {
            return UiTreeFetchResult(tree: uiTree, changed: false)
        }
        guard let data = json.data(using: .utf8) else {
            debugUiLog("ui_tree_json is not valid utf8")
            lastUiTreeJson = nil
            return UiTreeFetchResult(tree: uiTree, changed: false)
        }

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        do {
            let tree = try decoder.decode(UiTreeSnapshot.self, from: data)
            lastUiTreeJson = json
            debugUiLog("ui_tree decoded overlays=\(tree.overlays.count)")
            if DiagnosticsDebugLog.enabled {
                let overlayIds = tree.overlays.compactMap { node -> String? in
                    if case .panel(let panel) = node {
                        return panel.id
                    }
                    return nil
                }
                let completion = tree.completionSnapshot()
                let diagnostic = tree.diagnosticSnapshot()
                let hover = tree.hoverSnapshot()
                let docsPopoverCount = tree.docsPopoverSnapshots().count
                let signature = tree.signatureHelpSnapshot()
                DiagnosticsDebugLog.logChanged(
                    key: "editor.popup.tree.runtime\(runtimeInstanceId).editor\(editorId.value)",
                    value: "overlays=[\(overlayIds.joined(separator: ","))] completion_items=\(completion?.items.count ?? 0) completion_selected=\(completion?.selectedIndex ?? -1) diagnostic=\(diagnostic != nil ? 1 : 0) hover=\(hover != nil ? 1 : 0) docs_count=\(docsPopoverCount) signature=\(signature != nil ? 1 : 0)"
                )
            }
            return UiTreeFetchResult(tree: tree, changed: true)
        } catch {
            lastUiTreeJson = nil
            let hasPalette = json.contains("command_palette")
            debugUiLog("ui_tree decode failed: \(error)")
            debugUiLog("ui_tree json prefix: \(String(json.prefix(400)))")
            debugUiLog("ui_tree json contains command_palette=\(hasPalette)")
            return UiTreeFetchResult(tree: uiTree, changed: false)
        }
    }

    private func fetchBufferTabsSnapshot() -> BufferTabsFetchResult {
        let json = app.buffer_tabs_snapshot_json(editorId).toString()
        if let lastBufferTabsJson, lastBufferTabsJson == json {
            return BufferTabsFetchResult(snapshot: bufferTabsSnapshot, changed: false)
        }
        guard json != "null", let data = json.data(using: .utf8) else {
            let changed = bufferTabsSnapshot != nil || lastBufferTabsJson != json
            lastBufferTabsJson = json
            return BufferTabsFetchResult(snapshot: nil, changed: changed)
        }

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        do {
            let snapshot = try decoder.decode(BufferTabsSnapshot.self, from: data)
            lastBufferTabsJson = json
            return BufferTabsFetchResult(snapshot: snapshot, changed: true)
        } catch {
            lastBufferTabsJson = nil
            debugUiLog("buffer_tabs decode failed: \(error)")
            return BufferTabsFetchResult(snapshot: bufferTabsSnapshot, changed: false)
        }
    }

    private func fetchPendingKeys() -> [String] {
        let json = app.pending_keys_json(editorId).toString()
        guard let data = json.data(using: .utf8),
              let keys = try? JSONDecoder().decode([String].self, from: data) else {
            return []
        }
        return keys
    }

    private func fetchPendingKeyHints() -> PendingKeyHintsSnapshot? {
        let json = app.pending_key_hints_json(editorId).toString()
        guard json != "null",
              let data = json.data(using: .utf8) else {
            return nil
        }

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try? decoder.decode(PendingKeyHintsSnapshot.self, from: data)
    }

    private func shouldDriveSharedRuntime() -> Bool {
        guard let hostWindow else {
            return true
        }
        if let tabGroup = hostWindow.tabGroup {
            if tabGroup.selectedWindow === hostWindow {
                return true
            }
        }
        return hostWindow.isKeyWindow || hostWindow.isMainWindow
    }

    private func shouldSyncNativeWindowPresentation() -> Bool {
        shouldDriveSharedRuntime()
    }

    private func seedUntitledBufferForUnroutedWindowIfNeeded() {
        guard !seededUntitledForUnroutedWindow else { return }
        seededUntitledForUnroutedWindow = true
    }

    private func updateBoundBufferIdFromActiveBufferIfNeeded() {
        guard let activeTab = activeBufferTabSnapshot() else {
            return
        }
        if boundBufferId != activeTab.bufferId
            || pendingInitialBoundBufferId != nil
            || pendingInitialBoundFilePath != nil {
            boundBufferId = activeTab.bufferId
            pendingInitialBoundBufferId = nil
            pendingInitialBoundFilePath = nil
        }
    }

    private func activateBoundBufferForFocusedWindow(trigger: String) {
        refresh(trigger: trigger)
    }

    private func publishBoundBufferIdToWindowCoordinator() {
        _ = boundBufferId
    }

    private func registerHostWindowNotifications(for window: NSWindow?) {
        guard let window else { return }
        let center = NotificationCenter.default
        let names: [Notification.Name] = [
            NSWindow.didBecomeKeyNotification,
            NSWindow.didBecomeMainNotification,
            NSWindow.didResignKeyNotification,
            NSWindow.didResignMainNotification
        ]
        hostWindowNotificationTokens = names.map { name in
            center.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                self?.syncHostWindowFocusState()
                if name == NSWindow.didBecomeKeyNotification || name == NSWindow.didBecomeMainNotification {
                    self?.activateBoundBufferForFocusedWindow(trigger: "window_focus")
                }
            }
        }
    }

    private func unregisterHostWindowNotifications() {
        let center = NotificationCenter.default
        for token in hostWindowNotificationTokens {
            center.removeObserver(token)
        }
        hostWindowNotificationTokens.removeAll()
    }

    private func syncHostWindowFocusState() {
        let isFocused = hostWindow?.isKeyWindow == true || hostWindow?.isMainWindow == true
        guard isHostWindowFocused != isFocused else {
            return
        }
        isHostWindowFocused = isFocused
    }

    private func updateNativeTabOpenGatewayState() {
        if nativeTabGatewayRegistered {
            runtime.releaseNativeTabGateway()
            nativeTabGatewayRegistered = false
        }
    }

    private func processNativeTabOpenRequests() {
        return
    }

    private func syncNativeWindowPresentation() {
        guard let hostWindow else { return }
        let activeTab = activeBufferTabSnapshot()
        let activeTerminal = terminalPanes.first(where: \.isActive)
        let activeTerminalMetadata = activeTerminal.flatMap { pane in
            GhosttyRuntime.shared.terminalMetadata(runtimeId: runtimeInstanceId, for: pane.terminalId)
        }
        let activeFilePathString = app.active_file_path(editorId).toString()
        let activeFileURL: URL? = {
            guard !activeFilePathString.isEmpty else { return nil }
            return URL(fileURLWithPath: activeFilePathString)
        }()
        let presentation: NativeWindowPresentation

        if isActivePaneTerminal {
            let title = terminalPresentationTitle(from: activeTerminalMetadata)
            let representedPath = activeTerminalMetadata.flatMap { metadata in
                representedPathFromTerminalPwd(metadata.pwd)
            }
            presentation = NativeWindowPresentation(
                title: title,
                subtitle: "",
                representedFilePath: representedPath,
                isDocumentEdited: false
            )
        } else if let activeTab {
            let titleFromPath = activeFileURL?.lastPathComponent
            let titleSource = (titleFromPath?.isEmpty == false) ? titleFromPath! : activeTab.title
            let title = normalizeFallbackTitle(titleSource)
            let subtitle: String
            if let activeFileURL {
                subtitle = activeFileURL.deletingLastPathComponent().lastPathComponent
            } else {
                subtitle = activeTab.directoryHint ?? ""
            }
            let representedPath: String?
            if let activeFileURL {
                representedPath = activeFileURL.path
            } else if let filePath = activeTab.filePath, !filePath.isEmpty {
                representedPath = filePath
            } else {
                representedPath = nil
            }
            presentation = NativeWindowPresentation(
                title: title,
                subtitle: subtitle,
                representedFilePath: representedPath,
                isDocumentEdited: activeTab.modified
            )
        } else if let activeFileURL {
            presentation = NativeWindowPresentation(
                title: normalizeFallbackTitle(activeFileURL.lastPathComponent),
                subtitle: activeFileURL.deletingLastPathComponent().lastPathComponent,
                representedFilePath: activeFileURL.path,
                isDocumentEdited: false
            )
        } else if let initialFilePath {
            let url = URL(fileURLWithPath: initialFilePath)
            presentation = NativeWindowPresentation(
                title: normalizeFallbackTitle(url.lastPathComponent),
                subtitle: url.deletingLastPathComponent().lastPathComponent,
                representedFilePath: url.path,
                isDocumentEdited: false
            )
        } else {
            presentation = NativeWindowPresentation(
                title: "untitled",
                subtitle: "",
                representedFilePath: nil,
                isDocumentEdited: false
            )
        }

        if DiagnosticsDebugLog.enabled {
            let tabTitle = activeTab?.title ?? "<nil>"
            let tabPath = activeTab?.filePath ?? "<nil>"
            let terminalTitle = activeTerminalMetadata?.title ?? "<nil>"
            let terminalPwd = activeTerminalMetadata?.pwd ?? "<nil>"
            let terminalSeenTitle = activeTerminalMetadata?.seenTitle == true ? "1" : "0"
            let keyWindow = hostWindow.isKeyWindow ? "1" : "0"
            let mainWindow = hostWindow.isMainWindow ? "1" : "0"
            let tabSelected = (hostWindow.tabGroup?.selectedWindow === hostWindow) ? "1" : "0"
            DiagnosticsDebugLog.logChanged(
                key: "window.presentation.input",
                value: "active_path=\(debugTruncate(activeFilePathString, limit: 160)) tab_title=\(debugTruncate(tabTitle, limit: 80)) tab_path=\(debugTruncate(tabPath, limit: 160)) term_seen_title=\(terminalSeenTitle) term_title=\(debugTruncate(terminalTitle, limit: 120)) term_pwd=\(debugTruncate(terminalPwd, limit: 160)) computed_title=\(debugTruncate(presentation.title, limit: 80)) host_title=\(debugTruncate(hostWindow.title, limit: 80)) key=\(keyWindow) main=\(mainWindow) tab_selected=\(tabSelected)"
            )
        }

        let representedPath = hostWindow.representedURL?.path
        let windowMatchesPresentation = hostWindow.title == presentation.title
            && hostWindow.subtitle == ""
            && hostWindow.isDocumentEdited == presentation.isDocumentEdited
            && representedPath == presentation.representedFilePath

        if lastNativeWindowPresentation == presentation && windowMatchesPresentation {
            return
        }
        lastNativeWindowPresentation = presentation
        if navigationTitle != presentation.title {
            navigationTitle = presentation.title
        }

        hostWindow.title = presentation.title
        hostWindow.subtitle = ""
        hostWindow.isDocumentEdited = presentation.isDocumentEdited
        if let representedPath = presentation.representedFilePath {
            hostWindow.representedURL = URL(fileURLWithPath: representedPath)
        } else {
            hostWindow.representedURL = nil
        }

        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.logChanged(
                key: "window.presentation.output",
                value: "host_title=\(debugTruncate(hostWindow.title, limit: 80)) host_subtitle=\(debugTruncate(hostWindow.subtitle, limit: 80)) represented=\(debugTruncate(hostWindow.representedURL?.path ?? "<nil>", limit: 160)) edited=\(hostWindow.isDocumentEdited ? "1" : "0")"
            )
        }

        SwiftWindowTabsCoordinator.shared.windowPresentationDidChange(hostWindow)
    }

    private func activeBufferTabSnapshot() -> BufferTabItemSnapshot? {
        guard let snapshot = bufferTabsSnapshot else { return nil }
        if let activeIndex = snapshot.activeBufferIndex {
            return snapshot.tabs.first(where: { $0.bufferIndex == activeIndex })
        }
        if let activeTab = snapshot.activeTab, snapshot.tabs.indices.contains(activeTab) {
            return snapshot.tabs[activeTab]
        }
        return snapshot.tabs.first(where: { $0.isActive }) ?? snapshot.tabs.first
    }

    private func bufferIndexForBufferId(_ bufferId: UInt64) -> Int? {
        if let snapshot = bufferTabsSnapshot,
           let match = snapshot.tabs.first(where: { $0.bufferId == bufferId }) {
            return match.bufferIndex
        }

        let json = app.buffer_tabs_snapshot_json(editorId).toString()
        guard json != "null",
              let data = json.data(using: .utf8) else {
            return nil
        }
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        guard let snapshot = try? decoder.decode(BufferTabsSnapshot.self, from: data) else {
            return nil
        }
        bufferTabsSnapshot = snapshot
        lastBufferTabsJson = json
        return snapshot.tabs.first(where: { $0.bufferId == bufferId })?.bufferIndex
    }

    private func activateBufferIdIfNeeded(_ bufferId: UInt64) -> Bool {
        if let active = activeBufferTabSnapshot(), active.bufferId == bufferId {
            return true
        }
        guard let bufferIndex = bufferIndexForBufferId(bufferId) else {
            return false
        }
        return app.activate_buffer_tab(editorId, UInt(bufferIndex))
    }

    private struct BufferCloseContext {
        let bufferId: UInt64
        let title: String
        let modified: Bool
    }

    private func currentBufferCloseContext() -> BufferCloseContext? {
        if bufferTabsSnapshot == nil {
            let tabsFetch = fetchBufferTabsSnapshot()
            if tabsFetch.changed {
                bufferTabsSnapshot = tabsFetch.snapshot
            }
        }

        let targetBufferId = pendingInitialBoundBufferId ?? boundBufferId
        if let targetBufferId,
           let tab = targetBufferTab(forBufferId: targetBufferId) {
            return BufferCloseContext(
                bufferId: targetBufferId,
                title: displayTitle(fromPath: tab.filePath) ?? normalizeFallbackTitle(tab.title),
                modified: tab.modified
            )
        }

        guard let fallbackTab = activeBufferTabSnapshot() ?? bufferTabsSnapshot?.tabs.first else {
            return nil
        }
        return BufferCloseContext(
            bufferId: fallbackTab.bufferId,
            title: displayTitle(fromPath: fallbackTab.filePath) ?? normalizeFallbackTitle(fallbackTab.title),
            modified: fallbackTab.modified
        )
    }

    private func targetBufferTab(forBufferId bufferId: UInt64) -> BufferTabItemSnapshot? {
        guard let snapshot = bufferTabsSnapshot else {
            return nil
        }
        return snapshot.tabs.first(where: { $0.bufferId == bufferId })
    }

    private func targetBufferTab(forBufferIndex bufferIndex: Int) -> BufferTabItemSnapshot? {
        if let snapshot = bufferTabsSnapshot,
           let tab = snapshot.tabs.first(where: { $0.bufferIndex == bufferIndex }) {
            return tab
        }

        let tabsFetch = fetchBufferTabsSnapshot()
        if tabsFetch.changed {
            bufferTabsSnapshot = tabsFetch.snapshot
        }
        return tabsFetch.snapshot?.tabs.first(where: { $0.bufferIndex == bufferIndex })
    }

    private static func normalizedFilePath(_ path: String?) -> String? {
        guard let path, !path.isEmpty else { return nil }
        let url = URL(fileURLWithPath: path).standardizedFileURL.resolvingSymlinksInPath()
        return url.path
    }

    private func displayTitle(fromPath path: String?) -> String? {
        guard let path, !path.isEmpty else { return nil }
        let value = URL(fileURLWithPath: path).lastPathComponent
        return value.isEmpty ? nil : value
    }

    private func surfaceRailOpenFileItems() -> [SurfaceRailItemSnapshot] {
        (bufferTabsSnapshot?.tabs ?? []).map { tab in
            SurfaceRailItemSnapshot(
                id: "buffer:\(tab.bufferId)",
                kind: .buffer,
                title: normalizeFallbackTitle(tab.title),
                subtitle: surfaceRailOpenBufferSubtitle(for: tab),
                isActive: tab.isActive && !isActivePaneTerminal,
                isModified: tab.modified,
                statusText: nil,
                paneId: nil,
                bufferId: tab.bufferId,
                bufferIndex: tab.bufferIndex,
                terminalId: nil,
                canClose: true
            )
        }
    }

    private func surfaceRailOpenTerminalItems() -> [SurfaceRailItemSnapshot] {
        terminalSurfaces
            .map { surface in
                let metadata = GhosttyRuntime.shared.terminalMetadata(runtimeId: runtimeInstanceId, for: surface.terminalId)
                return SurfaceRailItemSnapshot(
                    id: "terminal:\(surface.terminalId)",
                    kind: .terminal,
                    title: terminalPresentationTitle(from: metadata),
                    subtitle: terminalSwitcherSubtitle(for: surface, metadata: metadata),
                    isActive: surface.isActive,
                    isModified: false,
                    statusText: nil,
                    paneId: surface.paneId,
                    bufferId: nil,
                    bufferIndex: nil,
                    terminalId: surface.terminalId,
                    canClose: true
                )
            }
    }

    @discardableResult
    func closeSurfaceRailBuffer(bufferId: UInt64) -> Bool {
        guard let tab = targetBufferTab(forBufferId: bufferId) else {
            refresh(trigger: "surface_rail_buffer_close_missing")
            return false
        }

        let context = BufferCloseContext(
            bufferId: bufferId,
            title: displayTitle(fromPath: tab.filePath) ?? normalizeFallbackTitle(tab.title),
            modified: tab.modified
        )

        if context.modified, let hostWindow {
            let alert = NSAlert()
            alert.messageText = "Close Buffer Without Saving?"
            alert.informativeText = "\"\(context.title)\" has unsaved changes. If you close the buffer the changes will be lost."
            alert.addButton(withTitle: "Close")
            alert.addButton(withTitle: "Cancel")
            alert.alertStyle = .warning
            alert.beginSheetModal(for: hostWindow) { [weak self] response in
                guard let self, response == .alertFirstButtonReturn else { return }
                _ = self.closeBufferForWindowClose(context)
            }
            return true
        }

        return closeBufferForWindowClose(context)
    }

    @discardableResult
    func closeSurfaceRailTerminal(terminalId: UInt64) -> Bool {
        guard focusTerminalSurface(terminalId: terminalId) else {
            return false
        }
        return closeSurface()
    }

    private func surfaceRailOpenBufferSubtitle(for tab: BufferTabItemSnapshot) -> String? {
        let hint = tab.directoryHint?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        if !hint.isEmpty {
            return hint
        }

        guard let filePath = tab.filePath?.trimmingCharacters(in: .whitespacesAndNewlines),
              !filePath.isEmpty else {
            return nil
        }

        let directory = URL(fileURLWithPath: filePath).deletingLastPathComponent().path
        guard !directory.isEmpty, directory != "/" else {
            return nil
        }

        return (directory as NSString).abbreviatingWithTildeInPath
    }

    private func normalizeFallbackTitle(_ title: String) -> String {
        let trimmed = title.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty || trimmed == "<untitled>" {
            return "untitled"
        }
        return trimmed
    }

    private func terminalPresentationTitle(from metadata: GhosttyTerminalMetadata?) -> String {
        guard let metadata else {
            if DiagnosticsDebugLog.enabled {
                DiagnosticsDebugLog.log(
                    "editor.term.title_fallback editor=\(editorId.value) reason=no_metadata"
                )
            }
            return "terminal"
        }

        let title = metadata.title.trimmingCharacters(in: .whitespacesAndNewlines)
        if metadata.seenTitle, !title.isEmpty {
            return title
        }

        let pwd = metadata.pwd.trimmingCharacters(in: .whitespacesAndNewlines)
        if let representedPath = representedPathFromTerminalPwd(pwd), !representedPath.isEmpty {
            return (representedPath as NSString).abbreviatingWithTildeInPath
        }
        if !title.isEmpty {
            return title
        }
        return "terminal"
    }

    private func terminalSwitcherSubtitle(
        for surface: TerminalSurfaceSnapshot,
        metadata: GhosttyTerminalMetadata?
    ) -> String {
        let rawPath = metadata.flatMap { representedPathFromTerminalPwd($0.pwd) } ?? ""
        let abbreviatedPath = rawPath.isEmpty ? "" : (rawPath as NSString).abbreviatingWithTildeInPath
        let paneLabel = surface.paneId.map { "p\($0)" } ?? "detached"
        if abbreviatedPath.isEmpty {
            return "t\(surface.terminalId) • \(paneLabel)"
        }
        return "\(abbreviatedPath) • t\(surface.terminalId) • \(paneLabel)"
    }

    private func abbreviatedPath(_ path: String?) -> String? {
        guard let path = path?.trimmingCharacters(in: .whitespacesAndNewlines),
              !path.isEmpty else {
            return nil
        }
        return (path as NSString).abbreviatingWithTildeInPath
    }

    private func normalizedTerminalSwitcherWindowTitle() -> String {
        let rawTitle = hostWindow?.title ?? navigationTitle
        let trimmed = rawTitle.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            return "window"
        }
        return normalizeFallbackTitle(trimmed)
    }

    private func representedPathFromTerminalPwd(_ rawPwd: String) -> String? {
        let trimmed = rawPwd.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return nil
        }

        if let url = URL(string: trimmed), url.isFileURL {
            let path = url.path
            return path.isEmpty ? nil : path
        }

        if trimmed.hasPrefix("/") {
            return trimmed
        }

        return nil
    }

    private func presentCloseConfirmation(
        for context: BufferCloseContext,
        in window: NSWindow,
        onConfirm: @escaping () -> Void
    ) {
        let alert = NSAlert()
        alert.messageText = "Close Buffer Without Saving?"
        alert.informativeText = "\"\(context.title)\" has unsaved changes. If you close the buffer the changes will be lost."
        alert.addButton(withTitle: "Close")
        alert.addButton(withTitle: "Cancel")
        alert.alertStyle = .warning
        alert.beginSheetModal(for: window) { [weak self] response in
            guard let self else { return }
            let alertWindow = alert.window
            self.closeConfirmationAlert = nil
            guard response == .alertFirstButtonReturn else { return }
            alertWindow.orderOut(nil)
            onConfirm()
        }
        closeConfirmationAlert = alert
    }

    private func canCloseWindowWithoutBufferClose(failedBufferId: UInt64) -> Bool {
        if bufferTabsSnapshot == nil {
            let tabsFetch = fetchBufferTabsSnapshot()
            if tabsFetch.changed {
                bufferTabsSnapshot = tabsFetch.snapshot
            }
        }

        guard let snapshot = bufferTabsSnapshot else {
            // If we cannot read tab state, don't block native close.
            return true
        }

        if snapshot.tabs.count <= 1 {
            return true
        }

        // The requested buffer is no longer present in this editor instance.
        if !snapshot.tabs.contains(where: { $0.bufferId == failedBufferId }) {
            return true
        }

        return false
    }

    func colorForHighlight(_ highlight: UInt32) -> SwiftUI.Color? {
        let style = cachedSyntaxHighlightStyle(for: highlight)
        guard style.has_fg else {
            return nil
        }
        return ColorMapper.color(from: style.fg)
    }

    func editorTextColor() -> SwiftUI.Color {
        uiThemeForegroundColor(
            scopes: ["ui.text", "ui.text.focus"],
            fallback: .white
        )
    }

    func editorVirtualTextColor() -> SwiftUI.Color {
        uiThemeForegroundColor(
            scopes: ["ui.virtual", "ui.text.inactive", "ui.text"],
            fallback: editorTextColor().opacity(0.65)
        )
    }

    func uiThemeStyle(_ scope: String) -> Style {
        app.theme_ui_style(scope)
    }

    func editorBackgroundColor() -> SwiftUI.Color {
        uiThemeBackgroundColor(
            scopes: ["ui.background", "ui.window", "ui.popup"],
            fallback: .black
        )
    }

    func popupTheme() -> PopupChromeTheme {
        let panelBackground = themeNSBackgroundColor(
            scopes: ["ui.popup", "ui.background", "ui.window"],
            fallback: NSColor.windowBackgroundColor
        )
        let panelBorder = themeNSForegroundColor(
            scopes: ["ui.window", "ui.background.separator", "ui.text.inactive"],
            fallback: NSColor.separatorColor
        )
        let primaryText = themeNSForegroundColor(
            scopes: ["ui.text", "ui.text.focus"],
            fallback: NSColor.labelColor
        )
        let secondaryText = themeNSForegroundColor(
            scopes: ["ui.text.inactive", "ui.virtual", "ui.text"],
            fallback: NSColor.secondaryLabelColor
        )
        let accent = themeNSForegroundColor(
            scopes: ["ui.text.focus", "ui.linenr.selected", "ui.text"],
            fallback: NSColor.controlAccentColor
        )
        let selectedText = themeNSForegroundColor(
            scopes: ["ui.menu.selected", "ui.text.focus", "ui.text"],
            fallback: primaryText
        )
        let selectedBackground = themeNSBackgroundColor(
            scopes: ["ui.menu.selected", "ui.selection", "ui.menu", "ui.popup"],
            fallback: accent.withAlphaComponent(0.22)
        )
        let hoveredBackground = themeNSBackgroundColor(
            scopes: ["ui.menu", "ui.popup"],
            fallback: accent.withAlphaComponent(0.10)
        )
        let link = themeNSForegroundColor(
            scopes: ["markup.link.url", "markup.link.text", "ui.text.focus", "ui.text"],
            fallback: accent
        )
        let heading = themeNSForegroundColor(
            scopes: ["markup.heading", "ui.text.focus", "ui.text"],
            fallback: primaryText
        )
        let code = themeNSForegroundColor(
            scopes: ["markup.raw", "markup.raw.inline", "ui.text"],
            fallback: primaryText
        )
        let keyword = themeNSForegroundColor(
            scopes: ["keyword", "keyword.control", "storage.type", "ui.text.focus"],
            fallback: accent
        )
        let typeName = themeNSForegroundColor(
            scopes: ["type", "type.builtin", "constructor", "ui.text.focus", "ui.text"],
            fallback: accent
        )
        let number = themeNSForegroundColor(
            scopes: ["constant.numeric", "constant", "ui.text"],
            fallback: primaryText
        )
        let string = themeNSForegroundColor(
            scopes: ["string", "string.special", "ui.text"],
            fallback: primaryText
        )
        let comment = themeNSForegroundColor(
            scopes: ["comment", "ui.text.inactive", "ui.virtual"],
            fallback: secondaryText
        )

        let ghostty = app.theme_ghostty_snapshot()
        let ansiPalette = [
            popupNSColor(from: ghostty.palette0),
            popupNSColor(from: ghostty.palette1),
            popupNSColor(from: ghostty.palette2),
            popupNSColor(from: ghostty.palette3),
            popupNSColor(from: ghostty.palette4),
            popupNSColor(from: ghostty.palette5),
            popupNSColor(from: ghostty.palette6),
            popupNSColor(from: ghostty.palette7),
            popupNSColor(from: ghostty.palette8),
            popupNSColor(from: ghostty.palette9),
            popupNSColor(from: ghostty.palette10),
            popupNSColor(from: ghostty.palette11),
            popupNSColor(from: ghostty.palette12),
            popupNSColor(from: ghostty.palette13),
            popupNSColor(from: ghostty.palette14),
            popupNSColor(from: ghostty.palette15),
        ]

        let docsTheme = CompletionDocsTheme(
            panelBackground: panelBackground,
            panelBorder: panelBorder,
            bodyColor: primaryText,
            headingColor: heading,
            linkColor: link,
            codeColor: code,
            keywordColor: keyword,
            typeColor: typeName,
            numberColor: number,
            stringColor: string,
            commentColor: comment,
            ansiPalette: ansiPalette
        )

        return PopupChromeTheme(
            panelBackground: panelBackground,
            panelBorder: panelBorder,
            primaryText: primaryText,
            secondaryText: secondaryText,
            selectedText: selectedText,
            selectedBackground: selectedBackground,
            hoveredBackground: hoveredBackground,
            accent: accent,
            docsTheme: docsTheme
        )
    }

    func bufferTabBarTheme() -> BufferTabBarTheme {
        func bg(_ scope: String) -> SwiftUI.Color? {
            let style = uiThemeStyle(scope)
            guard style.has_bg else { return nil }
            return ColorMapper.color(from: style.bg)
        }
        func fg(_ scope: String) -> SwiftUI.Color? {
            let style = uiThemeStyle(scope)
            guard style.has_fg else { return nil }
            return ColorMapper.color(from: style.fg)
        }

        let fallback = BufferTabBarTheme.fallback
        let barBackground = bg("ui.buffer_tabs")
            ?? bg("ui.window")
            ?? bg("ui.background")
            ?? fallback.barBackground
        let barBorder = fg("ui.buffer_tabs")
            ?? fg("ui.window")
            ?? fallback.barBorder
        let activeBg = bg("ui.buffer_tabs.tab.active")
            ?? bg("ui.window.active")
            ?? fallback.tabActiveBackground
        let activeFg = fg("ui.buffer_tabs.tab.active")
            ?? fg("ui.text.focus")
            ?? fallback.tabActiveForeground
        let inactiveBg = bg("ui.buffer_tabs.tab.inactive")
            ?? fallback.tabInactiveBackground
        let inactiveFg = fg("ui.buffer_tabs.tab.inactive")
            ?? fg("ui.text")
            ?? fallback.tabInactiveForeground
        let hoverBg = bg("ui.buffer_tabs.tab.hovered")
            ?? fallback.tabHoverBackground
        let modified = fg("ui.buffer_tabs.tab.modified")
            ?? fg("warning")
            ?? fallback.modifiedIndicator

        return BufferTabBarTheme(
            barBackground: barBackground,
            barBorder: barBorder.opacity(0.3),
            tabActiveBackground: activeBg,
            tabActiveForeground: activeFg,
            tabInactiveBackground: inactiveBg,
            tabInactiveForeground: inactiveFg,
            tabHoverBackground: hoverBg,
            tabStroke: inactiveFg.opacity(0.10),
            tabStrokeActive: activeFg.opacity(0.18),
            modifiedIndicator: modified,
            directoryText: inactiveFg.opacity(0.55)
        )
    }

    func completionDocsLanguageHint() -> String {
        let active = app.active_file_path(editorId).toString()
        let fallbackPath = activeBufferTabSnapshot()?.filePath ?? initialFilePath
        let path = (!active.isEmpty ? active : (fallbackPath ?? ""))
        guard !path.isEmpty else {
            return ""
        }
        let fileUrl = URL(fileURLWithPath: path)
        if !fileUrl.pathExtension.isEmpty {
            return fileUrl.pathExtension
        }
        return fileUrl.lastPathComponent
    }

    func selectCompletion(index: Int) {
        guard moveCompletionSelection(to: index) else {
            return
        }
        refresh(trigger: "completion_select")
    }

    func submitCompletion(index: Int) {
        _ = moveCompletionSelection(to: index)
        let enter = KeyEvent(kind: KeyKind.enter.rawValue, codepoint: 0, modifiers: 0)
        _ = app.handle_key(editorId, enter)
        refresh(trigger: "completion_submit")
    }

    @discardableResult
    private func moveCompletionSelection(to index: Int) -> Bool {
        guard let snapshot = uiTree.completionSnapshot(), !snapshot.items.isEmpty else {
            return false
        }

        let count = snapshot.items.count
        let target = max(0, min(index, count - 1))
        let current = max(0, min(snapshot.selectedIndex ?? 0, count - 1))
        if target == current {
            return false
        }

        let downSteps = (target - current + count) % count
        let upSteps = (current - target + count) % count
        let useDown = downSteps <= upSteps
        let steps = useDown ? downSteps : upSteps
        guard steps > 0 else {
            return false
        }

        let navEvent = KeyEvent(
            kind: (useDown ? KeyKind.down : KeyKind.up).rawValue,
            codepoint: 0,
            modifiers: 0
        )
        for _ in 0..<steps {
            _ = app.handle_key(editorId, navEvent)
        }
        return true
    }

    private func cachedSyntaxHighlightStyle(for highlight: UInt32) -> Style {
        if let style = syntaxHighlightStyleCache[highlight] {
            return style
        }
        let style = app.theme_highlight_style(highlight)
        syntaxHighlightStyleCache[highlight] = style
        return style
    }

    private func uiThemeBackgroundColor(
        scopes: [String],
        fallback: SwiftUI.Color
    ) -> SwiftUI.Color {
        for scope in scopes {
            let style = uiThemeStyle(scope)
            if style.has_bg, let color = ColorMapper.color(from: style.bg) {
                return color
            }
        }
        return fallback
    }

    private func uiThemeForegroundColor(
        scopes: [String],
        fallback: SwiftUI.Color
    ) -> SwiftUI.Color {
        for scope in scopes {
            let style = uiThemeStyle(scope)
            if style.has_fg, let color = ColorMapper.color(from: style.fg) {
                return color
            }
        }
        return fallback
    }

    private func themeNSBackgroundColor(scopes: [String], fallback: NSColor) -> NSColor {
        for scope in scopes {
            let style = uiThemeStyle(scope)
            if style.has_bg, let color = popupNSColor(from: style.bg) {
                return color
            }
        }
        return fallback
    }

    private func themeNSForegroundColor(scopes: [String], fallback: NSColor) -> NSColor {
        for scope in scopes {
            let style = uiThemeStyle(scope)
            if style.has_fg, let color = popupNSColor(from: style.fg) {
                return color
            }
        }
        return fallback
    }

    private func popupNSColor(from color: TheEditorFFIBridge.Color) -> NSColor? {
        switch color.kind {
        case 0:
            return nil
        case 1:
            let palette: [NSColor] = [
                .black,
                .systemRed,
                .systemGreen,
                .systemYellow,
                .systemBlue,
                .systemPurple,
                .systemCyan,
                .gray,
                .systemRed.withAlphaComponent(0.85),
                .systemGreen.withAlphaComponent(0.85),
                .systemYellow.withAlphaComponent(0.85),
                .systemBlue.withAlphaComponent(0.85),
                .systemPurple.withAlphaComponent(0.85),
                .systemCyan.withAlphaComponent(0.85),
                .lightGray,
                .white,
            ]
            let index = Int(color.value)
            return (index >= 0 && index < palette.count) ? palette[index] : nil
        case 2:
            let r = CGFloat((color.value >> 16) & 0xFF) / 255.0
            let g = CGFloat((color.value >> 8) & 0xFF) / 255.0
            let b = CGFloat(color.value & 0xFF) / 255.0
            return NSColor(red: r, green: g, blue: b, alpha: 1.0)
        case 3:
            return popupXtermNSColor(index: Int(color.value))
        default:
            return nil
        }
    }

    private func popupNSColor(from optionalColor: OptionalColor) -> NSColor? {
        guard optionalColor.has_value else {
            return nil
        }
        return popupNSColor(from: optionalColor.color)
    }

    private func popupXtermNSColor(index: Int) -> NSColor? {
        guard index >= 0 else {
            return nil
        }
        if index < 16 {
            return popupNSColor(from: TheEditorFFIBridge.Color(kind: 1, value: UInt32(index)))
        }
        if index >= 232 {
            let level = CGFloat(index - 232) / 23.0
            return NSColor(white: level, alpha: 1.0)
        }
        let idx = index - 16
        let r = idx / 36
        let g = (idx % 36) / 6
        let b = idx % 6
        func component(_ value: Int) -> CGFloat {
            let levels: [CGFloat] = [0.0, 0.37, 0.58, 0.74, 0.87, 1.0]
            return levels[min(max(value, 0), levels.count - 1)]
        }
        return NSColor(
            red: component(r),
            green: component(g),
            blue: component(b),
            alpha: 1.0
        )
    }

    private func synchronizeEffectiveTheme(force: Bool = false) {
        let nextThemeName = app.theme_effective_name().toString()
        guard force || nextThemeName != effectiveThemeName else {
            return
        }

        effectiveThemeName = nextThemeName
        syntaxHighlightStyleCache.removeAll(keepingCapacity: true)
    }

    private func debugUiLog(_ message: String) {
        guard ProcessInfo.processInfo.environment["THE_SWIFT_DEBUG_UI"] == "1" else {
            return
        }
        let line = "[the-swift ui] \(message)\n"
        if let data = line.data(using: .utf8) {
            FileHandle.standardError.write(data)
        }
    }

    private func debugDiagnosticsSnapshot(trigger: String, plan: RenderPlan) {
        guard DiagnosticsDebugLog.enabled else { return }

        let cursorSummary: String
        let cursorCount = Int(plan.cursor_count())
        if cursorCount > 0 {
            let pos = plan.cursor_at(0).pos()
            cursorSummary = "\(pos.row):\(pos.col)"
        } else {
            cursorSummary = "none"
        }

        let lineCount = Int(plan.line_count())
        var totalSpanCount = 0
        var virtualSpanCount = 0
        var virtualRows: [Int] = []
        for lineIndex in 0..<lineCount {
            let line = plan.line_at(UInt(lineIndex))
            let row = Int(line.row())
            let spanCount = Int(line.span_count())
            totalSpanCount += spanCount
            for spanIndex in 0..<spanCount {
                let span = line.span_at(UInt(spanIndex))
                if span.is_virtual() {
                    virtualSpanCount += 1
                    if virtualRows.last != row {
                        virtualRows.append(row)
                    }
                }
            }
        }

        let inlineCount = Int(plan.inline_diagnostic_line_count())
        var inlineItems: [String] = []
        inlineItems.reserveCapacity(inlineCount)
        for i in 0..<inlineCount {
            let line = plan.inline_diagnostic_line_at(UInt(i))
            inlineItems.append(
                "\(line.row()):\(line.col()):\(line.severity()):\(debugTruncate(line.text().toString(), limit: 90))"
            )
        }

        let eolCount = Int(plan.eol_diagnostic_count())
        var eolItems: [String] = []
        eolItems.reserveCapacity(eolCount)
        var cursorDeltaToEol: Int? = nil
        for i in 0..<eolCount {
            let eol = plan.eol_diagnostic_at(UInt(i))
            eolItems.append(
                "\(eol.row()):\(eol.col()):\(eol.severity()):\(debugTruncate(eol.message().toString(), limit: 90))"
            )
            if cursorCount > 0 {
                let pos = plan.cursor_at(0).pos()
                if pos.row == eol.row() {
                    cursorDeltaToEol = Int(eol.col()) - Int(pos.col)
                }
            }
        }

        let underlineCount = Int(plan.diagnostic_underline_count())
        let summary = [
            "trigger=\(trigger)",
            "cursor=\(cursorSummary)",
            "cursor_to_eol=\(cursorDeltaToEol.map(String.init) ?? "na")",
            "lines=\(lineCount)",
            "spans=\(totalSpanCount)",
            "virtual_spans=\(virtualSpanCount)",
            "virtual_rows=\(virtualRows.prefix(8).map(String.init).joined(separator: ","))",
            "inline_count=\(inlineCount)",
            "eol_count=\(eolCount)",
            "underline_count=\(underlineCount)",
            "inline=[\(inlineItems.joined(separator: " || "))]",
            "eol=[\(eolItems.joined(separator: " || "))]"
        ].joined(separator: " ")

        DiagnosticsDebugLog.logChanged(key: "model.snapshot", value: summary)
    }

    private func debugTruncate(_ text: String, limit: Int) -> String {
        let normalized = text
            .replacingOccurrences(of: "\n", with: "\\n")
            .replacingOccurrences(of: "\t", with: "\\t")
        if normalized.count <= limit {
            return normalized
        }
        let idx = normalized.index(normalized.startIndex, offsetBy: limit)
        return "\(normalized[..<idx])..."
    }

    private func debugLogFilePickerPreview(preview: PreviewData, requestedOffset: UInt) {
        guard DiagnosticsDebugLog.enabled else { return }
        guard let snapshot = filePickerSnapshot, snapshot.pickerKind == 1 else { return }

        let lineCount = Int(preview.line_count())
        var focusedSummary = "none"
        var firstLineSummary = "none"
        var lastLineSummary = "none"

        if lineCount > 0 {
            let firstLine = preview.line_at(0)
            let lastLine = preview.line_at(UInt(lineCount - 1))
            firstLineSummary = "\(firstLine.virtual_row()):\(firstLine.line_number())"
            lastLineSummary = "\(lastLine.virtual_row()):\(lastLine.line_number())"

            for index in 0..<lineCount {
                let line = preview.line_at(UInt(index))
                if line.focused() {
                    focusedSummary = "\(line.virtual_row()):\(line.line_number())"
                    break
                }
            }
        }

        let requested = requestedOffset == UInt.max ? "focus" : String(requestedOffset)
        DiagnosticsDebugLog.log(
            "picker.preview diagnostics requested=\(requested) hint=\(filePickerPreviewOffsetHint) visible=\(filePickerPreviewVisibleRows) overscan=\(filePickerPreviewOverscan) returned_offset=\(preview.offset()) window_start=\(preview.window_start()) total=\(preview.total_lines()) count=\(lineCount) focused=\(focusedSummary) first=\(firstLineSummary) last=\(lastLineSummary) path=\(preview.path().toString())"
        )
    }

    private func debugSurfaceRailState(trigger: String) {
        guard DiagnosticsDebugLog.enabled else { return }

        let paneSummary = paneSurfaceItems.map { pane in
            let kind = pane.kind == .editor ? "e" : "t"
            return "p\(pane.paneId):\(kind)"
        }.joined(separator: ",")

        let editorSummary = editorSurfacePanes.map { surface in
            "p\(surface.paneId):b\(surface.bufferIndex):a\(surface.isActive ? 1 : 0):\(normalizeFallbackTitle(surface.title))"
        }.joined(separator: ",")

        let terminalSummary = terminalSurfaces.map { surface in
            "t\(surface.terminalId):p\(surface.paneId ?? 0):a\(surface.isActive ? 1 : 0)"
        }.joined(separator: ",")

        let bufferSummary = bufferTabsSnapshot?.tabs.map { tab in
            "b\(tab.bufferIndex):a\(tab.isActive ? 1 : 0):\(normalizeFallbackTitle(tab.title))"
        }.joined(separator: ",") ?? ""

        let summary = [
            "trigger=\(trigger)",
            "activePane=\(framePlan.active_pane_id())",
            "activeBuffer=\(debugActiveBufferSummary())",
            "lastEditorPane=\(lastFocusedEditorPaneId ?? 0)",
            "lastTerminal=\(lastFocusedTerminalSurfaceId ?? 0)",
            "panes=[\(paneSummary)]",
            "editorSurfaces=[\(editorSummary)]",
            "terminalSurfaces=[\(terminalSummary)]",
            "openBuffers=[\(bufferSummary)]"
        ].joined(separator: " ")

        DiagnosticsDebugLog.logChanged(
            key: "surface_rail.state.\(runtimeInstanceId).\(editorId.value)",
            value: summary
        )
    }

    private func debugActiveBufferSummary() -> String {
        guard let activeTab = activeBufferTabSnapshot() else {
            return "none"
        }
        return "b\(activeTab.bufferIndex):\(normalizeFallbackTitle(activeTab.title))"
    }

    private func debugFirstResponderSummary() -> String {
        guard let responder = hostWindow?.firstResponder else {
            return "none"
        }
        return String(describing: type(of: responder))
    }
}
