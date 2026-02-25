import AppKit
import SwiftUI

enum PickerPanelLayout {
    case center, top, bottom
}

struct PickerPanelVirtualListConfig {
    let rowHeight: CGFloat
    let rowSpacing: CGFloat
    let verticalPadding: CGFloat
    let horizontalPadding: CGFloat
    let overscanRows: Int
}

private struct PickerRowFramePreferenceKey: PreferenceKey {
    static var defaultValue: [Int: CGRect] = [:]

    static func reduce(value: inout [Int: CGRect], nextValue: () -> [Int: CGRect]) {
        value.merge(nextValue(), uniquingKeysWith: { _, new in new })
    }
}

private struct PickerViewportFramePreferenceKey: PreferenceKey {
    static var defaultValue: CGRect = .zero

    static func reduce(value: inout CGRect, nextValue: () -> CGRect) {
        value = nextValue()
    }
}

private struct PickerNativeScrollRequest: Equatable {
    let id: Int
    let rowTop: CGFloat
    let rowHeight: CGFloat
}

private final class PickerNativeScrollDocumentView: NSView {
    override var isFlipped: Bool { true }
}

private final class PickerNativeScrollContainerView: NSView {
    let scrollView: NSScrollView = NSScrollView()
    private let documentView = PickerNativeScrollDocumentView()
    private let hostingView = NSHostingView(rootView: AnyView(EmptyView()))
    private var currentContentHeight: CGFloat = 0

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setup()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) not implemented")
    }

    override var isFlipped: Bool { true }

    private func setup() {
        scrollView.borderType = .noBorder
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.usesPredominantAxisScrolling = true
        scrollView.scrollerStyle = .overlay
        scrollView.contentView.postsBoundsChangedNotifications = true
        scrollView.documentView = documentView

        documentView.addSubview(hostingView)
        addSubview(scrollView)
    }

    override func layout() {
        super.layout()
        scrollView.frame = bounds
        layoutDocument()
    }

    func setRootView(_ rootView: AnyView, contentHeight: CGFloat) {
        hostingView.rootView = rootView
        currentContentHeight = max(0, contentHeight)
        layoutDocument()
    }

    private func layoutDocument() {
        let width = max(0, scrollView.contentSize.width)
        let height = max(0, currentContentHeight)
        documentView.frame = NSRect(x: 0, y: 0, width: width, height: height)
        hostingView.frame = documentView.bounds
    }
}

private struct NativePickerScrollView<Content: View>: NSViewRepresentable {
    let contentHeight: CGFloat
    let scrollRequest: PickerNativeScrollRequest?
    let onScrollMetrics: (CGFloat, CGFloat) -> Void
    let content: Content

    init(
        contentHeight: CGFloat,
        scrollRequest: PickerNativeScrollRequest?,
        onScrollMetrics: @escaping (CGFloat, CGFloat) -> Void,
        @ViewBuilder content: () -> Content
    ) {
        self.contentHeight = contentHeight
        self.scrollRequest = scrollRequest
        self.onScrollMetrics = onScrollMetrics
        self.content = content()
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(onScrollMetrics: onScrollMetrics)
    }

    func makeNSView(context: Context) -> PickerNativeScrollContainerView {
        let view = PickerNativeScrollContainerView(frame: .zero)
        context.coordinator.bind(to: view.scrollView)
        return view
    }

    func updateNSView(_ nsView: PickerNativeScrollContainerView, context: Context) {
        let coordinator = context.coordinator
        coordinator.onScrollMetrics = onScrollMetrics
        coordinator.bind(to: nsView.scrollView)
        nsView.setRootView(AnyView(content), contentHeight: contentHeight)
        coordinator.applyScrollRequest(scrollRequest, contentHeight: contentHeight)
        DispatchQueue.main.async {
            coordinator.reportMetrics()
            coordinator.applyScrollRequest(scrollRequest, contentHeight: contentHeight)
        }
    }

    static func dismantleNSView(_ nsView: PickerNativeScrollContainerView, coordinator: Coordinator) {
        _ = nsView
        coordinator.unbind()
    }

    final class Coordinator {
        var onScrollMetrics: (CGFloat, CGFloat) -> Void

        private weak var scrollView: NSScrollView?
        private var observers: [NSObjectProtocol] = []
        private var isLiveScrolling = false
        private var lastHandledRequestId: Int = -1

        init(onScrollMetrics: @escaping (CGFloat, CGFloat) -> Void) {
            self.onScrollMetrics = onScrollMetrics
        }

        deinit {
            unbind()
        }

        func bind(to scrollView: NSScrollView) {
            if self.scrollView === scrollView { return }
            unbind()
            self.scrollView = scrollView

            observers.append(NotificationCenter.default.addObserver(
                forName: NSView.boundsDidChangeNotification,
                object: scrollView.contentView,
                queue: .main
            ) { [weak self] _ in
                self?.reportMetrics()
            })

            observers.append(NotificationCenter.default.addObserver(
                forName: NSScrollView.willStartLiveScrollNotification,
                object: scrollView,
                queue: .main
            ) { [weak self] _ in
                self?.isLiveScrolling = true
            })

            observers.append(NotificationCenter.default.addObserver(
                forName: NSScrollView.didEndLiveScrollNotification,
                object: scrollView,
                queue: .main
            ) { [weak self] _ in
                self?.isLiveScrolling = false
                self?.reportMetrics()
            })
        }

        func unbind() {
            observers.forEach { NotificationCenter.default.removeObserver($0) }
            observers.removeAll()
            scrollView = nil
            isLiveScrolling = false
            lastHandledRequestId = -1
        }

        func reportMetrics() {
            guard let scrollView else { return }
            let bounds = scrollView.contentView.bounds
            onScrollMetrics(max(0, bounds.minY), max(0, bounds.height))
        }

        func applyScrollRequest(_ request: PickerNativeScrollRequest?, contentHeight: CGFloat) {
            guard let request else { return }
            guard request.id != lastHandledRequestId else { return }
            guard let scrollView else { return }

            let rowHeight = max(1, request.rowHeight)
            let maxRowTop = max(0, contentHeight - rowHeight)
            let rowTop = max(0, min(request.rowTop, maxRowTop))
            let width = max(1, scrollView.contentSize.width)
            let rect = NSRect(x: 0, y: rowTop, width: width, height: rowHeight)

            // Native minimal scroll behavior (only scroll when needed).
            scrollView.documentView?.scrollToVisible(rect)
            scrollView.reflectScrolledClipView(scrollView.contentView)
            lastHandledRequestId = request.id
            reportMetrics()
        }
    }
}

struct PickerPanel<
    LeadingHeader: View,
    TrailingHeader: View,
    ItemContent: View,
    EmptyContent: View
>: View {
    // Configuration
    let width: CGFloat
    let maxListHeight: CGFloat
    let placeholder: String
    let fontSize: CGFloat
    let layout: PickerPanelLayout
    let pageSize: Int
    let showTabNavigation: Bool
    let showPageNavigation: Bool
    let showCtrlCClose: Bool
    let autoSelectFirstItem: Bool
    var showBackground: Bool = true
    var virtualList: PickerPanelVirtualListConfig? = nil

    // Data
    let itemCount: Int
    let externalQuery: String
    let externalSelectedIndex: Int?

    // Callbacks
    let onQueryChange: (String) -> Void
    let onSubmit: (Int?) -> Void
    let onClose: () -> Void
    let onSelectionChange: ((Int) -> Void)?

    // Content
    @ViewBuilder let leadingHeader: () -> LeadingHeader
    @ViewBuilder let trailingHeader: () -> TrailingHeader
    let itemContent: (_ index: Int, _ isSelected: Bool, _ isHovered: Bool) -> ItemContent
    @ViewBuilder let emptyContent: () -> EmptyContent

    // Internal state
    @State private var query: String = ""
    @State private var selectedIndex: Int? = nil
    @State private var hoveredIndex: Int? = nil
    @State private var rowFrames: [Int: CGRect] = [:]
    @State private var viewportFrame: CGRect = .zero
    @State private var visibleIndexRange: ClosedRange<Int>? = nil
    @State private var lastProgrammaticScrollIndex: Int? = nil
    @State private var nativeScrollTop: CGFloat = 0
    @State private var nativeViewportHeight: CGFloat = 0
    @State private var nativeScrollRequest: PickerNativeScrollRequest? = nil
    @State private var nativeScrollRequestCounter: Int = 0
    @FocusState private var isTextFieldFocused: Bool

    private let backgroundColor: Color = Color(nsColor: .windowBackgroundColor)

    var body: some View {
        panelContainer
            .background(
                PickerKeyInterceptor(
                    onMoveSelection: { delta in moveSelection(delta) },
                    onClose: showCtrlCClose ? { onClose() } : nil,
                    onTextInput: { chars in
                        query.append(chars)
                        isTextFieldFocused = true
                    },
                    onBackspace: {
                        if !query.isEmpty { query.removeLast() }
                        isTextFieldFocused = true
                    },
                    isTextFieldFocused: isTextFieldFocused,
                    pageSize: pageSize,
                    showTabNavigation: showTabNavigation,
                    showPageNavigation: showPageNavigation
                )
                .frame(width: 0, height: 0)
            )
            .onAppear {
                query = externalQuery
                selectedIndex = initialSelection()
                lastProgrammaticScrollIndex = nil
                DispatchQueue.main.async {
                    isTextFieldFocused = true
                }
                if let sel = selectedIndex {
                    onSelectionChange?(sel)
                }
            }
            .onChange(of: externalQuery) { newValue in
                if newValue != query {
                    query = newValue
                }
                lastProgrammaticScrollIndex = nil
                syncSelection()
            }
            .onChange(of: itemCount) { _ in
                lastProgrammaticScrollIndex = nil
                syncSelection()
            }
            .onChange(of: selectedIndex) { newValue in
                normalizeSelection(newValue)
            }
    }

    // MARK: - Layout

    private var panelContainer: some View {
        Group {
            if !showBackground {
                panelCard
            } else {
                Group {
                    switch layout {
                    case .bottom:
                        VStack {
                            Spacer()
                            panelCard
                        }
                        .padding(.horizontal, 24)
                        .padding(.bottom, 24)
                    case .top:
                        VStack {
                            panelCard
                            Spacer()
                        }
                        .padding(.horizontal, 24)
                        .padding(.top, 24)
                    case .center:
                        panelCard
                            .padding()
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: layoutAlignment)
            }
        }
    }

    private var layoutAlignment: Alignment {
        switch layout {
        case .bottom: return .bottom
        case .top: return .top
        case .center: return .center
        }
    }

    // MARK: - Glass card

    private var panelCard: some View {
        let content = VStack(alignment: .leading, spacing: 0) {
            panelHeader
            Divider()
            panelList
        }
        .frame(maxWidth: width)

        return Group {
            if showBackground {
                content
                    .background(
                        ZStack {
                            Rectangle()
                                .fill(.ultraThinMaterial)
                            Rectangle()
                                .fill(backgroundColor)
                                .blendMode(.color)
                        }
                        .compositingGroup()
                    )
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                    .overlay(
                        RoundedRectangle(cornerRadius: 12)
                            .stroke(Color(nsColor: .tertiaryLabelColor).opacity(0.75))
                    )
                    .shadow(radius: 28, x: 0, y: 12)
            } else {
                content
            }
        }
    }

    // MARK: - Header

    private var panelHeader: some View {
        HStack(spacing: 8) {
            leadingHeader()

            TextField(placeholder, text: $query)
                .font(FontLoader.uiFont(size: fontSize).weight(.light))
                .textFieldStyle(.plain)
                .focused($isTextFieldFocused)
                .onSubmit {
                    onSubmit(clampedIndex(selectedIndex))
                }
                .onExitCommand {
                    onClose()
                }
                .onChange(of: query) { newValue in
                    if newValue != externalQuery {
                        onQueryChange(newValue)
                    }
                }

            Spacer()

            trailingHeader()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    // MARK: - List

    @ViewBuilder
    private var panelList: some View {
        if itemCount == 0 {
            emptyContent()
                .frame(maxHeight: maxListHeight)
        } else if let virtualList {
            nativeVirtualizedPanelList(config: virtualList)
        } else {
            legacyPanelList
        }
    }

    private var legacyPanelList: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 4) {
                    ForEach(0..<itemCount, id: \.self) { index in
                        rowContainer(index: index, fixedRowHeight: nil, trackFrame: true)
                            .id(index)
                    }
                }
                .padding(10)
            }
            .frame(maxHeight: maxListHeight)
            .background(
                GeometryReader { proxy in
                    Color.clear.preference(
                        key: PickerViewportFramePreferenceKey.self,
                        value: proxy.frame(in: .global)
                    )
                }
            )
            .onPreferenceChange(PickerRowFramePreferenceKey.self) { frames in
                rowFrames = frames
                recomputeVisibleIndexRange()
            }
            .onPreferenceChange(PickerViewportFramePreferenceKey.self) { frame in
                viewportFrame = frame
                recomputeVisibleIndexRange()
            }
            .onChange(of: selectedIndex) { newIndex in
                guard let index = newIndex else { return }
                scrollSelectionIntoView(index: index, proxy: proxy)
            }
            .onAppear {
                guard let index = selectedIndex else { return }
                scrollSelectionIntoView(index: index, proxy: proxy)
            }
        }
    }

    private func nativeVirtualizedPanelList(config: PickerPanelVirtualListConfig) -> some View {
        let renderRange = virtualizedRenderRange(config: config)
        let renderIndices = Array(renderRange)
        let topSpacer = virtualTopSpacerHeight(before: renderRange.lowerBound, config: config)
        let bottomSpacer = virtualBottomSpacerHeight(after: renderRange.upperBound, config: config)
        let contentHeight = virtualContentHeight(config: config)

        return NativePickerScrollView(
            contentHeight: contentHeight,
            scrollRequest: nativeScrollRequest
        ) { top, viewportHeight in
            nativeScrollTop = top
            nativeViewportHeight = viewportHeight
            recomputeVisibleIndexRangeNative(config: config)
        } content: {
            VStack(alignment: .leading, spacing: 0) {
                if topSpacer > 0 {
                    Color.clear.frame(height: topSpacer)
                }

                if !renderIndices.isEmpty {
                    VStack(alignment: .leading, spacing: config.rowSpacing) {
                        ForEach(renderIndices, id: \.self) { index in
                            rowContainer(index: index, fixedRowHeight: config.rowHeight, trackFrame: false)
                        }
                    }
                }

                if bottomSpacer > 0 {
                    Color.clear.frame(height: bottomSpacer)
                }
            }
            .padding(.horizontal, config.horizontalPadding)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(maxHeight: maxListHeight)
        .onChange(of: selectedIndex) { newIndex in
            guard let index = newIndex else { return }
            scrollSelectionIntoViewNative(index: index, config: config)
        }
        .onAppear {
            recomputeVisibleIndexRangeNative(config: config)
            guard let index = selectedIndex else { return }
            scrollSelectionIntoViewNative(index: index, config: config)
        }
        .onChange(of: itemCount) { _ in
            recomputeVisibleIndexRangeNative(config: config)
        }
    }

    @ViewBuilder
    private func rowContainer(index: Int, fixedRowHeight: CGFloat?, trackFrame: Bool) -> some View {
        let row = rowButton(index: index, fixedRowHeight: fixedRowHeight)
        if trackFrame {
            row.background(
                GeometryReader { proxy in
                    Color.clear.preference(
                        key: PickerRowFramePreferenceKey.self,
                        value: [index: proxy.frame(in: .global)]
                    )
                }
            )
        } else {
            row
        }
    }

    private func rowButton(index: Int, fixedRowHeight: CGFloat?) -> some View {
        let isSelected = selectedIndex == index
        let isHovered = hoveredIndex == index

        return Button {
            selectedIndex = index
            onSelectionChange?(index)
            onSubmit(index)
        } label: {
            itemContent(index, isSelected, isHovered)
                .padding(8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .frame(height: fixedRowHeight, alignment: .topLeading)
                .background(
                    RoundedRectangle(cornerRadius: 6)
                        .fill(isSelected ? Color.accentColor.opacity(0.2) : (isHovered ? Color.secondary.opacity(0.15) : Color.clear))
                )
        }
        .buttonStyle(.plain)
        .contentShape(Rectangle())
        .onHover { hovering in
            hoveredIndex = hovering ? index : nil
        }
    }

    private func virtualizedRenderRange(config: PickerPanelVirtualListConfig) -> ClosedRange<Int> {
        guard itemCount > 0 else { return 0...0 }

        let visible = visibleIndexRange ?? estimatedVisibleIndexRange(config: config)
        let overscan = max(0, config.overscanRows)
        let start = max(0, visible.lowerBound - overscan)
        let end = min(itemCount - 1, visible.upperBound + overscan)
        return start...max(start, end)
    }

    private func estimatedVisibleIndexRange(config: PickerPanelVirtualListConfig) -> ClosedRange<Int> {
        guard itemCount > 0 else { return 0...0 }

        let step = virtualRowStep(config: config)
        let fallbackViewport = max(1, maxListHeight)
        let estimatedVisible = max(1, Int(ceil(fallbackViewport / max(1, step))))
        let anchor = clampedIndex(selectedIndex) ?? 0
        let end = min(itemCount - 1, anchor + estimatedVisible - 1)
        return anchor...max(anchor, end)
    }

    private func recomputeVisibleIndexRangeNative(config: PickerPanelVirtualListConfig) {
        guard itemCount > 0 else {
            visibleIndexRange = nil
            return
        }
        guard nativeViewportHeight > 0 else {
            visibleIndexRange = nil
            return
        }

        let step = virtualRowStep(config: config)
        let startY = max(0, nativeScrollTop - config.verticalPadding)
        let endY = max(0, nativeScrollTop + nativeViewportHeight - config.verticalPadding)

        let first = max(0, min(itemCount - 1, Int(floor(startY / max(1, step)))))
        let last = max(0, min(itemCount - 1, Int(floor(max(0, endY - 1) / max(1, step)))))
        visibleIndexRange = first...max(first, last)
    }

    private func virtualRowStep(config: PickerPanelVirtualListConfig) -> CGFloat {
        max(1, config.rowHeight + config.rowSpacing)
    }

    private func virtualRowTop(_ index: Int, config: PickerPanelVirtualListConfig) -> CGFloat {
        config.verticalPadding + (CGFloat(max(0, index)) * virtualRowStep(config: config))
    }

    private func virtualContentHeight(config: PickerPanelVirtualListConfig) -> CGFloat {
        guard itemCount > 0 else {
            return config.verticalPadding * 2
        }
        let rowsHeight = CGFloat(itemCount) * config.rowHeight
        let spacingHeight = CGFloat(max(0, itemCount - 1)) * max(0, config.rowSpacing)
        return (config.verticalPadding * 2) + rowsHeight + spacingHeight
    }

    private func virtualTopSpacerHeight(before startIndex: Int, config: PickerPanelVirtualListConfig) -> CGFloat {
        guard itemCount > 0 else { return config.verticalPadding }
        return virtualRowTop(startIndex, config: config)
    }

    private func virtualBottomSpacerHeight(after endIndex: Int, config: PickerPanelVirtualListConfig) -> CGFloat {
        guard itemCount > 0 else { return config.verticalPadding }
        let remainingRows = max(0, itemCount - endIndex - 1)
        return config.verticalPadding + (CGFloat(remainingRows) * virtualRowStep(config: config))
    }

    private func scrollSelectionIntoViewNative(index: Int, config: PickerPanelVirtualListConfig) {
        nativeScrollRequestCounter += 1
        nativeScrollRequest = PickerNativeScrollRequest(
            id: nativeScrollRequestCounter,
            rowTop: virtualRowTop(index, config: config),
            rowHeight: max(1, config.rowHeight)
        )
        lastProgrammaticScrollIndex = index
    }

    // MARK: - Selection logic

    private func moveSelection(_ delta: Int) {
        guard itemCount > 0 else { return }

        let len = itemCount
        let next: Int
        if let current = clampedIndex(selectedIndex) {
            let raw = current + delta
            next = ((raw % len) + len) % len
        } else {
            next = delta >= 0 ? 0 : (len - 1)
        }

        selectedIndex = next
        onSelectionChange?(next)
    }

    private func syncSelection() {
        if itemCount == 0 {
            selectedIndex = nil
            return
        }
        let prev = selectedIndex
        if !autoSelectFirstItem {
            selectedIndex = clampedIndex(externalSelectedIndex)
        } else {
            selectedIndex = clampedIndex(selectedIndex)
                ?? clampedIndex(externalSelectedIndex)
                ?? (autoSelectFirstItem ? 0 : nil)
        }
        if selectedIndex != prev, let sel = selectedIndex {
            onSelectionChange?(sel)
        }
    }

    private func clampedIndex(_ index: Int?) -> Int? {
        guard itemCount > 0 else { return nil }
        guard let index else { return nil }
        return max(0, min(index, itemCount - 1))
    }

    private func initialSelection() -> Int? {
        guard itemCount > 0 else { return nil }
        return clampedIndex(externalSelectedIndex) ?? (autoSelectFirstItem ? 0 : nil)
    }

    private func normalizeSelection(_ newValue: Int?) {
        guard itemCount > 0 else {
            if selectedIndex != nil {
                selectedIndex = nil
            }
            return
        }

        let normalized = clampedIndex(newValue) ?? (autoSelectFirstItem ? 0 : nil)
        if selectedIndex != normalized {
            selectedIndex = normalized
        }
    }

    private func scrollSelectionIntoView(index: Int, proxy: ScrollViewProxy) {
        guard shouldScrollSelectionIntoView(index: index) else { return }

        var transaction = Transaction()
        transaction.animation = nil
        withTransaction(transaction) {
            // nil anchor keeps native "only scroll when needed" behavior.
            proxy.scrollTo(index, anchor: nil)
        }
        lastProgrammaticScrollIndex = index
    }

    private func shouldScrollSelectionIntoView(index: Int) -> Bool {
        if lastProgrammaticScrollIndex == index {
            return false
        }

        guard let visible = visibleIndexRange else {
            return true
        }

        let span = max(0, visible.upperBound - visible.lowerBound)
        let margin = min(2, span / 4)
        let safeLower = visible.lowerBound + margin
        let safeUpper = visible.upperBound - margin

        if safeLower <= safeUpper, index >= safeLower, index <= safeUpper {
            return false
        }
        return true
    }

    private func recomputeVisibleIndexRange() {
        guard viewportFrame != .zero, !rowFrames.isEmpty else {
            visibleIndexRange = nil
            return
        }

        let visibleIndices = rowFrames
            .compactMap { index, frame -> Int? in
                let intersects = frame.maxY > viewportFrame.minY && frame.minY < viewportFrame.maxY
                return intersects ? index : nil
            }
            .sorted()

        guard let first = visibleIndices.first, let last = visibleIndices.last else {
            visibleIndexRange = nil
            return
        }
        visibleIndexRange = first...last
    }
}

// MARK: - NSEvent-based key interceptor

/// Intercepts navigation keys (arrows, Ctrl+P/N, Tab, Ctrl+U/D, Ctrl+C) at the
/// NSEvent level, bypassing SwiftUI's keyboard shortcut system which can miss
/// events when a TextField has focus.
///
/// Also intercepts printable characters and backspace so that typing always
/// goes to the search field, even if another view (e.g. preview panel) has
/// stolen keyboard focus.
private struct PickerKeyInterceptor: NSViewRepresentable {
    let onMoveSelection: (Int) -> Void
    let onClose: (() -> Void)?
    let onTextInput: ((String) -> Void)?
    let onBackspace: (() -> Void)?
    let isTextFieldFocused: Bool
    let pageSize: Int
    let showTabNavigation: Bool
    let showPageNavigation: Bool

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        let coordinator = context.coordinator
        coordinator.isActive = true
        coordinator.monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            guard coordinator.isActive else { return event }
            return coordinator.handleKey(event)
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        let c = context.coordinator
        c.onMoveSelection = onMoveSelection
        c.onClose = onClose
        c.onTextInput = onTextInput
        c.onBackspace = onBackspace
        c.isTextFieldFocused = isTextFieldFocused
        c.pageSize = pageSize
        c.showTabNavigation = showTabNavigation
        c.showPageNavigation = showPageNavigation
    }

    static func dismantleNSView(_ nsView: NSView, coordinator: Coordinator) {
        coordinator.isActive = false
        if let monitor = coordinator.monitor {
            NSEvent.removeMonitor(monitor)
            coordinator.monitor = nil
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(
            onMoveSelection: onMoveSelection,
            onClose: onClose,
            onTextInput: onTextInput,
            onBackspace: onBackspace,
            isTextFieldFocused: isTextFieldFocused,
            pageSize: pageSize,
            showTabNavigation: showTabNavigation,
            showPageNavigation: showPageNavigation
        )
    }

    class Coordinator {
        var onMoveSelection: (Int) -> Void
        var onClose: (() -> Void)?
        var onTextInput: ((String) -> Void)?
        var onBackspace: (() -> Void)?
        var isTextFieldFocused: Bool
        var pageSize: Int
        var showTabNavigation: Bool
        var showPageNavigation: Bool
        var monitor: Any?
        var isActive: Bool = false

        init(
            onMoveSelection: @escaping (Int) -> Void,
            onClose: (() -> Void)?,
            onTextInput: ((String) -> Void)?,
            onBackspace: (() -> Void)?,
            isTextFieldFocused: Bool,
            pageSize: Int,
            showTabNavigation: Bool,
            showPageNavigation: Bool
        ) {
            self.onMoveSelection = onMoveSelection
            self.onClose = onClose
            self.onTextInput = onTextInput
            self.onBackspace = onBackspace
            self.isTextFieldFocused = isTextFieldFocused
            self.pageSize = pageSize
            self.showTabNavigation = showTabNavigation
            self.showPageNavigation = showPageNavigation
        }

        deinit {
            if let monitor { NSEvent.removeMonitor(monitor) }
        }

        func handleKey(_ event: NSEvent) -> NSEvent? {
            let keyCode = event.keyCode
            let mods = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            let importantMods = mods.intersection([.command, .option, .control, .shift])
            let chars = event.charactersIgnoringModifiers ?? ""

            // Up arrow (no modifiers)
            if keyCode == 126 && importantMods.isEmpty {
                onMoveSelection(-1)
                return nil
            }

            // Down arrow (no modifiers)
            if keyCode == 125 && importantMods.isEmpty {
                onMoveSelection(1)
                return nil
            }

            // Ctrl+P / Ctrl+N
            if importantMods == [.control] {
                if chars == "p" { onMoveSelection(-1); return nil }
                if chars == "n" { onMoveSelection(1); return nil }
            }

            // Tab / Shift+Tab
            if showTabNavigation && keyCode == 48 {
                if importantMods.isEmpty { onMoveSelection(1); return nil }
                if importantMods == [.shift] { onMoveSelection(-1); return nil }
            }

            // Ctrl+U / Ctrl+D (page navigation)
            if showPageNavigation && importantMods == [.control] {
                if chars == "u" { onMoveSelection(-pageSize); return nil }
                if chars == "d" { onMoveSelection(pageSize); return nil }
            }

            // Ctrl+C (close)
            if let onClose, importantMods == [.control] && chars == "c" {
                onClose()
                return nil
            }

            // When the TextField has focus, let it handle text input natively
            // (including selection-aware backspace, Cmd+A, etc.)
            if isTextFieldFocused {
                return event
            }

            // Below: TextField does NOT have focus (e.g. preview panel stole it).
            // Intercept text input and forward to the query.

            // Let Cmd-key combos pass through (Cmd+C for copy, Cmd+V for paste, etc.)
            if importantMods.contains(.command) {
                return event
            }

            // Backspace — forward to query
            if keyCode == 51 && importantMods.isEmpty {
                onBackspace?()
                return nil
            }

            // Printable characters (no control/option modifiers) — forward to query.
            if !importantMods.contains(.control) && !importantMods.contains(.option) {
                if let typed = event.characters, !typed.isEmpty {
                    let scalar = typed.unicodeScalars.first!
                    // Only forward actual printable characters (not function keys, etc.)
                    if scalar.value >= 0x20 && scalar.value < 0xF700 {
                        onTextInput?(typed)
                        return nil
                    }
                }
            }

            return event
        }
    }
}
