import AppKit
import SwiftUI

private let completionPanelEdgePadding: CGFloat = 8
private let completionRowHeight: CGFloat = 24
private let completionHorizontalPadding: CGFloat = 10

struct EditorCompletionMenuView: View {
    @ObservedObject var controller: EditorSurfaceController

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                if let scene = controller.scene, controller.completionMenu.isOpen {
                    let frame = completionFrame(scene: scene, state: controller.completionMenu)
                    EditorPopoverPanel(frame: frame, backgroundColor: controller.chrome.backgroundColor) {
                        EditorCompletionListPanel(
                            controller: controller,
                            completion: controller.completionMenu,
                            frameWidth: frame.width
                        )
                    }
                    .zIndex(3)

                    if controller.completionDocs.isOpen {
                        EditorDocsPanelOverlay(
                            kind: .completionDocs,
                            panel: controller.completionDocs,
                            scene: scene,
                            backgroundColor: controller.chrome.backgroundColor,
                            anchorFrame: frame,
                            onEscape: controller.closeCompletionMenu
                        )
                        .zIndex(4)
                    }
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height, alignment: .topLeading)
        }
        .allowsHitTesting(true)
    }

    private func completionFrame(scene: EditorRenderScene, state: EditorCompletionMenuState) -> CGRect {
        let metrics = scene.info.surfaceMetrics
        let viewportSize = CGSize(
            width: CGFloat(scene.info.viewportWidth) * metrics.cellSizePoints.width,
            height: CGFloat(scene.info.viewportHeight) * metrics.cellSizePoints.height
        )
        let baseOrigin = scene.displayOrigin(col: state.col, row: state.row)
        let exportedSize = CGSize(
            width: CGFloat(state.width) * metrics.cellSizePoints.width,
            height: CGFloat(state.height) * metrics.cellSizePoints.height
        )
        let fittedWidth = min(max(contentWidth(for: state), 260), min(max(exportedSize.width, 260), 460))
        let fittedHeight = min(
            max(CGFloat(min(state.items.count, 10)) * completionRowHeight + 8, exportedSize.height),
            max(viewportSize.height - completionPanelEdgePadding * 2, completionRowHeight)
        )
        let x = min(max(baseOrigin.x, completionPanelEdgePadding), max(viewportSize.width - fittedWidth - completionPanelEdgePadding, completionPanelEdgePadding))
        let y = completionAnchoredOriginY(
            scene: scene,
            viewportHeight: viewportSize.height,
            baseOriginY: baseOrigin.y,
            exportedHeight: exportedSize.height,
            fittedHeight: fittedHeight
        )
        return CGRect(x: x, y: y, width: fittedWidth, height: fittedHeight)
    }

    private func completionAnchoredOriginY(
        scene: EditorRenderScene,
        viewportHeight: CGFloat,
        baseOriginY: CGFloat,
        exportedHeight: CGFloat,
        fittedHeight: CGFloat
    ) -> CGFloat {
        let lowerBound = completionPanelEdgePadding
        let upperBound = max(viewportHeight - fittedHeight - completionPanelEdgePadding, completionPanelEdgePadding)
        guard let cursor = scene.primaryCursor else {
            return min(max(baseOriginY, lowerBound), upperBound)
        }

        let gap: CGFloat = 6
        let cellHeight = scene.info.surfaceMetrics.cellSizePoints.height
        let cursorTopY = scene.displayOrigin(col: cursor.col, row: cursor.row).y
        let cursorBottomY = cursorTopY + cellHeight
        let exportedBottom = baseOriginY + exportedHeight

        let belowY = cursorBottomY + gap
        let aboveY = cursorTopY - gap - fittedHeight
        let prefersBelow = baseOriginY >= cursorBottomY
        let prefersAbove = exportedBottom <= cursorTopY

        if prefersBelow, belowY <= upperBound {
            return min(max(belowY, lowerBound), upperBound)
        }
        if prefersAbove, aboveY >= lowerBound {
            return min(max(aboveY, lowerBound), upperBound)
        }

        let roomBelow = viewportHeight - completionPanelEdgePadding - (cursorBottomY + gap)
        let roomAbove = cursorTopY - gap - completionPanelEdgePadding
        let fallbackY = roomBelow >= roomAbove ? belowY : aboveY
        return min(max(fallbackY, lowerBound), upperBound)
    }

    private func contentWidth(for state: EditorCompletionMenuState) -> CGFloat {
        let titleFont = NSFont.systemFont(ofSize: 12, weight: .medium)
        let subtitleFont = NSFont.systemFont(ofSize: 11, weight: .regular)
        let iconWidth: CGFloat = state.items.contains(where: { $0.leadingIcon != nil }) ? 18 : 0
        let widest = state.items.reduce(CGFloat.zero) { partial, item in
            let titleWidth = (item.title as NSString).size(withAttributes: [.font: titleFont]).width
            let subtitleWidth = item.subtitle.map { ($0 as NSString).size(withAttributes: [.font: subtitleFont]).width } ?? 0
            return max(partial, titleWidth + subtitleWidth + iconWidth + completionHorizontalPadding * 2 + 16)
        }
        let stepperColumn: CGFloat = state.items.count > 1 ? 30 : 0
        return widest + stepperColumn
    }
}

private struct EditorCompletionListPanel: View {
    @ObservedObject var controller: EditorSurfaceController
    let completion: EditorCompletionMenuState
    let frameWidth: CGFloat

    @State private var isLiveScrolling = false

    private var visibleRows: Int {
        min(max(completion.items.count, 1), 10)
    }

    var body: some View {
        HStack(spacing: 0) {
            CompletionMenuOffsetScrollView(
                rowHeight: completionRowHeight,
                offset: completion.scrollOffset,
                totalRows: completion.items.count,
                visibleRows: visibleRows,
                onOffsetChange: { controller.setCompletionMenuScroll($0) },
                onLiveScrollChanged: { isScrolling in
                    guard isLiveScrolling != isScrolling else { return }
                    isLiveScrolling = isScrolling
                }
            ) {
                LazyVStack(spacing: 0) {
                    ForEach(completion.items) { item in
                        EditorCompletionRow(
                            item: item,
                            isSelected: completion.selectedIndex == item.index,
                            hoverSelectionEnabled: !isLiveScrolling,
                            onSelect: { controller.selectCompletionMenuIndex(item.index) },
                            onSubmit: {
                                controller.selectCompletionMenuIndex(item.index)
                                controller.submitCompletionMenu()
                            }
                        )
                        .id(item.index)
                    }
                }
                .padding(.vertical, 4)
            }

            if completion.items.count > 1 {
                VStack(spacing: 6) {
                    Spacer(minLength: 0)
                    Button {
                        controller.stepCompletionMenuSelection(forward: false)
                    } label: {
                        Image(systemName: "chevron.up")
                            .font(.system(size: 11, weight: .semibold))
                    }
                    .buttonStyle(.borderless)
                    .foregroundStyle(.secondary)
                    .frame(width: 26, height: 22)
                    .contentShape(Rectangle())
                    .help("Previous suggestion")

                    Button {
                        controller.stepCompletionMenuSelection(forward: true)
                    } label: {
                        Image(systemName: "chevron.down")
                            .font(.system(size: 11, weight: .semibold))
                    }
                    .buttonStyle(.borderless)
                    .foregroundStyle(.secondary)
                    .frame(width: 26, height: 22)
                    .contentShape(Rectangle())
                    .help("Next suggestion")
                    Spacer(minLength: 0)
                }
                .frame(width: 28)
                .padding(.trailing, 2)
                .padding(.vertical, 4)
            }
        }
        .frame(width: frameWidth)
    }
}

private struct CompletionMenuOffsetScrollView<Content: View>: NSViewRepresentable {
    let rowHeight: CGFloat
    let offset: Int
    let totalRows: Int
    let visibleRows: Int
    let onOffsetChange: (Int) -> Void
    let onLiveScrollChanged: (Bool) -> Void
    let content: Content

    init(
        rowHeight: CGFloat,
        offset: Int,
        totalRows: Int,
        visibleRows: Int,
        onOffsetChange: @escaping (Int) -> Void,
        onLiveScrollChanged: @escaping (Bool) -> Void,
        @ViewBuilder content: () -> Content
    ) {
        self.rowHeight = rowHeight
        self.offset = offset
        self.totalRows = totalRows
        self.visibleRows = visibleRows
        self.onOffsetChange = onOffsetChange
        self.onLiveScrollChanged = onLiveScrollChanged
        self.content = content()
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(
            rowHeight: rowHeight,
            totalRows: totalRows,
            visibleRows: visibleRows,
            onOffsetChange: onOffsetChange,
            onLiveScrollChanged: onLiveScrollChanged
        )
    }

    func makeNSView(context: Context) -> CompletionHostingScrollView {
        let scrollView = CompletionHostingScrollView()
        context.coordinator.attach(to: scrollView)
        return scrollView
    }

    func updateNSView(_ nsView: CompletionHostingScrollView, context: Context) {
        context.coordinator.rowHeight = rowHeight
        context.coordinator.totalRows = totalRows
        context.coordinator.visibleRows = visibleRows
        context.coordinator.onOffsetChange = onOffsetChange
        context.coordinator.onLiveScrollChanged = onLiveScrollChanged
        nsView.update(rootView: AnyView(content))
        nsView.layoutDocumentView()
        context.coordinator.applyExternalOffset(offset, in: nsView)
    }

    @MainActor
    final class Coordinator: NSObject {
        var rowHeight: CGFloat
        var totalRows: Int
        var visibleRows: Int
        var onOffsetChange: (Int) -> Void
        var onLiveScrollChanged: (Bool) -> Void
        private weak var scrollView: CompletionHostingScrollView?
        private var isApplyingExternalOffset = false
        private var isLiveScrolling = false
        private var lastSentOffset: Int?
        private var pendingOffset: Int?

        init(
            rowHeight: CGFloat,
            totalRows: Int,
            visibleRows: Int,
            onOffsetChange: @escaping (Int) -> Void,
            onLiveScrollChanged: @escaping (Bool) -> Void
        ) {
            self.rowHeight = rowHeight
            self.totalRows = totalRows
            self.visibleRows = visibleRows
            self.onOffsetChange = onOffsetChange
            self.onLiveScrollChanged = onLiveScrollChanged
        }

        deinit {
            NotificationCenter.default.removeObserver(self)
        }

        func attach(to scrollView: CompletionHostingScrollView) {
            self.scrollView = scrollView
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(handleBoundsDidChangeNotification(_:)),
                name: NSView.boundsDidChangeNotification,
                object: scrollView.contentView
            )
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(handleWillStartLiveScroll(_:)),
                name: NSScrollView.willStartLiveScrollNotification,
                object: scrollView
            )
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(handleDidLiveScroll(_:)),
                name: NSScrollView.didLiveScrollNotification,
                object: scrollView
            )
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(handleDidEndLiveScroll(_:)),
                name: NSScrollView.didEndLiveScrollNotification,
                object: scrollView
            )
        }

        func applyExternalOffset(_ offset: Int, in scrollView: CompletionHostingScrollView) {
            lastSentOffset = clampedOffset(offset)
            guard !isLiveScrolling else { return }
            let targetY = CGFloat(clampedOffset(offset)) * rowHeight
            let currentY = clampY(scrollView.contentView.bounds.origin.y)
            guard abs(currentY - targetY) > max(rowHeight * 0.25, 0.5) else { return }
            isApplyingExternalOffset = true
            scrollView.contentView.scroll(to: NSPoint(x: 0, y: targetY))
            scrollView.reflectScrolledClipView(scrollView.contentView)
            isApplyingExternalOffset = false
        }

        @objc private func handleBoundsDidChangeNotification(_ notification: Notification) {
            boundsDidChange()
        }

        @objc private func handleWillStartLiveScroll(_ notification: Notification) {
            isLiveScrolling = true
            pendingOffset = nil
            onLiveScrollChanged(true)
        }

        @objc private func handleDidLiveScroll(_ notification: Notification) {
            boundsDidChange()
        }

        @objc private func handleDidEndLiveScroll(_ notification: Notification) {
            isLiveScrolling = false
            onLiveScrollChanged(false)
            if let pendingOffset, pendingOffset != lastSentOffset {
                lastSentOffset = pendingOffset
                self.pendingOffset = nil
                onOffsetChange(pendingOffset)
            } else {
                boundsDidChange()
            }
            if let scrollView, let lastSentOffset {
                applyExternalOffset(lastSentOffset, in: scrollView)
            }
        }

        private func boundsDidChange() {
            guard !isApplyingExternalOffset, let scrollView else { return }
            let y = clampY(scrollView.contentView.bounds.origin.y)
            let offset = clampedOffset(Int(floor(y / max(rowHeight, 1))))
            guard offset != lastSentOffset else { return }
            if isLiveScrolling {
                pendingOffset = offset
                return
            }
            lastSentOffset = offset
            onOffsetChange(offset)
        }

        private func maxOffset() -> Int {
            max(totalRows - max(visibleRows, 1), 0)
        }

        private func clampedOffset(_ offset: Int) -> Int {
            min(max(offset, 0), maxOffset())
        }

        private func clampY(_ y: CGFloat) -> CGFloat {
            min(max(y, 0), CGFloat(maxOffset()) * rowHeight)
        }
    }
}

@MainActor
private final class CompletionHostingScrollView: NSScrollView {
    private let documentContainer = CompletionDocumentContainerView()
    private let hostingView = NSHostingView(rootView: AnyView(EmptyView()))

    init() {
        super.init(frame: .zero)
        drawsBackground = false
        borderType = .noBorder
        hasVerticalScroller = true
        hasHorizontalScroller = false
        autohidesScrollers = true
        usesPredominantAxisScrolling = true
        scrollerStyle = .overlay
        verticalScrollElasticity = .automatic
        horizontalScrollElasticity = .automatic
        contentView.postsBoundsChangedNotifications = true
        documentView = documentContainer
        documentContainer.addSubview(hostingView)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func update(rootView: AnyView) {
        hostingView.rootView = rootView
    }

    func layoutDocumentView() {
        let width = max(contentSize.width, 1)
        let fittingSize = hostingView.fittingSize
        let height = max(fittingSize.height, contentSize.height)
        documentContainer.frame = NSRect(x: 0, y: 0, width: width, height: height)
        hostingView.frame = NSRect(x: 0, y: 0, width: width, height: fittingSize.height)
    }

    override func layout() {
        super.layout()
        layoutDocumentView()
    }
}

private final class CompletionDocumentContainerView: NSView {
    override var isFlipped: Bool { true }
}

private struct EditorCompletionRow: View {
    let item: EditorCompletionMenuItem
    let isSelected: Bool
    let hoverSelectionEnabled: Bool
    let onSelect: () -> Void
    let onSubmit: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            if let icon = item.leadingIcon {
                Group {
                    if icon.count == 1 {
                        Text(icon)
                            .font(.system(size: 12, weight: .medium))
                    } else {
                        Text(editorCompletionLeadingText(icon: icon))
                            .font(.custom(EditorIconFont.postScriptName, size: 12))
                    }
                }
                .foregroundStyle(Color(nsColor: item.leadingColor?.color ?? .secondaryLabelColor))
                .frame(width: 14, alignment: .center)
            }

            Text(item.title)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(isSelected ? .primary : .primary)
                .lineLimit(1)

            if let subtitle = item.subtitle, !subtitle.isEmpty {
                Text(subtitle)
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            Spacer(minLength: 0)
        }
        .padding(.horizontal, completionHorizontalPadding)
        .frame(maxWidth: .infinity, minHeight: completionRowHeight, alignment: .leading)
        .background(selectionBackground)
        .contentShape(Rectangle())
        .onHover { isHovering in
            if isHovering, hoverSelectionEnabled {
                onSelect()
            }
        }
        .onTapGesture(count: 2, perform: onSubmit)
        .onTapGesture(perform: onSelect)
    }

    @ViewBuilder
    private var selectionBackground: some View {
        if isSelected {
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .fill(Color.accentColor.opacity(0.18))
                .padding(.horizontal, 4)
                .padding(.vertical, 1)
        } else {
            Color.clear
        }
    }
}
