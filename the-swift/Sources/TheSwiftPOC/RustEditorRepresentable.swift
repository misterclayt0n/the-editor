import AppKit
import SwiftUI

struct EditorSurfaceRepresentable: NSViewRepresentable {
    @ObservedObject var controller: EditorSurfaceController
    let agentBackgroundColor: NSColor
    let agentSelectionColor: NSColor
    let isRenderingSuspended: Bool

    func makeNSView(context: Context) -> EditorSurfaceScrollView {
        let view = EditorSurfaceScrollView(controller: controller)
        view.configureAgentPaneAppearance(
            backgroundColor: agentBackgroundColor,
            selectionColor: agentSelectionColor
        )
        view.setRenderingSuspended(isRenderingSuspended)
        return view
    }

    func updateNSView(_ nsView: EditorSurfaceScrollView, context: Context) {
        nsView.updateBufferFontSize(controller.bufferFontSize)
        nsView.configureAgentPaneAppearance(
            backgroundColor: agentBackgroundColor,
            selectionColor: agentSelectionColor
        )
        nsView.setRenderingSuspended(isRenderingSuspended)
    }
}

// Backwards-compatible wrapper name for the original POC file.
typealias RustEditorRepresentable = EditorSurfaceRepresentable
