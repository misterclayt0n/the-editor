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
    private var liveScrollStartObserver: ObserverTokenBox?
    private var liveScrollEndObserver: ObserverTokenBox?
    private var liveScrollObserver: ObserverTokenBox?
    private var preferredScrollerStyleObserver: ObserverTokenBox?
    private var isSyncingScroll = false
    private var isLiveScrolling = false
    private var lastSentRow: Int?

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
        scrollView.scrollerStyle = .overlay
        scrollView.contentView.clipsToBounds = false
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
        liveScrollStartObserver = ObserverTokenBox(raw: NotificationCenter.default.addObserver(
            forName: NSScrollView.willStartLiveScrollNotification,
            object: scrollView,
            queue: .main
        ) { [weak self] _ in
            guard let self else { return }
            Task { @MainActor in
                self.isLiveScrolling = true
            }
        })
        liveScrollEndObserver = ObserverTokenBox(raw: NotificationCenter.default.addObserver(
            forName: NSScrollView.didEndLiveScrollNotification,
            object: scrollView,
            queue: .main
        ) { [weak self] _ in
            guard let self else { return }
            Task { @MainActor in
                self.isLiveScrolling = false
                self.sendScrollRowIfNeeded()
            }
        })
        liveScrollObserver = ObserverTokenBox(raw: NotificationCenter.default.addObserver(
            forName: NSScrollView.didLiveScrollNotification,
            object: scrollView,
            queue: .main
        ) { [weak self] _ in
            guard let self else { return }
            Task { @MainActor in
                self.sendScrollRowIfNeeded()
            }
        })
        preferredScrollerStyleObserver = ObserverTokenBox(raw: NotificationCenter.default.addObserver(
            forName: NSScroller.preferredScrollerStyleDidChangeNotification,
            object: nil,
            queue: nil
        ) { [weak self] _ in
            guard let self else { return }
            Task { @MainActor in
                self.handleScrollerStyleChange()
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
        if let liveScrollStartObserver {
            NotificationCenter.default.removeObserver(liveScrollStartObserver.raw)
        }
        if let liveScrollEndObserver {
            NotificationCenter.default.removeObserver(liveScrollEndObserver.raw)
        }
        if let liveScrollObserver {
            NotificationCenter.default.removeObserver(liveScrollObserver.raw)
        }
        if let preferredScrollerStyleObserver {
            NotificationCenter.default.removeObserver(preferredScrollerStyleObserver.raw)
        }
    }

    override func layout() {
        super.layout()
        scrollView.frame = bounds
        surfaceView.frame.size = scrollView.contentView.bounds.size
        documentView.frame.size.width = scrollView.bounds.width
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
        guard !isSyncingScroll, !isLiveScrolling else { return }
        let cellHeight = scene.info.surfaceMetrics.cellSizePoints.height
        let targetOrigin = CGPoint(x: 0, y: CGFloat(scene.info.scrollRow) * cellHeight)
        if scrollView.contentView.bounds.origin.y != targetOrigin.y {
            isSyncingScroll = true
            scrollView.contentView.scroll(to: targetOrigin)
            scrollView.reflectScrolledClipView(scrollView.contentView)
            isSyncingScroll = false
        }
        lastSentRow = scene.info.scrollRow
    }

    private func handleScrollChange() {
        synchronizeSurfaceFrame()
        guard !isSyncingScroll, !isLiveScrolling else { return }
        sendScrollRowIfNeeded()
    }

    private func handleScrollerStyleChange() {
        scrollView.scrollerStyle = .overlay
        updateTrackingAreas()
    }

    private func sendScrollRowIfNeeded() {
        let cellHeight = controller.scene?.info.surfaceMetrics.cellSizePoints.height ?? surfaceView.cellSize.height
        guard cellHeight > 0 else { return }
        let row = Int((scrollView.contentView.documentVisibleRect.origin.y / cellHeight).rounded(.down))
        guard row != lastSentRow else { return }
        lastSentRow = row
        controller.setScrollRow(row)
    }

    override func mouseMoved(with event: NSEvent) {
        guard NSScroller.preferredScrollerStyle == .legacy else { return }
        scrollView.flashScrollers()
    }

    override func updateTrackingAreas() {
        trackingAreas.forEach(removeTrackingArea)
        super.updateTrackingAreas()
        guard let scroller = scrollView.verticalScroller else { return }
        addTrackingArea(NSTrackingArea(
            rect: convert(scroller.bounds, from: scroller),
            options: [.mouseMoved, .activeInKeyWindow],
            owner: self,
            userInfo: nil
        ))
    }
}
