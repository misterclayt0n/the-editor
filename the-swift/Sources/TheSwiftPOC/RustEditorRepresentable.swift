import SwiftUI

struct EditorSurfaceRepresentable: NSViewRepresentable {
    @ObservedObject var controller: EditorSurfaceController

    func makeNSView(context: Context) -> EditorSurfaceScrollView {
        EditorSurfaceScrollView(controller: controller)
    }

    func updateNSView(_ nsView: EditorSurfaceScrollView, context: Context) {
        nsView.updateBufferFontSize(controller.bufferFontSize)
    }
}

// Backwards-compatible wrapper name for the original POC file.
typealias RustEditorRepresentable = EditorSurfaceRepresentable
