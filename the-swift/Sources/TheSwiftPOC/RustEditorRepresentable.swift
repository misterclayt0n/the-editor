import SwiftUI

struct EditorSurfaceRepresentable: NSViewRepresentable {
    let initialPath: String?

    func makeNSView(context: Context) -> EditorSurfaceScrollView {
        EditorSurfaceScrollView(initialPath: initialPath)
    }

    func updateNSView(_ nsView: EditorSurfaceScrollView, context: Context) {}
}

// Backwards-compatible wrapper name for the original POC file.
typealias RustEditorRepresentable = EditorSurfaceRepresentable
