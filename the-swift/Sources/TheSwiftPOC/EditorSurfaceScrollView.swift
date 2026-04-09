import AppKit
import Foundation

@MainActor
final class EditorSurfaceScrollView: NSView, EditorSurfaceControllerDelegate {
    let controller: EditorSurfaceController
    let surfaceView: EditorSurfaceView
    let terminalContainerView = GhosttyTerminalOverlayContainerView()
    let terminalRegistry: GhosttyTerminalRegistry

    init(controller: EditorSurfaceController) {
        self.controller = controller
        self.terminalRegistry = GhosttyTerminalRegistry(controller: controller)
        guard let surfaceView = EditorSurfaceView(controller: controller) else {
            fatalError("failed to create metal-backed editor surface")
        }
        self.surfaceView = surfaceView
        super.init(frame: .zero)

        controller.delegate = self
        controller.editorFirstResponder = surfaceView
        addSubview(surfaceView)
        addSubview(terminalContainerView)
        controller.refreshSnapshot()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func layout() {
        super.layout()
        surfaceView.frame = bounds
        terminalContainerView.frame = bounds
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if let scene = controller.scene {
            updateWindowResizeIncrements(scene)
            terminalRegistry.reconcile(
                scene: scene,
                openItems: controller.openItems,
                in: terminalContainerView,
                editorSurfaceView: surfaceView
            )
        }
    }

    override func viewWillStartLiveResize() {
        super.viewWillStartLiveResize()
        controller.beginLiveResize()
    }

    override func viewDidEndLiveResize() {
        super.viewDidEndLiveResize()
        controller.endLiveResize()
    }

    func editorController(_ controller: EditorSurfaceController, didUpdateScene scene: EditorRenderScene) {
        surfaceView.update(scene: scene)
        terminalRegistry.reconcile(
            scene: scene,
            openItems: controller.openItems,
            in: terminalContainerView,
            editorSurfaceView: surfaceView
        )
        updateWindowResizeIncrements(scene)
    }

    private func updateWindowResizeIncrements(_ scene: EditorRenderScene) {
        guard let window else { return }
        let cellSize = scene.info.surfaceMetrics.cellSizePoints
        guard cellSize.width > 0, cellSize.height > 0 else { return }
        if window.contentResizeIncrements != cellSize {
            window.contentResizeIncrements = cellSize
        }
    }
}
