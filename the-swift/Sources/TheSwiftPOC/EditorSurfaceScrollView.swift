import AppKit
import Foundation
import SwiftUI

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

        terminalContainerView.wantsLayer = true

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
        measureSidebarSignpostedInterval("SurfaceScrollViewLayout", counterKey: "surfaceScrollView.layout.ms") {
            super.layout()
            surfaceView.frame = bounds
            terminalContainerView.frame = bounds
            reconcileOverlayViews()
            logLayoutMetrics(reason: "layout")
        }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        logLayoutMetrics(reason: "movedToWindow")
        if let scene = controller.scene {
            updateWindowResizeIncrements(scene)
        }
        reconcileOverlayViews()
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

    func configureAgentPaneAppearance(
        backgroundColor: NSColor,
        selectionColor: NSColor
    ) {
        _ = backgroundColor
        _ = selectionColor
    }

    func setRenderingSuspended(_ suspended: Bool) {
        surfaceView.setRenderingSuspended(suspended)
    }

    func editorController(_ controller: EditorSurfaceController, didUpdateScene scene: EditorRenderScene) {
        sidebarPerfIncrement("surfaceScrollView.didUpdateScene.request")
        measureSidebarSignpostedInterval("SurfaceScrollViewDidUpdateScene", counterKey: "surfaceScrollView.didUpdateScene.ms") {
            surfaceView.update(scene: scene)
            updateWindowResizeIncrements(scene)
            reconcileOverlayViews()
            if !surfaceView.isRenderingSuspended {
                logSceneMetrics(scene)
                logLayoutMetrics(reason: "sceneUpdate")
            }
        }
    }

    private func reconcileOverlayViews() {
        terminalRegistry.reconcile(
            scene: controller.scene,
            openItems: controller.openItems,
            in: terminalContainerView,
            editorSurfaceView: surfaceView
        )
        surfaceView.layer?.mask = nil
        terminalContainerView.layer?.mask = nil
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
        let defaultIncrements = NSSize(width: 1, height: 1)
        // Cell-snapped contentResizeIncrements makes the zoomed window stop short of the
        // screen edges whenever the available height isn't an exact multiple of the cell size.
        // cmux doesn't snap window zoom this way; keep native freeform resizing and let the
        // renderer absorb any grid remainder inside the content area instead.
        if window.contentResizeIncrements != defaultIncrements {
            window.contentResizeIncrements = defaultIncrements
        }
    }
}
