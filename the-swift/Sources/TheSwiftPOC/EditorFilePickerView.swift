import AppKit
import SwiftUI

struct EditorFilePickerView: View {
    @ObservedObject var controller: EditorSurfaceController
    @State private var localQuery: String = ""
    @State private var suppressQueryCallback = false
    @FocusState private var isQueryFocused: Bool

    private let listRowHeight: CGFloat = 34
    private let previewRowHeight: CGFloat = 16
    private let panelWidth: CGFloat = 880
    private let panelHeight: CGFloat = 520

    var body: some View {
        ZStack {
            if controller.filePicker.isOpen {
                GeometryReader { geometry in
                    let contentHeight = max(panelFrame(in: geometry.size).height - 72, 1)
                    let listVisibleRows = max(Int(contentHeight / listRowHeight), 1)
                    let previewVisibleRows = max(Int(contentHeight / previewRowHeight), 1)

                    ZStack {
                        Color.black.opacity(0.12)
                            .ignoresSafeArea()
                            .onTapGesture {
                                controller.closeFilePicker()
                            }

                        browserPanel(
                            in: panelFrame(in: geometry.size),
                            listVisibleRows: listVisibleRows,
                            previewVisibleRows: previewVisibleRows
                        )
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .onAppear {
                        controller.configureFilePicker(listVisibleRows: listVisibleRows, previewVisibleRows: previewVisibleRows)
                        localQuery = controller.filePicker.query
                        DispatchQueue.main.async {
                            isQueryFocused = true
                        }
                    }
                    .onChange(of: listVisibleRows) { _, rows in
                        controller.configureFilePicker(listVisibleRows: rows, previewVisibleRows: previewVisibleRows)
                    }
                    .onChange(of: previewVisibleRows) { _, rows in
                        controller.configureFilePicker(listVisibleRows: listVisibleRows, previewVisibleRows: rows)
                    }
                }
            }
        }
        .onChange(of: controller.filePicker.query) { _, newValue in
            guard localQuery != newValue else { return }
            suppressQueryCallback = true
            localQuery = newValue
            DispatchQueue.main.async {
                suppressQueryCallback = false
            }
        }
        .onChange(of: controller.filePicker.isOpen) { _, isOpen in
            if isOpen {
                localQuery = controller.filePicker.query
                DispatchQueue.main.async {
                    isQueryFocused = true
                }
            } else {
                DispatchQueue.main.async {
                    controller.focusEditor()
                }
            }
        }
    }

    private func browserPanel(in frame: CGRect, listVisibleRows: Int, previewVisibleRows: Int) -> some View {
        let nsBackgroundColor = controller.scene?.backgroundColor ?? .windowBackgroundColor
        let backgroundColor = Color(nsColor: nsBackgroundColor)
        let scheme: ColorScheme = pickerUsesLightScheme(nsBackgroundColor) ? .light : .dark

        return VStack(spacing: 0) {
            queryBar

            Divider()

            HStack(spacing: 0) {
                resultsPane(listVisibleRows: listVisibleRows)
                    .frame(width: min(frame.width * 0.36, 320))

                Divider()

                if controller.filePicker.showPreview {
                    previewPane(previewVisibleRows: previewVisibleRows)
                }
            }
        }
        .frame(width: frame.width, height: frame.height)
        .background(
            ZStack {
                RoundedRectangle(cornerRadius: 14)
                    .fill(.ultraThinMaterial)
                RoundedRectangle(cornerRadius: 14)
                    .fill(backgroundColor)
                    .blendMode(.color)
            }
            .compositingGroup()
        )
        .clipShape(RoundedRectangle(cornerRadius: 14))
        .overlay(
            RoundedRectangle(cornerRadius: 14)
                .stroke(Color(nsColor: .separatorColor).opacity(0.7), lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.22), radius: 30, x: 0, y: 18)
        .environment(\.colorScheme, scheme)
    }

    private var queryBar: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(.secondary)

                VStack(alignment: .leading, spacing: 2) {
                    TextField(controller.filePicker.title, text: $localQuery)
                        .textFieldStyle(.plain)
                        .font(.system(size: 16, weight: .regular))
                        .focused($isQueryFocused)
                        .onSubmit {
                            controller.submitFilePicker()
                        }
                        .onExitCommand {
                            controller.closeFilePicker()
                        }
                        .onMoveCommand {
                            switch $0 {
                            case .up, .down:
                                controller.moveFilePickerSelection($0)
                            default:
                                break
                            }
                        }
                        .onChange(of: localQuery) { _, newValue in
                            guard !suppressQueryCallback else { return }
                            controller.setFilePickerQuery(newValue)
                        }

                    HStack(spacing: 8) {
                        Text(resultSummary)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        if let statusBanner = controller.filePicker.statusBanner {
                            Text(statusBanner.text)
                                .font(.caption)
                                .foregroundStyle(statusBanner.kind == .warning ? .red : .secondary)
                        }
                        if let error = controller.filePicker.error, !error.isEmpty {
                            Text(error)
                                .font(.caption)
                                .foregroundStyle(.red)
                        }
                    }
                }

                Spacer(minLength: 0)

                if let modeLabel = searchModeLabel {
                    HStack(spacing: 6) {
                        Text(modeLabel)
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(searchModeColor)
                        Button {
                            controller.cycleFilePickerSearchMode()
                        } label: {
                            Image(systemName: "arrow.triangle.2.circlepath")
                                .font(.system(size: 12, weight: .semibold))
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(.secondary)
                        .help("Cycle search mode (plain · regex · fuzzy)")
                        .keyboardShortcut(.tab, modifiers: [.shift])
                    }
                }

                hiddenMovementButtons

                if controller.filePicker.isLoading {
                    ProgressView()
                        .controlSize(.small)
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
        }
    }

    private var hiddenMovementButtons: some View {
        Group {
            Button { controller.moveFilePickerSelection(.up) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.upArrow, modifiers: [])
            Button { controller.moveFilePickerSelection(.down) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.downArrow, modifiers: [])
        }
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }

    private func resultsPane(listVisibleRows: Int) -> some View {
        VStack(spacing: 0) {
            NativeVerticalOffsetScrollView(
                rowHeight: listRowHeight,
                offset: controller.filePicker.visibleItemStart,
                totalRows: controller.filePicker.matchedCount,
                visibleRows: listVisibleRows,
                onOffsetChange: controller.setFilePickerListOffset
            ) {
                VStack(alignment: .leading, spacing: 0) {
                    Color.clear.frame(height: CGFloat(controller.filePicker.visibleItemStart) * listRowHeight)

                    if controller.filePicker.items.isEmpty {
                        Text(controller.filePicker.isLoading ? "Searching…" : "No matches")
                            .font(.system(size: 12))
                            .foregroundStyle(.secondary)
                            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                            .padding(14)
                            .frame(height: listRowHeight * 3, alignment: .topLeading)
                    } else {
                        VStack(alignment: .leading, spacing: 0) {
                            ForEach(controller.filePicker.items) { item in
                                EditorFilePickerRow(
                                    item: item,
                                    query: controller.filePicker.query,
                                    isSelected: controller.filePicker.selectedIndex == item.globalIndex,
                                    onSelect: {
                                        controller.selectFilePickerIndex(item.globalIndex)
                                    },
                                    onOpen: {
                                        controller.submitFilePicker(index: item.globalIndex)
                                    }
                                )
                                .frame(height: listRowHeight)
                            }
                        }
                    }

                    Color.clear.frame(height: CGFloat(max(controller.filePicker.matchedCount - controller.filePicker.visibleItemStart - controller.filePicker.items.count, 0)) * listRowHeight)
                }
                .padding(.horizontal, 8)
            }

            Divider()

            HStack {
                Text(resultsVisibleSummary)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                Spacer()
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
        }
        .background(Color.clear)
    }

    private func previewPane(previewVisibleRows: Int) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(controller.filePicker.previewPath ?? "Preview")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(.primary)
                        .lineLimit(1)
                    Text(previewSummary)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)

            Divider()

            NativeVerticalOffsetScrollView(
                rowHeight: previewRowHeight,
                offset: controller.filePicker.previewOffset,
                totalRows: controller.filePicker.previewTotalRows,
                visibleRows: previewVisibleRows,
                onOffsetChange: controller.setFilePickerPreviewOffset
            ) {
                VStack(alignment: .leading, spacing: 0) {
                    Color.clear.frame(height: CGFloat(controller.filePicker.previewWindowStart) * previewRowHeight)

                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(controller.filePicker.previewLines) { line in
                            EditorFilePickerPreviewLineView(line: line)
                                .frame(height: previewRowHeight, alignment: .center)
                        }
                    }

                    Color.clear.frame(height: CGFloat(max(controller.filePicker.previewTotalRows - controller.filePicker.previewWindowStart - controller.filePicker.previewLines.count, 0)) * previewRowHeight)
                }
                .padding(.horizontal, 6)
            }

            Divider()

            HStack {
                Text(previewVisibleSummary)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                Spacer()
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(Color.clear)
        }
    }

    private var resultSummary: String {
        let state = controller.filePicker
        if state.isLoading && state.matchedCount == 0 {
            return "Searching…"
        }
        if state.matchedCount == 1 {
            return "1 result"
        }
        return "\(state.matchedCount) results"
    }

    private var previewSummary: String {
        let state = controller.filePicker
        switch state.previewNavigationMode {
        case .anchored, .scrollable:
            return state.previewTotalRows == 1 ? "1 line" : "\(state.previewTotalRows) lines"
        case .static:
            return "Preview"
        }
    }

    private var searchModeLabel: String? {
        let state = controller.filePicker
        guard state.kind == .liveGrep else { return nil }
        switch state.searchMode {
        case .none:
            return nil
        case .plainText:
            return "plain"
        case .regex:
            return "regex"
        case .fuzzy:
            return "fuzzy"
        }
    }

    private var searchModeColor: Color {
        switch controller.filePicker.searchMode {
        case .plainText:
            return .secondary
        case .regex:
            return .orange
        case .fuzzy:
            return .cyan
        case .none:
            return .secondary
        }
    }

    private var resultsVisibleSummary: String {
        let state = controller.filePicker
        guard state.matchedCount > 0 else { return resultSummary }
        let start = state.visibleItemStart + 1
        let end = state.visibleItemStart + state.items.count
        return "Showing \(start)–\(end) of \(state.matchedCount)"
    }

    private var previewVisibleSummary: String {
        let state = controller.filePicker
        guard state.previewTotalRows > 0 else { return previewSummary }
        let start = state.previewOffset + 1
        let end = min(state.previewOffset + state.previewLines.count, state.previewTotalRows)
        return "Lines \(start)–\(end) of \(state.previewTotalRows)"
    }

    private func pickerUsesLightScheme(_ color: NSColor) -> Bool {
        guard let color = color.usingColorSpace(.sRGB) else { return false }
        let luminance = (0.299 * color.redComponent) + (0.587 * color.greenComponent) + (0.114 * color.blueComponent)
        return luminance > 0.7
    }

    private func panelFrame(in containerSize: CGSize) -> CGRect {
        CGRect(
            x: max((containerSize.width - min(panelWidth, containerSize.width - 48)) / 2, 24),
            y: max((containerSize.height - min(panelHeight, containerSize.height - 56)) / 2 - 18, 20),
            width: min(panelWidth, containerSize.width - 48),
            height: min(panelHeight, containerSize.height - 56)
        )
    }
}

/// One space per tab keeps UTF-16 lengths stable so match highlight ranges stay valid.
private func filePickerDisplayText(_ text: String) -> String {
    text.replacingOccurrences(of: "\t", with: " ")
}

private struct EditorFilePickerRow: View {
    let item: EditorFilePickerItem
    let query: String
    let isSelected: Bool
    let onSelect: () -> Void
    let onOpen: () -> Void
    @State private var isHovered = false

    var body: some View {
        Group {
            switch item.rowKind {
            case .liveGrepHeader:
                liveGrepHeaderRow
            case .liveGrepMatch:
                liveGrepMatchRow
            default:
                standardRow
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
        .background(rowBackground)
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .contentShape(RoundedRectangle(cornerRadius: 6))
        .onTapGesture {
            guard item.selectable else { return }
            onSelect()
        }
        .onTapGesture(count: 2) {
            guard item.selectable else { return }
            onOpen()
        }
        .onHover { isHovered = $0 }
        .accessibilityElement(children: .combine)
    }

    private var standardRow: some View {
        HStack(spacing: 10) {
            if showsIcon {
                EditorSemanticIconView(iconName: item.icon, isDirectory: item.isDirectory, size: 13)
                    .frame(width: 18)
                    .foregroundStyle(iconColor)
            }

            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 8) {
                    Text(primaryAttributedText)
                        .font(.system(size: 13, weight: .regular))
                        .lineLimit(1)

                    if let tertiary = item.tertiary, !tertiary.isEmpty {
                        Text(filePickerDisplayText(tertiary))
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                }

                HStack(spacing: 8) {
                    if let secondary = item.secondary, !secondary.isEmpty {
                        Text(secondaryAttributedText(secondary))
                            .font(.caption2)
                            .lineLimit(1)
                    }
                    if let quaternary = item.quaternary, !quaternary.isEmpty {
                        Text(filePickerDisplayText(quaternary))
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                            .lineLimit(1)
                    }
                }
            }

            Spacer(minLength: 0)

            if item.line > 0 {
                Text("\(item.line):\(max(item.column, 1))")
                    .font(.caption2.monospacedDigit())
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var liveGrepHeaderRow: some View {
        HStack(spacing: 8) {
            Text(filePickerDisplayText(item.primary))
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.primary)
                .lineLimit(1)
            if let secondary = item.secondary, !secondary.isEmpty {
                Text(filePickerDisplayText(secondary))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
        }
    }

    private var liveGrepMatchRow: some View {
        HStack(spacing: 8) {
            Text(lineLabel)
                .font(.system(size: 12, weight: .regular))
                .foregroundStyle(.secondary)
                .frame(width: 56, alignment: .trailing)

            Text(highlightedText(filePickerDisplayText(item.primary), ranges: item.primaryMatchRanges, baseColor: .primary))
                .font(.system(size: 13, weight: .regular))
                .lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)

            Spacer(minLength: 0)
        }
    }

    private var lineLabel: String {
        ":\(item.line):\(max(item.column, 1))"
    }

    private var showsIcon: Bool {
        switch item.rowKind {
        case .liveGrepHeader, .liveGrepMatch:
            return false
        default:
            return !item.icon.isEmpty
        }
    }

    private var primaryAttributedText: AttributedString {
        highlightedText(
            filePickerDisplayText(item.primary),
            ranges: item.primaryMatchRanges,
            baseColor: item.selectable ? .primary : .secondary
        )
    }

    private func secondaryAttributedText(_ text: String) -> AttributedString {
        highlightedText(
            filePickerDisplayText(text),
            ranges: item.secondaryMatchRanges,
            baseColor: .secondary
        )
    }

    private func highlightedText(_ text: String, ranges: [Range<Int>], baseColor: Color) -> AttributedString {
        var attributed = AttributedString(text)
        attributed.foregroundColor = baseColor

        let effectiveRanges = ranges.isEmpty ? fallbackMatchRanges(in: text) : ranges
        for range in effectiveRanges.reversed() {
            guard let attributedRange = attributedRange(in: attributed, range: range) else { continue }
            attributed[attributedRange].backgroundColor = Color.accentColor.opacity(isSelected ? 0.28 : 0.18)
            attributed[attributedRange].foregroundColor = .primary
        }
        return attributed
    }

    private func fallbackMatchRanges(in text: String) -> [Range<Int>] {
        let needle = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !needle.isEmpty else { return [] }
        let lowerText = text.lowercased()
        let lowerNeedle = needle.lowercased()
        guard let range = lowerText.range(of: lowerNeedle) else { return [] }
        let start = lowerText.distance(from: lowerText.startIndex, to: range.lowerBound)
        let end = lowerText.distance(from: lowerText.startIndex, to: range.upperBound)
        return [start..<end]
    }

    private func attributedRange(in attributed: AttributedString, range: Range<Int>) -> Range<AttributedString.Index>? {
        guard range.lowerBound >= 0, range.upperBound > range.lowerBound else { return nil }
        let characters = attributed.characters
        guard let start = characters.index(characters.startIndex, offsetBy: range.lowerBound, limitedBy: characters.endIndex),
              let end = characters.index(characters.startIndex, offsetBy: range.upperBound, limitedBy: characters.endIndex),
              start < end
        else {
            return nil
        }
        return start..<end
    }

    private var rowBackground: some ShapeStyle {
        if isSelected {
            return AnyShapeStyle(Color.accentColor.opacity(0.2))
        }
        if isHovered {
            return AnyShapeStyle(Color.secondary.opacity(0.14))
        }
        return AnyShapeStyle(Color.clear)
    }

    private var iconColor: Color {
        switch item.rowKind {
        case .diagnostics:
            return .orange
        case .symbols:
            return .accentColor
        case .liveGrepHeader:
            return .secondary
        case .liveGrepMatch:
            return .accentColor
        case .vcsDiffHeader, .vcsDiffHunk:
            return .green
        case .generic:
            return item.isDirectory ? .accentColor : .secondary
        }
    }

    }

private let filePickerPreviewLineNumberColumnWidth: CGFloat = 26
private let filePickerPreviewLineNumberTrailing: CGFloat = 6

private struct EditorFilePickerPreviewLineView: View {
    let line: EditorFilePickerPreviewLine

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 0) {
            if let lineNumber = line.lineNumber {
                Text("\(lineNumber)")
                    .font(.system(size: 11, weight: .regular))
                    .foregroundStyle(.secondary)
                    .frame(width: filePickerPreviewLineNumberColumnWidth, alignment: .trailing)
                    .padding(.trailing, filePickerPreviewLineNumberTrailing)
                    .textSelection(.disabled)
            } else {
                Color.clear
                    .frame(width: filePickerPreviewLineNumberColumnWidth + filePickerPreviewLineNumberTrailing)
            }

            Text(attributedContent)
                .font(.system(size: 11, weight: .regular))
                .lineLimit(1)
                .truncationMode(.tail)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.leading, 2)
        .padding(.trailing, 4)
        .padding(.vertical, 1)
        .background(lineBackground)
        .clipped()
    }

    private var attributedContent: AttributedString {
        var attributed = AttributedString(filePickerDisplayText(line.marker ?? ""))
        if !attributed.characters.isEmpty {
            attributed.foregroundColor = .secondary
        }

        for segment in line.segments {
            var piece = AttributedString(filePickerDisplayText(segment.text))
            piece.foregroundColor = Color(nsColor: segment.style.foregroundColor)
            if segment.isMatch {
                piece.backgroundColor = Color.accentColor.opacity(0.22)
            } else if let background = segment.style.backgroundColor {
                piece.backgroundColor = Color(nsColor: background).opacity(0.7)
            }
            attributed.append(piece)
        }

        return attributed
    }

    private var lineBackground: some View {
        Group {
            switch line.kind {
            case .added:
                Color.green.opacity(0.08)
            case .removed:
                Color.red.opacity(0.08)
            case .modified:
                Color.orange.opacity(0.08)
            default:
                line.focused ? Color.accentColor.opacity(0.12) : Color.clear
            }
        }
    }
}

private struct NativeVerticalOffsetScrollView<Content: View>: NSViewRepresentable {
    let rowHeight: CGFloat
    let offset: Int
    let totalRows: Int
    let visibleRows: Int
    let onOffsetChange: (Int) -> Void
    let content: Content

    init(
        rowHeight: CGFloat,
        offset: Int,
        totalRows: Int,
        visibleRows: Int,
        onOffsetChange: @escaping (Int) -> Void,
        @ViewBuilder content: () -> Content
    ) {
        self.rowHeight = rowHeight
        self.offset = offset
        self.totalRows = totalRows
        self.visibleRows = visibleRows
        self.onOffsetChange = onOffsetChange
        self.content = content()
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(rowHeight: rowHeight, totalRows: totalRows, visibleRows: visibleRows, onOffsetChange: onOffsetChange)
    }

    func makeNSView(context: Context) -> PickerHostingScrollView {
        let scrollView = PickerHostingScrollView()
        context.coordinator.attach(to: scrollView)
        return scrollView
    }

    func updateNSView(_ nsView: PickerHostingScrollView, context: Context) {
        context.coordinator.rowHeight = rowHeight
        context.coordinator.totalRows = totalRows
        context.coordinator.visibleRows = visibleRows
        context.coordinator.onOffsetChange = onOffsetChange
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
        private weak var scrollView: PickerHostingScrollView?
        private var isApplyingExternalOffset = false
        private var isLiveScrolling = false
        private var lastSentOffset: Int?

        init(rowHeight: CGFloat, totalRows: Int, visibleRows: Int, onOffsetChange: @escaping (Int) -> Void) {
            self.rowHeight = rowHeight
            self.totalRows = totalRows
            self.visibleRows = visibleRows
            self.onOffsetChange = onOffsetChange
        }

        deinit {
            NotificationCenter.default.removeObserver(self)
        }

        func attach(to scrollView: PickerHostingScrollView) {
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

        func applyExternalOffset(_ offset: Int, in scrollView: PickerHostingScrollView) {
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
        }

        @objc private func handleDidLiveScroll(_ notification: Notification) {
            boundsDidChange()
        }

        @objc private func handleDidEndLiveScroll(_ notification: Notification) {
            isLiveScrolling = false
            boundsDidChange()
            if let scrollView, let lastSentOffset {
                applyExternalOffset(lastSentOffset, in: scrollView)
            }
        }

        private func boundsDidChange() {
            guard !isApplyingExternalOffset, let scrollView else { return }
            let y = clampY(scrollView.contentView.bounds.origin.y)
            let offset = clampedOffset(Int(floor(y / max(rowHeight, 1))))
            guard offset != lastSentOffset else { return }
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
private final class PickerHostingScrollView: NSScrollView {
    private let documentContainer = FlippedDocumentContainerView()
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

@MainActor
private final class FlippedDocumentContainerView: NSView {
    override var isFlipped: Bool { true }
}
