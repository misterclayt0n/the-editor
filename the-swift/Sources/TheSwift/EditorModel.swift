import AppKit
import Foundation
import SwiftUI
import TheEditorFFIBridge

final class EditorModel: ObservableObject {
    private let app: TheEditorFFIBridge.App
    let editorId: EditorId
    @Published var plan: RenderPlan
    private var viewport: Rect
    let cellSize: CGSize
    let font: Font

    init() {
        self.app = TheEditorFFIBridge.App()
        let fontInfo = FontLoader.loadIosevka(size: 14)
        self.cellSize = fontInfo.cellSize
        self.font = fontInfo.font
        self.viewport = Rect(x: 0, y: 0, width: 80, height: 24)
        let scroll = Position(row: 0, col: 0)

        self.editorId = app.create_editor("hello from the-lib\nswift demo", viewport, scroll)
        self.plan = app.render_plan(editorId)
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
    }

    func insert(_ text: String) {
        _ = app.insert(editorId, text)
        refresh()
    }

    func handleKey(event: NSEvent) {
        guard let keyEvent = KeyEventMapper.map(event: event) else {
            return
        }
        _ = app.handle_key(editorId, keyEvent)
        refresh()
    }
}
