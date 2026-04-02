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

    private var surfaceConfiguration: EditorSurfaceConfiguration?
    private var markedText: String = ""

    init(initialPath: String?) {
        self.handle = EditorFFIBridge.createHandle(initialPath: initialPath).map(EditorHandleBox.init(raw:))
        refreshSnapshot()
    }

    deinit {
        EditorFFIBridge.destroyHandle(handle?.raw)
    }

    func configureSurface(size: CGSize, backingScale: CGFloat, fontMetrics: EditorFontMetrics) {
        let configuration = fontMetrics.surfaceConfiguration(viewSize: size, backingScale: backingScale)
        guard configuration != surfaceConfiguration else { return }
        surfaceConfiguration = configuration
        guard EditorFFIBridge.configureSurface(handle?.raw, configuration: configuration) else { return }
        refreshSnapshot()
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
