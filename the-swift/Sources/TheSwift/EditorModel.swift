import AppKit
import Foundation
import SwiftUI
import TheEditorFFIBridge

final class EditorModel: ObservableObject {
    private let app: TheEditorFFIBridge.App
    let editorId: EditorId
    @Published var plan: RenderPlan
    @Published var uiTree: UiTreeSnapshot = .empty
    private var viewport: Rect
    private var effectiveViewport: Rect
    let cellSize: CGSize
    let bufferFont: Font
    let bufferNSFont: NSFont
    private let initialFilePath: String?
    private(set) var mode: EditorMode = .normal
    @Published var pendingKeys: [String] = []
    @Published var pendingKeyHints: PendingKeyHintsSnapshot? = nil
    @Published var filePickerSnapshot: FilePickerSnapshot? = nil
    let filePickerPreviewModel = FilePickerPreviewModel()
    private var filePickerTimer: Timer? = nil
    private var backgroundTimer: Timer? = nil
    private var scrollRemainderX: CGFloat = 0
    private var scrollRemainderY: CGFloat = 0
    private var syntaxHighlightStyleCache: [UInt32: Style] = [:]
    private var lastUiTreeJson: String? = nil

    init(filePath: String? = nil) {
        self.app = TheEditorFFIBridge.App()
        self.initialFilePath = filePath
        let fontInfo = FontLoader.loadBufferFont(size: 14)
        self.cellSize = fontInfo.cellSize
        self.bufferFont = fontInfo.font
        self.bufferNSFont = fontInfo.nsFont
        self.viewport = Rect(x: 0, y: 0, width: 80, height: 24)
        self.effectiveViewport = self.viewport
        let scroll = Position(row: 0, col: 0)
        let initialText = EditorModel.loadText(filePath: filePath)
        self.editorId = app.create_editor(initialText, viewport, scroll)
        if let filePath {
            _ = app.set_file_path(editorId, filePath)
        }
        self.plan = app.render_plan(editorId)
        self.mode = EditorMode(rawValue: app.mode(editorId)) ?? .normal
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
        _ = app.poll_background(editorId)
        let uiFetch = fetchUiTree()
        if uiFetch.changed {
            uiTree = uiFetch.tree
        }
        updateEffectiveViewport()

        plan = app.render_plan(editorId)
        debugDiagnosticsSnapshot(trigger: trigger, plan: plan)

        mode = EditorMode(rawValue: app.mode(editorId)) ?? .normal
        pendingKeys = fetchPendingKeys()
        pendingKeyHints = fetchPendingKeyHints()
        refreshFilePicker()

        if app.take_should_quit() {
            NSApp.terminate(nil)
        }
    }

    private func updateEffectiveViewport() {
        let reserved = statuslineReservedRows(in: uiTree)
        let height = max(1, Int(viewport.height) - reserved)
        let next = Rect(x: 0, y: 0, width: viewport.width, height: UInt16(height))
        if !rectsEqual(next, effectiveViewport) {
            effectiveViewport = next
            _ = app.set_viewport(editorId, next)
        }
    }

    private func rectsEqual(_ lhs: Rect, _ rhs: Rect) -> Bool {
        lhs.x == rhs.x && lhs.y == rhs.y && lhs.width == rhs.width && lhs.height == rhs.height
    }

    private func statuslineReservedRows(in tree: UiTreeSnapshot) -> Int {
        for node in tree.overlays {
            if case .panel(let panel) = node, panel.id == "statusline" {
                // Fixed 22pt statusline height
                return max(1, Int((22.0 / cellSize.height).rounded()))
            }
        }
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

    func selectCommandPalette(index: Int) {
        _ = app.command_palette_select_filtered(editorId, UInt(index))
        refresh()
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
        // Only refresh the preview — don't re-serialize the full item list
        // (up to 10k items) on every arrow key press.
        refreshFilePickerPreview()
    }

    func closeFilePicker() {
        filePickerTimer?.invalidate()
        filePickerTimer = nil
        _ = app.file_picker_close(editorId)
        filePickerSnapshot = nil
        filePickerPreviewModel.preview = nil
        refresh(trigger: "file_picker_close")
    }

    func refreshFilePicker() {
        let json = app.file_picker_snapshot_json(editorId, 10_000).toString()
        guard let data = json.data(using: .utf8) else {
            stopFilePickerTimerIfNeeded()
            filePickerSnapshot = nil
            return
        }

        let decoder = JSONDecoder()
        do {
            let snapshot = try decoder.decode(FilePickerSnapshot.self, from: data)
            if snapshot.active {
                filePickerSnapshot = FilePickerSnapshot(
                    active: snapshot.active,
                    query: snapshot.query,
                    matchedCount: snapshot.matchedCount,
                    totalCount: snapshot.totalCount,
                    scanning: snapshot.scanning,
                    root: snapshot.root,
                    items: snapshot.items?.enumerated().map { index, item in
                        FilePickerItemSnapshot(
                            id: index,
                            display: item.display,
                            isDir: item.isDir,
                            icon: item.icon,
                            matchIndices: item.matchIndices
                        )
                    }
                )
                filePickerPreviewModel.preview = app.file_picker_preview(editorId)
                startFilePickerTimerIfNeeded(scanning: snapshot.scanning ?? false)
            } else {
                stopFilePickerTimerIfNeeded()
                filePickerSnapshot = nil
            }
        } catch {
            debugUiLog("file_picker decode failed: \(error)")
            stopFilePickerTimerIfNeeded()
            filePickerSnapshot = nil
        }
    }

    /// Lightweight refresh — direct FFI, no JSON. Called on selection change.
    func refreshFilePickerPreview() {
        filePickerPreviewModel.preview = app.file_picker_preview(editorId)
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
            if self.app.poll_background(self.editorId) {
                self.refresh(trigger: "background")
            }
        }
    }

    private struct UiTreeFetchResult {
        let tree: UiTreeSnapshot
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

    func colorForHighlight(_ highlight: UInt32) -> SwiftUI.Color? {
        let style = cachedSyntaxHighlightStyle(for: highlight)
        guard style.has_fg else {
            return nil
        }
        return ColorMapper.color(from: style.fg)
    }

    func completionDocsLanguageHint() -> String {
        guard let path = initialFilePath, !path.isEmpty else {
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
}
