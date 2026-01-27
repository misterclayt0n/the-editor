import Foundation
import TheEditorFFIBridge

final class EditorModel: ObservableObject {
    private let app: App
    let editorId: EditorId
    @Published var plan: RenderPlan
    private var viewport: Rect

    init() {
        self.app = App()
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
}
