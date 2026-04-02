import AppKit
import Foundation
import TheEditorFFI

@MainActor
protocol EditorSurfaceControllerDelegate: AnyObject {
    func editorController(_ controller: EditorSurfaceController, didUpdateScene scene: EditorRenderScene)
}

private struct EditorHandleBox: @unchecked Sendable {
    let raw: OpaquePointer
}

@MainActor
final class EditorSurfaceController {
    weak var delegate: EditorSurfaceControllerDelegate?

    fileprivate var handle: EditorHandleBox?
    private(set) var scene: EditorRenderScene?
    private(set) var currentMode: EditorMode = .normal

    private var viewportCols: UInt16 = 1
    private var viewportRows: UInt16 = 1
    private var markedText: String = ""

    init(initialPath: String?) {
        self.handle = EditorFFIBridge.createHandle(initialPath: initialPath).map(EditorHandleBox.init(raw:))
        refreshSnapshot()
    }

    deinit {
        EditorFFIBridge.destroyHandle(handle?.raw)
    }

    func setViewport(size: CGSize, cellSize: CGSize) {
        let cols = UInt16(max(Int(size.width / max(cellSize.width, 1)), 1))
        let rows = UInt16(max(Int(size.height / max(cellSize.height, 1)), 1))
        guard cols != viewportCols || rows != viewportRows else { return }
        viewportCols = cols
        viewportRows = rows
        EditorFFIBridge.setViewport(handle?.raw, cols: cols, rows: rows)
        refreshSnapshot()
    }

    func setScrollRow(_ row: Int) {
        guard EditorFFIBridge.setScrollRow(handle?.raw, row: UInt32(max(row, 0))) else { return }
        refreshSnapshot()
    }

    func handleKey(_ event: the_editor_key_event_t) {
        guard EditorFFIBridge.sendKey(handle?.raw, event: event) else { return }
        if event.kind == THE_EDITOR_KEY_ESCAPE.rawValue {
            markedText = ""
        }
        refreshSnapshot()
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

    func refreshSnapshot() {
        guard let snapshot = EditorFFIBridge.makeSnapshot(handle?.raw) else { return }
        currentMode = snapshot.info.mode
        let marked = markedTextOverlay(from: snapshot)
        let scene = EditorRenderScene.from(snapshot: snapshot, markedText: marked)
        self.scene = scene
        delegate?.editorController(self, didUpdateScene: scene)
    }

    private func markedTextOverlay(from snapshot: EditorSnapshot) -> EditorMarkedText? {
        guard !markedText.isEmpty else { return nil }
        guard let cursor = snapshot.cursors.first else { return nil }
        return EditorMarkedText(text: markedText, row: cursor.row, col: cursor.col)
    }
}
