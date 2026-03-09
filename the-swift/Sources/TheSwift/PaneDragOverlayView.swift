import AppKit
import Foundation
import SwiftUI

enum PaneDragHandleLayout {
    static let handleHeight: CGFloat = 10
    static let handleHitHeight: CGFloat = 16
    static let handleIdealWidth: CGFloat = 36
    static let handleTopInset: CGFloat = 6

    static func resolvedHandleWidth(for paneFrame: CGRect) -> CGFloat {
        min(handleIdealWidth, max(14, paneFrame.width - 6))
    }

    static func handleFrame(for paneFrame: CGRect) -> CGRect {
        let width = resolvedHandleWidth(for: paneFrame)
        return CGRect(
            x: paneFrame.midX - (width / 2),
            y: paneFrame.minY + handleTopInset,
            width: width,
            height: handleHitHeight
        )
    }

    static func interactionFrame(for paneFrame: CGRect) -> CGRect {
        handleFrame(for: paneFrame)
    }
}

struct PaneDragPaneSnapshot: Identifiable, Equatable {
    let paneId: UInt64
    let frame: CGRect
    let isActive: Bool
    let previewTitle: String

    var id: UInt64 { paneId }
}

struct PaneDragOverlayView: View {
    enum DropEdge: UInt8, Equatable {
        case left = 0
        case right = 1
        case up = 2
        case down = 3

        static func calculate(at point: CGPoint, in size: CGSize) -> DropEdge {
            let width = max(1, size.width)
            let height = max(1, size.height)
            let relX = min(max(point.x / width, 0), 1)
            let relY = min(max(point.y / height, 0), 1)

            let distToLeft = relX
            let distToRight = 1 - relX
            let distToTop = relY
            let distToBottom = 1 - relY

            let minDist = min(distToLeft, distToRight, distToTop, distToBottom)
            if minDist == distToLeft { return .left }
            if minDist == distToRight { return .right }
            if minDist == distToTop { return .up }
            return .down
        }

        func previewRect(in frame: CGRect) -> CGRect {
            switch self {
            case .left:
                return CGRect(
                    x: frame.minX,
                    y: frame.minY,
                    width: frame.width / 2,
                    height: frame.height
                )
            case .right:
                return CGRect(
                    x: frame.midX,
                    y: frame.minY,
                    width: frame.width / 2,
                    height: frame.height
                )
            case .up:
                return CGRect(
                    x: frame.minX,
                    y: frame.minY,
                    width: frame.width,
                    height: frame.height / 2
                )
            case .down:
                return CGRect(
                    x: frame.minX,
                    y: frame.midY,
                    width: frame.width,
                    height: frame.height / 2
                )
            }
        }
    }

    private struct DropTarget: Equatable {
        let destinationPaneId: UInt64
        let destinationFrame: CGRect
        let edge: DropEdge

        var previewRect: CGRect {
            edge.previewRect(in: destinationFrame)
        }
    }

    private struct DragSession: Equatable {
        let sourcePaneId: UInt64
        let currentLocation: CGPoint
        let target: DropTarget?
    }

    private static let coordinateSpaceName = "PaneDragOverlay"
    private let dropPreviewInset: CGFloat = 7
    private let dragPreviewOffset = CGPoint(x: 14, y: -26)
    private let projectionBlue = Color(red: 0.36, green: 0.63, blue: 0.98)

    let panes: [PaneDragPaneSnapshot]
    let onMovePane: (UInt64, UInt64, UInt8) -> Void

    @State private var hoveredPaneId: UInt64? = nil
    @State private var dragSession: DragSession? = nil

    var body: some View {
        GeometryReader { proxy in
            ZStack(alignment: .topLeading) {
                if let target = dragSession?.target {
                    dropPreview(target)
                }

                if let dragSession {
                    dragPreview(dragSession, containerSize: proxy.size)
                }

                ForEach(panes) { pane in
                    handle(for: pane)
                        .frame(width: handleFrame(for: pane).width, height: handleFrame(for: pane).height)
                        .offset(x: handleFrame(for: pane).minX, y: handleFrame(for: pane).minY)
                }

                PaneDragInteractionLayer(
                    handleRegions: panes.map { pane in
                        PaneDragInteractionLayer.HandleRegion(
                            paneId: pane.paneId,
                            frame: interactionFrame(for: pane)
                        )
                    },
                    onHoverChanged: { paneId in
                        hoveredPaneId = paneId
                    },
                    onDragChanged: { paneId, location in
                        let target = dropTarget(at: location, sourcePaneId: paneId)
                        dragSession = DragSession(
                            sourcePaneId: paneId,
                            currentLocation: location,
                            target: target
                        )
                    },
                    onDragEnded: { paneId, location in
                        let target = dropTarget(at: location, sourcePaneId: paneId)
                        dragSession = nil
                        guard let target else {
                            return
                        }
                        onMovePane(paneId, target.destinationPaneId, target.edge.rawValue)
                    }
                )
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .coordinateSpace(name: Self.coordinateSpaceName)
    }

    @ViewBuilder
    private func dropPreview(_ target: DropTarget) -> some View {
        let rect = target.previewRect.insetBy(dx: dropPreviewInset, dy: dropPreviewInset)
        let cornerRadius = min(12, max(7, min(rect.width, rect.height) * 0.09))

        RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
            .fill(projectionBlue.opacity(0.16))
            .overlay {
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .stroke(projectionBlue.opacity(0.46), lineWidth: 1)
            }
            .frame(width: max(0, rect.width), height: max(0, rect.height))
            .offset(x: rect.minX, y: rect.minY)
            .shadow(color: projectionBlue.opacity(0.16), radius: 14, y: 2)
            .allowsHitTesting(false)
            .accessibilityHidden(true)
    }

    @ViewBuilder
    private func dragPreview(_ session: DragSession, containerSize: CGSize) -> some View {
        let title = panes.first(where: { $0.paneId == session.sourcePaneId })?.previewTitle ?? "Pane"
        let maxWidth: CGFloat = 260
        let textWidth = min(maxWidth, max(92, CGFloat(title.count) * 6.6 + 42))
        let height: CGFloat = 26
        let x = min(
            max(8, session.currentLocation.x + dragPreviewOffset.x),
            max(8, containerSize.width - textWidth - 8)
        )
        let y = min(
            max(8, session.currentLocation.y + dragPreviewOffset.y),
            max(8, containerSize.height - height - 8)
        )

        HStack(spacing: 8) {
            Text(title)
                .font(FontLoader.uiFont(size: 10).weight(.semibold))
                .foregroundStyle(Color.white.opacity(0.92))
                .lineLimit(1)
                .truncationMode(.middle)

            Text("×1")
                .font(FontLoader.uiFont(size: 9).weight(.semibold))
                .foregroundStyle(Color.white.opacity(0.72))
        }
        .padding(.horizontal, 9)
        .frame(width: textWidth, height: height, alignment: .leading)
        .background {
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .fill(Color.black.opacity(0.74))
        }
        .overlay {
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .stroke(Color.white.opacity(0.10), lineWidth: 0.6)
        }
        .shadow(color: Color.black.opacity(0.26), radius: 9, y: 3)
        .offset(x: x, y: y)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    private func handle(for pane: PaneDragPaneSnapshot) -> some View {
        let isHovering = hoveredPaneId == pane.paneId
        let isDragging = dragSession?.sourcePaneId == pane.paneId
        let handleWidth = resolvedHandleWidth(for: pane)
        let showBackground = isHovering || isDragging
        let fillOpacity = isDragging ? 0.16 : 0.10
        let dotOpacity = isDragging ? 0.70 : (isHovering ? 0.54 : 0.26)

        return ZStack {
            if showBackground {
                Capsule(style: .continuous)
                    .fill(Color.white.opacity(fillOpacity))
                    .overlay {
                        Capsule(style: .continuous)
                            .stroke(Color.white.opacity(0.08), lineWidth: 0.6)
                    }
                    .transition(.opacity)
            }

            Image(systemName: "ellipsis")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(Color.white.opacity(dotOpacity))
                .offset(y: -0.5)
        }
        .animation(.easeInOut(duration: 0.12), value: showBackground)
        .frame(width: handleWidth, height: PaneDragHandleLayout.handleHeight)
        .frame(width: max(handleWidth + 8, 28), height: PaneDragHandleLayout.handleHitHeight)
        .contentShape(Rectangle())
        .help("Drag Pane")
        .accessibilityLabel("Drag Pane")
    }

    private func handleFrame(for pane: PaneDragPaneSnapshot) -> CGRect {
        PaneDragHandleLayout.handleFrame(for: pane.frame)
    }

    private func resolvedHandleWidth(for pane: PaneDragPaneSnapshot) -> CGFloat {
        PaneDragHandleLayout.resolvedHandleWidth(for: pane.frame)
    }

    private func interactionFrame(for pane: PaneDragPaneSnapshot) -> CGRect {
        PaneDragHandleLayout.interactionFrame(for: pane.frame)
    }

    private func dropTarget(at point: CGPoint, sourcePaneId: UInt64) -> DropTarget? {
        let destination = panes.first { pane in
            pane.paneId != sourcePaneId && pane.frame.contains(point)
        }
        guard let destination else {
            return nil
        }
        let localPoint = CGPoint(
            x: point.x - destination.frame.minX,
            y: point.y - destination.frame.minY
        )
        return DropTarget(
            destinationPaneId: destination.paneId,
            destinationFrame: destination.frame,
            edge: DropEdge.calculate(at: localPoint, in: destination.frame.size)
        )
    }
}

private struct PaneDragInteractionLayer: NSViewRepresentable {
    struct HandleRegion: Equatable {
        let paneId: UInt64
        let frame: CGRect
    }

    let handleRegions: [HandleRegion]
    let onHoverChanged: (UInt64?) -> Void
    let onDragChanged: (UInt64, CGPoint) -> Void
    let onDragEnded: (UInt64, CGPoint) -> Void

    func makeNSView(context: Context) -> PaneDragInteractionNSView {
        let view = PaneDragInteractionNSView()
        view.handleRegions = handleRegions
        view.onHoverChanged = onHoverChanged
        view.onDragChanged = onDragChanged
        view.onDragEnded = onDragEnded
        return view
    }

    func updateNSView(_ nsView: PaneDragInteractionNSView, context: Context) {
        nsView.handleRegions = handleRegions
        nsView.onHoverChanged = onHoverChanged
        nsView.onDragChanged = onDragChanged
        nsView.onDragEnded = onDragEnded
    }
}

private final class PaneDragInteractionNSView: NSView {
    var handleRegions: [PaneDragInteractionLayer.HandleRegion] = [] {
        didSet {
            guard oldValue != handleRegions else { return }
            if activePaneId != nil, !handleRegions.contains(where: { $0.paneId == activePaneId }) {
                activePaneId = nil
            }
            if hoveredPaneId != nil, !handleRegions.contains(where: { $0.paneId == hoveredPaneId }) {
                hoveredPaneId = nil
                onHoverChanged?(nil)
            }
            window?.invalidateCursorRects(for: self)
        }
    }
    var onHoverChanged: ((UInt64?) -> Void)?
    var onDragChanged: ((UInt64, CGPoint) -> Void)?
    var onDragEnded: ((UInt64, CGPoint) -> Void)?

    private var trackingArea: NSTrackingArea?
    private var hoveredPaneId: UInt64?
    private var activePaneId: UInt64?

    override var isFlipped: Bool { true }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        window?.acceptsMouseMovedEvents = true
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea {
            removeTrackingArea(trackingArea)
        }
        let area = NSTrackingArea(
            rect: .zero,
            options: [.activeInKeyWindow, .inVisibleRect, .mouseMoved, .cursorUpdate, .mouseEnteredAndExited],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        trackingArea = area
    }

    override func resetCursorRects() {
        super.resetCursorRects()
        let cursor: NSCursor = activePaneId == nil ? .openHand : .closedHand
        if DiagnosticsDebugLog.paneDragCursorEnabled {
            DiagnosticsDebugLog.paneDragCursorLog(
                "drag.resetCursorRects active=\(activePaneId ?? 0) hovered=\(hoveredPaneId ?? 0) cursor=\(cursorName(cursor)) regions=\(handleRegionsSummary())"
            )
        }
        for region in handleRegions {
            let rect = region.frame.intersection(bounds)
            guard !rect.isEmpty else { continue }
            addCursorRect(rect, cursor: cursor)
        }
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        let localPoint = convert(point, from: superview)
        if activePaneId != nil {
            logCursorEvent("drag.hitTest", point: localPoint, extra: "active=1 result=self")
            return self
        }
        let region = handleRegion(at: localPoint)
        logCursorEvent(
            "drag.hitTest",
            point: localPoint,
            extra: "active=0 region=\(region?.paneId ?? 0) result=\(region == nil ? "nil" : "self")"
        )
        return region == nil ? nil : self
    }

    override func mouseMoved(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        updateHover(at: point)
        let cursor = preferredCursor(at: point)
        logCursorEvent(
            "drag.mouseMoved",
            point: point,
            extra: "hovered=\(hoveredPaneId ?? 0) active=\(activePaneId ?? 0) cursor=\(cursorName(cursor))"
        )
        cursor.set()
        super.mouseMoved(with: event)
    }

    override func cursorUpdate(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        let cursor = preferredCursor(at: point)
        logCursorEvent(
            "drag.cursorUpdate",
            point: point,
            extra: "hovered=\(hoveredPaneId ?? 0) active=\(activePaneId ?? 0) cursor=\(cursorName(cursor))"
        )
        cursor.set()
    }

    override func mouseEntered(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        updateHover(at: point)
        logCursorEvent("drag.mouseEntered", point: point, extra: "hovered=\(hoveredPaneId ?? 0)")
    }

    override func mouseExited(with event: NSEvent) {
        guard activePaneId == nil else { return }
        let point = convert(event.locationInWindow, from: nil)
        if hoveredPaneId != nil {
            hoveredPaneId = nil
            onHoverChanged?(nil)
        }
        logCursorEvent("drag.mouseExited", point: point, extra: "hovered=0")
    }

    override func mouseDown(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        guard let region = handleRegion(at: point) else {
            logCursorEvent("drag.mouseDown", point: point, extra: "region=0 ignored=1")
            return
        }
        activePaneId = region.paneId
        if hoveredPaneId != region.paneId {
            hoveredPaneId = region.paneId
            onHoverChanged?(region.paneId)
        }
        window?.invalidateCursorRects(for: self)
        logCursorEvent("drag.mouseDown", point: point, extra: "region=\(region.paneId) active=\(region.paneId)")
        NSCursor.closedHand.set()
    }

    override func mouseDragged(with event: NSEvent) {
        guard let activePaneId else {
            return
        }
        let point = convert(event.locationInWindow, from: nil)
        onDragChanged?(activePaneId, point)
        logCursorEvent("drag.mouseDragged", point: point, extra: "active=\(activePaneId)")
        NSCursor.closedHand.set()
    }

    override func mouseUp(with event: NSEvent) {
        guard let activePaneId else {
            return
        }
        let point = convert(event.locationInWindow, from: nil)
        onDragEnded?(activePaneId, point)
        self.activePaneId = nil
        updateHover(at: point)
        window?.invalidateCursorRects(for: self)
        let cursor = preferredCursor(at: point)
        logCursorEvent(
            "drag.mouseUp",
            point: point,
            extra: "released=\(activePaneId) hovered=\(hoveredPaneId ?? 0) cursor=\(cursorName(cursor))"
        )
        cursor.set()
    }

    private func handleRegion(at point: CGPoint) -> PaneDragInteractionLayer.HandleRegion? {
        handleRegions.first(where: { $0.frame.contains(point) })
    }

    private func updateHover(at point: CGPoint) {
        guard activePaneId == nil else { return }
        let nextHoveredPaneId = handleRegion(at: point)?.paneId
        guard hoveredPaneId != nextHoveredPaneId else {
            return
        }
        hoveredPaneId = nextHoveredPaneId
        logCursorEvent("drag.hoverChanged", point: point, extra: "hovered=\(nextHoveredPaneId ?? 0)")
        onHoverChanged?(nextHoveredPaneId)
    }

    private func preferredCursor(at point: CGPoint) -> NSCursor {
        if activePaneId != nil {
            return .closedHand
        }
        if handleRegion(at: point) != nil {
            return .openHand
        }
        return .arrow
    }

    private func logCursorEvent(_ event: String, point: CGPoint, extra: String) {
        guard DiagnosticsDebugLog.paneDragCursorEnabled else { return }
        DiagnosticsDebugLog.paneDragCursorLog(
            "\(event) point=(\(fmt(point.x)),\(fmt(point.y))) region=\(handleRegion(at: point)?.paneId ?? 0) \(extra)"
        )
    }

    private func handleRegionsSummary() -> String {
        handleRegions.map { region in
            "\(region.paneId):\(fmt(region.frame.minX)),\(fmt(region.frame.minY)),\(fmt(region.frame.width))x\(fmt(region.frame.height))"
        }
        .joined(separator: "|")
    }

    private func cursorName(_ cursor: NSCursor) -> String {
        if cursor === NSCursor.openHand {
            return "openHand"
        }
        if cursor === NSCursor.closedHand {
            return "closedHand"
        }
        if cursor === NSCursor.arrow {
            return "arrow"
        }
        if cursor === NSCursor.iBeam {
            return "iBeam"
        }
        return "other"
    }

    private func fmt(_ value: CGFloat) -> String {
        String(format: "%.1f", value)
    }
}
