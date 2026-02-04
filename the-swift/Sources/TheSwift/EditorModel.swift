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
    let cellSize: CGSize
    let font: Font
    private(set) var mode: EditorMode = .normal
    private var scrollRemainderX: CGFloat = 0
    private var scrollRemainderY: CGFloat = 0

    init(filePath: String? = nil) {
        self.app = TheEditorFFIBridge.App()
        let fontInfo = FontLoader.loadIosevka(size: 14)
        self.cellSize = fontInfo.cellSize
        self.font = fontInfo.font
        self.viewport = Rect(x: 0, y: 0, width: 80, height: 24)
        let scroll = Position(row: 0, col: 0)
        let initialText = EditorModel.loadText(filePath: filePath)
        self.editorId = app.create_editor(initialText, viewport, scroll)
        self.plan = app.render_plan(editorId)
        self.mode = EditorMode(rawValue: app.mode(editorId)) ?? .normal
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
        _ = app.set_viewport(editorId, viewport)
        refresh()
    }

    func refresh() {
        plan = app.render_plan(editorId)
        mode = EditorMode(rawValue: app.mode(editorId)) ?? .normal
        uiTree = fetchUiTree()
        if app.take_should_quit() {
            NSApp.terminate(nil)
        }
    }

    func insert(_ text: String) {
        _ = app.insert(editorId, text)
        refresh()
    }

    func handleKeyEvent(_ keyEvent: KeyEvent) {
        if mode == .command {
            if sendUiKeyEvent(keyEvent) {
                refresh()
                return
            }
        }

        _ = app.handle_key(editorId, keyEvent)
        refresh()
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
        refresh()
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
        refresh()
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

    func submitCommandPalette(index: Int) {
        _ = app.command_palette_submit_filtered(editorId, UInt(index))
        refresh()
    }

    func closeCommandPalette() {
        _ = app.command_palette_close(editorId)
        refresh()
    }

    func setCommandPaletteQuery(_ query: String) {
        _ = app.command_palette_set_query(editorId, query)
        uiTree = fetchUiTree()
    }

    private func fetchUiTree() -> UiTreeSnapshot {
        let json = app.ui_tree_json(editorId).toString()
        guard let data = json.data(using: .utf8) else {
            return .empty
        }

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        do {
            return try decoder.decode(UiTreeSnapshot.self, from: data)
        } catch {
            return .empty
        }
    }
}
