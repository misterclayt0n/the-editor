import AppKit
import Foundation

@MainActor
final class EditorSurfaceScrollView: NSView, EditorSurfaceControllerDelegate {
    let controller: EditorSurfaceController
    let surfaceView: EditorSurfaceView
    let terminalContainerView = GhosttyTerminalOverlayContainerView()
    let terminalRegistry: GhosttyTerminalRegistry
    private var lastLayoutDebugSignature: String?
    private var lastSceneDebugSignature: String?

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
        logLayoutMetrics(reason: "layout")
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        logLayoutMetrics(reason: "movedToWindow")
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

    func updateBufferFontSize(_ pointSize: CGFloat) {
        surfaceView.updateBufferFontSize(pointSize)
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
        logSceneMetrics(scene)
        logLayoutMetrics(reason: "sceneUpdate")
    }

    private func logSceneMetrics(_ scene: EditorRenderScene) {
        let signature = [
            "scene",
            "viewport=\(scene.info.viewportWidth)x\(scene.info.viewportHeight)",
            String(format: "cell=%.2fx%.2f", scene.info.surfaceMetrics.cellSizePoints.width, scene.info.surfaceMetrics.cellSizePoints.height),
            "panes=\(scene.panes.count)",
            "paneStrips=\(scene.paneItemStripPaneIDs.count)",
            "openItemGroups=\(controller.openItems.groups.count)",
            "bufferTabs=\(controller.bufferTabs.tabs.count)",
            "statusItems=\(controller.chrome.statusBar.items.count)"
        ].joined(separator: " ")
        guard signature != lastSceneDebugSignature else { return }
        lastSceneDebugSignature = signature
        layoutDebugLog(signature)
    }

    private func logLayoutMetrics(reason: String) {
        let windowFrameText = window.map { rectText($0.frame) } ?? "nil"
        let contentLayoutText = window.map { rectText($0.contentLayoutRect) } ?? "nil"
        let contentViewBoundsText = window?.contentView.map { rectText($0.bounds) } ?? "nil"
        let signature = [
            "surface reason=\(reason)",
            "bounds=\(rectText(bounds))",
            "frame=\(rectText(frame))",
            "safeArea=\(edgeInsetsText(safeAreaInsets))",
            "surfaceFrame=\(rectText(surfaceView.frame))",
            "terminalFrame=\(rectText(terminalContainerView.frame))",
            "windowFrame=\(windowFrameText)",
            "contentLayoutRect=\(contentLayoutText)",
            "contentViewBounds=\(contentViewBoundsText)"
        ].joined(separator: " ")
        guard signature != lastLayoutDebugSignature else { return }
        lastLayoutDebugSignature = signature
        layoutDebugLog(signature)
    }

    private func rectText(_ rect: CGRect) -> String {
        String(format: "(x:%.1f y:%.1f w:%.1f h:%.1f)", rect.origin.x, rect.origin.y, rect.size.width, rect.size.height)
    }

    private func edgeInsetsText(_ insets: NSEdgeInsets) -> String {
        String(format: "(t:%.1f l:%.1f b:%.1f r:%.1f)", insets.top, insets.left, insets.bottom, insets.right)
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
