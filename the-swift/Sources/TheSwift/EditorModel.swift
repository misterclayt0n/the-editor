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
    @Published var isActivePaneTerminal: Bool = false
    @Published var uiTree: UiTreeSnapshot = .empty
    @Published var bufferTabsSnapshot: BufferTabsSnapshot? = nil
    @Published var navigationTitle: String = "untitled"
    private var viewport: Rect
    private var effectiveViewport: Rect
    let cellSize: CGSize
    let bufferFont: Font
    let bufferNSFont: NSFont
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
        let fontInfo = FontLoader.loadBufferFont(size: 14)
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
        _ = trigger
        let shouldDriveRuntime = shouldDriveSharedRuntime()
        if shouldDriveRuntime {
            _ = app.poll_background(editorId)
        }
        let uiFetch = fetchUiTree()
        if uiFetch.changed {
            uiTree = uiFetch.tree
        }
        let tabsFetch = fetchBufferTabsSnapshot()
        if tabsFetch.changed {
            bufferTabsSnapshot = tabsFetch.snapshot
        }
        updateBoundBufferIdFromActiveBufferIfNeeded()
        // Buffer tabs in the Swift app are native window tabs, not in-content chrome.
        setTopChromeReservedRows(0)
        updateEffectiveViewport()

        framePlan = app.frame_render_plan(editorId)
        plan = framePlan.active_plan()
        splitSeparators = fetchSplitSeparators()
        updateTerminalPaneSnapshots(from: framePlan)
        updateTerminalSurfaceSnapshots()
        updateSurfaceOverviewSnapshots(from: framePlan)
        debugDiagnosticsSnapshot(trigger: trigger, plan: plan)

        mode = EditorMode(rawValue: app.mode(editorId)) ?? .normal
        pendingKeys = fetchPendingKeys()
        pendingKeyHints = fetchPendingKeyHints()
        refreshFilePicker()
        refreshFileTree()

        // Sync title/subtitle after pane snapshots update so terminal/editor focus
        // state reflects the latest frame.
        if shouldSyncNativeWindowPresentation() {
            syncNativeWindowPresentation()
        }

        if app.take_should_quit() {
            NSApp.terminate(nil)
        }
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
        let paneCount = Int(framePlan.pane_count())
        guard paneCount > 0 else { return nil }
        for index in 0..<paneCount {
            let pane = framePlan.pane_at(UInt(index))
            if pane.is_active() {
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

    @discardableResult
    private func splitActivePane(axis: PaneSplitAxis) -> Bool {
        guard app.split_active_pane(editorId, axis.rawValue) else {
            return false
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
        if isActivePaneTerminal, closeTerminalInActivePane() {
            return true
        }

        let paneCountBefore = Int(framePlan.pane_count())
        if paneCountBefore > 1 {
            let didExecuteClose = executeNamedCommand("wclose")
            let paneCountAfter = Int(framePlan.pane_count())
            if paneCountAfter < paneCountBefore || didExecuteClose {
                return true
            }
            return false
        }

        guard let hostWindow else {
            return false
        }
        hostWindow.performClose(nil)
        return true
    }

    @discardableResult
    func toggleLastTerminalSurface() -> Bool {
        let activePaneId = framePlan.active_pane_id()
        guard let activeItem = paneSurfaceItems.first(where: { $0.paneId == activePaneId }) else {
            return false
        }

        switch activeItem.kind {
        case .terminal:
            if let target = preferredEditorPaneId(excluding: activePaneId) {
                return focusPane(paneId: target, trigger: "toggle_last_terminal")
            }
            return hideActiveTerminalSurface()
        case .editor:
            if let terminalId = preferredTerminalSurfaceId() {
                return focusTerminalSurface(terminalId: terminalId)
            }
            lastFocusedEditorPaneId = activePaneId
            return openTerminalInActivePane()
        }
    }

    @discardableResult
    private func focusPane(paneId: UInt64, trigger: String) -> Bool {
        guard paneId != 0 else {
            return false
        }
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
            return false
        }
        refresh(trigger: trigger)
        return true
    }

    @discardableResult
    func executeNamedCommand(_ command: EditorNamedCommand) -> Bool {
        switch command {
        case .openNativeTab:
            return openNativeUntitledTab()
        case .closeSurface:
            return closeSurface()
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
        case .toggleLastTerminal:
            return toggleLastTerminalSurface()
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
            let isActive = pane.is_active()
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

    func selectBufferTab(bufferIndex: Int) {
        guard bufferIndex >= 0 else { return }
        guard app.activate_buffer_tab(editorId, UInt(bufferIndex)) else {
            return
        }
        refresh(trigger: "buffer_tab")
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

        presentCloseConfirmation(for: context, in: window)
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
        let uiFetch = fetchUiTree()
        if uiFetch.changed {
            uiTree = uiFetch.tree
        }
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
            NSWindow.didBecomeMainNotification
        ]
        hostWindowNotificationTokens = names.map { name in
            center.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                self?.activateBoundBufferForFocusedWindow(trigger: "window_focus")
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

    private func presentCloseConfirmation(for context: BufferCloseContext, in window: NSWindow) {
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
            guard self.closeBufferForWindowClose(context) else { return }
            self.allowProgrammaticWindowClose = true
            window.performClose(nil)
        }
        closeConfirmationAlert = alert
    }

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

    func uiThemeStyle(_ scope: String) -> Style {
        app.theme_ui_style(scope)
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
}
