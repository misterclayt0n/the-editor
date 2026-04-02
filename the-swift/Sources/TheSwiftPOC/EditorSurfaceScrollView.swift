import AppKit
import Foundation

private struct ObserverTokenBox: @unchecked Sendable {
    let raw: NSObjectProtocol
}

private final class FlippedDocumentView: NSView {
    override var isFlipped: Bool { true }
}

@MainActor
final class EditorSurfaceScrollView: NSView, EditorSurfaceControllerDelegate {
    let controller: EditorSurfaceController
    let scrollView: NSScrollView
    private let documentView: FlippedDocumentView
    let surfaceView: EditorSurfaceView

    private var boundsObserver: ObserverTokenBox?
    private var isSyncingScroll = false

    init(initialPath: String?) {
        self.controller = EditorSurfaceController(initialPath: initialPath)
        self.scrollView = NSScrollView(frame: .zero)
        self.documentView = FlippedDocumentView(frame: .zero)
        guard let surfaceView = EditorSurfaceView(controller: controller) else {
            fatalError("failed to create metal-backed editor surface")
        }
        self.surfaceView = surfaceView
        super.init(frame: .zero)

        controller.delegate = self

        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = false
        scrollView.usesPredominantAxisScrolling = true
        scrollView.contentView.postsBoundsChangedNotifications = true
        scrollView.documentView = documentView
        documentView.addSubview(surfaceView)
        addSubview(scrollView)

        boundsObserver = ObserverTokenBox(raw: NotificationCenter.default.addObserver(
            forName: NSView.boundsDidChangeNotification,
            object: scrollView.contentView,
            queue: .main
        ) { [weak self] _ in
            guard let self else { return }
            Task { @MainActor in
                self.handleScrollChange()
            }
        })

        controller.refreshSnapshot()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        if let boundsObserver {
            NotificationCenter.default.removeObserver(boundsObserver.raw)
        }
    }

    override func layout() {
        super.layout()
        scrollView.frame = bounds
        surfaceView.frame.size = scrollView.contentView.bounds.size
        synchronizeSurfaceFrame()
    }

    func editorController(_ controller: EditorSurfaceController, didUpdateScene scene: EditorRenderScene) {
        updateDocumentMetrics(scene)
        synchronizeScrollPosition(scene)
        surfaceView.update(scene: scene)
        synchronizeSurfaceFrame()
    }

    private func updateDocumentMetrics(_ scene: EditorRenderScene) {
        let cellSize = scene.info.surfaceMetrics.cellSizePoints
        let contentHeight = max(CGFloat(max(scene.info.documentLineCount, scene.info.viewportHeight)) * cellSize.height, scrollView.contentView.bounds.height)
        documentView.frame.size = CGSize(width: scrollView.contentView.bounds.width, height: contentHeight)
    }

    private func synchronizeSurfaceFrame() {
        let visibleRect = scrollView.contentView.documentVisibleRect
        surfaceView.frame = CGRect(origin: visibleRect.origin, size: scrollView.contentView.bounds.size)
    }

    private func synchronizeScrollPosition(_ scene: EditorRenderScene) {
        guard !isSyncingScroll else { return }
        let cellHeight = scene.info.surfaceMetrics.cellSizePoints.height
        let targetOrigin = CGPoint(x: 0, y: CGFloat(scene.info.scrollRow) * cellHeight)
        if scrollView.contentView.bounds.origin.y != targetOrigin.y {
            isSyncingScroll = true
            scrollView.contentView.scroll(to: targetOrigin)
            scrollView.reflectScrolledClipView(scrollView.contentView)
            isSyncingScroll = false
        }
    }

    private func handleScrollChange() {
        synchronizeSurfaceFrame()
        guard !isSyncingScroll else { return }
        let cellHeight = controller.scene?.info.surfaceMetrics.cellSizePoints.height ?? surfaceView.cellSize.height
        let row = Int((scrollView.contentView.bounds.origin.y / max(cellHeight, 1)).rounded(.down))
        controller.setScrollRow(row)
    }
}
