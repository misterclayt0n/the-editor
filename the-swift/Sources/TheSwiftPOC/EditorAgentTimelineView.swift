import AppKit
import Foundation
import SwiftUI

struct EditorAgentTimelineEntry: Identifiable, Equatable, Hashable {
    enum Kind: String, Hashable {
        case user
        case assistant
        case note
        case tool
    }

    let id: String
    let kind: Kind
    let title: String?
    let text: String
    let isStreaming: Bool
    let status: String?
    let contextSummary: String?
    let renderedMarkdown: EditorRenderedMarkdown?
    let revisionToken: Int

    init(
        id: String,
        kind: Kind,
        title: String?,
        text: String,
        isStreaming: Bool,
        status: String?,
        contextSummary: String?,
        renderedMarkdown: EditorRenderedMarkdown?,
        revisionToken: Int
    ) {
        self.id = id
        self.kind = kind
        self.title = title
        self.text = text
        self.isStreaming = isStreaming
        self.status = status
        self.contextSummary = contextSummary
        self.renderedMarkdown = renderedMarkdown
        self.revisionToken = revisionToken
    }

    init(item: EditorAgentTranscriptItem) {
        self.init(
            id: item.id,
            kind: Kind(rawValue: item.kind.rawValue) ?? .note,
            title: item.title,
            text: item.text,
            isStreaming: item.isStreaming,
            status: item.status,
            contextSummary: item.contextSummary,
            renderedMarkdown: item.renderedMarkdown,
            revisionToken: item.revision
        )
    }

    var isStreamingDraftAssistant: Bool {
        kind == .assistant && isStreaming
    }
}

extension EditorAgentTranscriptItem {
    init(entry: EditorAgentTimelineEntry) {
        self.init(
            id: entry.id,
            kind: Kind(rawValue: entry.kind.rawValue) ?? .note,
            title: entry.title,
            text: entry.text,
            isStreaming: entry.isStreaming,
            status: entry.status,
            contextSummary: entry.contextSummary,
            renderedMarkdown: entry.renderedMarkdown,
            revision: entry.revisionToken
        )
    }
}

@MainActor
final class EditorAgentTimelineController: NSObject {
    private struct LayoutEntry {
        let id: String
        let y: CGFloat
        let height: CGFloat
    }

    private struct HeightCacheKey: Hashable {
        let id: String
        let revisionToken: Int
        let width: Int
        let isExpanded: Bool
    }

    private enum Layout {
        static let transcriptSpacing: CGFloat = 10
        static let userBubbleMaxWidthFraction: CGFloat = 0.78
        static let userBubbleMaxWidth: CGFloat = 520
        static let minUserBubbleTextWidth: CGFloat = 160
        static let minAssistantWidth: CGFloat = 180
        static let toolBodyMaxHeight: CGFloat = 220
        static let visibleOverscan: CGFloat = 500
        static let alwaysLiveTailRows = 8
        static let fallbackContentWidth = 720
        static let minUsableWidth = 160
        static let bottomPinnedThreshold: CGFloat = 36
    }

    weak var scrollView: EditorAgentTimelineScrollView?
    weak var documentView: EditorAgentTimelineDocumentView?

    var onToggleToolExpansion: ((String) -> Void)?
    var onPinnedStateChange: ((Bool) -> Void)?
    var onJumpAffordanceChange: ((Bool) -> Void)?

    private var entries: [EditorAgentTimelineEntry] = []
    private var expandedToolItemIDs: Set<String> = []
    private var selectionColor: NSColor = .selectedContentBackgroundColor
    private var rowLayouts: [LayoutEntry] = []
    private var heightCache: [HeightCacheKey: CGFloat] = [:]
    private var visibleRowViews: [String: AgentTranscriptNativeRowView] = [:]
    private var reusePool: [AgentTranscriptNativeRowView] = []
    private var boundsObserver: NSObjectProtocol?
    private var lastStableContentWidth: Int?
    private var lastJumpToLatestToken = 0
    private var lastBoundsLogTime: CFAbsoluteTime = 0
    private var pendingScrollToBottomWorkItem: DispatchWorkItem?
    private var pendingAnimatedScrollToBottom = false
    private var updateSequence = 0


    func attach(scrollView: EditorAgentTimelineScrollView, documentView: EditorAgentTimelineDocumentView) {
        detach()
        self.scrollView = scrollView
        self.documentView = documentView
        scrollView.documentView = documentView
        scrollView.contentView.postsBoundsChangedNotifications = true
        boundsObserver = NotificationCenter.default.addObserver(
            forName: NSView.boundsDidChangeNotification,
            object: scrollView.contentView,
            queue: nil
        ) { [weak self] _ in
            Task { @MainActor in
                self?.boundsDidChange()
            }
        }
    }

    func detach() {
        if let boundsObserver {
            NotificationCenter.default.removeObserver(boundsObserver)
            self.boundsObserver = nil
        }
        pendingScrollToBottomWorkItem?.cancel()
        pendingScrollToBottomWorkItem = nil
        pendingAnimatedScrollToBottom = false
        for view in visibleRowViews.values {
            view.removeFromSuperview()
        }
        visibleRowViews.removeAll()
        reusePool.removeAll()
        scrollView = nil
        documentView = nil
    }

    func apply(
        entries newEntries: [EditorAgentTimelineEntry],
        selectionColor: NSColor,
        expandedToolItemIDs newExpandedToolItemIDs: Set<String>,
        topInset: CGFloat,
        jumpToLatestToken: Int,
        onToggleToolExpansion: @escaping (String) -> Void
    ) {
        guard let scrollView else { return }

        updateSequence += 1
        let updateID = updateSequence
        let started = CFAbsoluteTimeGetCurrent()
        let previousEntries = entries
        let previousExpandedToolItemIDs = expandedToolItemIDs
        let wasPinnedToBottom = isPinnedToBottom(in: scrollView)

        scrollView.contentInsets = NSEdgeInsets(top: topInset, left: 0, bottom: 6, right: 0)
        self.onToggleToolExpansion = onToggleToolExpansion

        let width = contentWidth(in: scrollView)
        let widthChanged = width != lastStableContentWidth
        if widthChanged {
            lastStableContentWidth = width
        }

        let selectionColorChanged = !self.selectionColor.isEqual(selectionColor)
        self.selectionColor = selectionColor
        let expansionDiff = previousExpandedToolItemIDs.symmetricDifference(newExpandedToolItemIDs)
        expandedToolItemIDs = newExpandedToolItemIDs

        let jumpToLatestChanged = jumpToLatestToken != lastJumpToLatestToken
        lastJumpToLatestToken = jumpToLatestToken

        let layoutStarted = CFAbsoluteTimeGetCurrent()
        let layoutResult = syncEntries(
            from: previousEntries,
            to: newEntries,
            width: width,
            widthChanged: widthChanged,
            expandedDiff: expansionDiff
        )
        let layoutMs = (CFAbsoluteTimeGetCurrent() - layoutStarted) * 1000

        updateDocumentFrame(width: width)
        updateVisibleRows(width: width, forceReconfigure: selectionColorChanged || widthChanged || layoutResult == .fullReload)

        if jumpToLatestChanged {
            onJumpAffordanceChange?(false)
            requestScrollToBottom(animated: true, in: scrollView)
        } else if newEntries.isEmpty {
            onJumpAffordanceChange?(false)
            onPinnedStateChange?(true)
        } else if layoutResult != .none || !expansionDiff.isEmpty {
            if wasPinnedToBottom {
                onJumpAffordanceChange?(false)
                requestScrollToBottom(animated: false, in: scrollView)
            } else if previousEntries.last?.id != newEntries.last?.id || previousEntries.last?.revisionToken != newEntries.last?.revisionToken {
                onJumpAffordanceChange?(true)
            }
        } else if widthChanged, wasPinnedToBottom {
            requestScrollToBottom(animated: false, in: scrollView)
        }

        updatePinnedState(in: scrollView)

        let totalMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
        let totalMsText = String(format: "%.2f", totalMs)
        let layoutMsText = String(format: "%.2f", layoutMs)
        agentPerfLog(
            "transcript.timeline.update #\(updateID) entries=\(entries.count) width=\(width) mode=\(layoutResult.rawValue) widthChanged=\(widthChanged) selectionChanged=\(selectionColorChanged) totalMs=\(totalMsText) layoutMs=\(layoutMsText)"
        )
    }

    func setEntries(_ entries: [EditorAgentTimelineEntry]) {
        self.entries = entries
    }

    func appendEntries(_ appendedEntries: [EditorAgentTimelineEntry]) {
        guard !appendedEntries.isEmpty else { return }
        entries.append(contentsOf: appendedEntries)
    }

    func replaceEntry(id: String, with entry: EditorAgentTimelineEntry) {
        guard let index = entries.firstIndex(where: { $0.id == id }) else { return }
        entries[index] = entry
    }

    private enum LayoutResult: String {
        case none
        case incremental
        case fullReload
    }

    private func syncEntries(
        from oldEntries: [EditorAgentTimelineEntry],
        to newEntries: [EditorAgentTimelineEntry],
        width: Int,
        widthChanged: Bool,
        expandedDiff: Set<String>
    ) -> LayoutResult {
        if widthChanged {
            entries = newEntries
            rebuildLayouts(from: 0, width: width)
            pruneCaches()
            return .fullReload
        }

        guard canApplyIncrementally(from: oldEntries, to: newEntries) else {
            entries = newEntries
            rebuildLayouts(from: 0, width: width)
            pruneCaches(removingNonVisibleRows: false)
            return oldEntries == newEntries && expandedDiff.isEmpty ? .none : .fullReload
        }

        entries = newEntries
        let changedIndex = firstChangedIndex(from: oldEntries, to: newEntries, expandedDiff: expandedDiff)
        guard let changedIndex else {
            pruneCaches()
            return .none
        }

        if changedIndex == oldEntries.count, newEntries.count > oldEntries.count {
            entries = newEntries
        }
        rebuildLayouts(from: changedIndex, width: width)
        pruneCaches(removingNonVisibleRows: false)
        return .incremental
    }

    private func canApplyIncrementally(from oldEntries: [EditorAgentTimelineEntry], to newEntries: [EditorAgentTimelineEntry]) -> Bool {
        guard !oldEntries.isEmpty else { return false }
        guard newEntries.count >= oldEntries.count else { return false }
        for index in 0..<oldEntries.count {
            guard oldEntries[index].id == newEntries[index].id else { return false }
        }
        return true
    }

    private func firstChangedIndex(
        from oldEntries: [EditorAgentTimelineEntry],
        to newEntries: [EditorAgentTimelineEntry],
        expandedDiff: Set<String>
    ) -> Int? {
        if oldEntries.isEmpty {
            return newEntries.isEmpty ? nil : 0
        }

        var firstChanged: Int?
        let prefixCount = min(oldEntries.count, newEntries.count)
        for index in 0..<prefixCount where oldEntries[index].revisionToken != newEntries[index].revisionToken {
            firstChanged = index
            break
        }

        if newEntries.count > oldEntries.count {
            firstChanged = min(firstChanged ?? Int.max, oldEntries.count)
        }

        if !expandedDiff.isEmpty {
            let firstExpandedChange = newEntries.firstIndex { expandedDiff.contains($0.id) }
            if let firstExpandedChange {
                firstChanged = min(firstChanged ?? Int.max, firstExpandedChange)
            }
        }

        return firstChanged
    }

    private func rebuildLayouts(from startIndex: Int, width: Int) {
        let clampedStart = min(max(startIndex, 0), entries.count)
        if clampedStart == 0 {
            rowLayouts.removeAll(keepingCapacity: true)
        } else if rowLayouts.count > clampedStart {
            rowLayouts.removeSubrange(clampedStart..<rowLayouts.count)
        }

        var y = rowLayouts.last.map { $0.y + $0.height + Layout.transcriptSpacing } ?? 0
        for index in clampedStart..<entries.count {
            let entry = entries[index]
            let isExpanded = expandedToolItemIDs.contains(entry.id)
            let height = exactHeight(for: entry, width: width, isExpanded: isExpanded)
            rowLayouts.append(LayoutEntry(id: entry.id, y: y, height: height))
            y += height + Layout.transcriptSpacing
        }
    }

    private func exactHeight(for entry: EditorAgentTimelineEntry, width: Int, isExpanded: Bool) -> CGFloat {
        let cacheKey = HeightCacheKey(id: entry.id, revisionToken: entry.revisionToken, width: width, isExpanded: isExpanded)
        if let cached = heightCache[cacheKey] {
            return cached
        }

        let started = CFAbsoluteTimeGetCurrent()
        let measuredHeight = measureHeight(for: entry, width: CGFloat(width), isExpanded: isExpanded)
        let elapsedMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
        if elapsedMs >= 8 {
            let elapsedMsText = String(format: "%.2f", elapsedMs)
            agentPerfLog(
                "transcript.timeline.height id=\(entry.id) kind=\(entry.kind.rawValue) chars=\(entry.text.count) width=\(width) ms=\(elapsedMsText)"
            )
        }
        heightCache[cacheKey] = measuredHeight
        return measuredHeight
    }

    private func measureHeight(for entry: EditorAgentTimelineEntry, width: CGFloat, isExpanded: Bool) -> CGFloat {
        switch entry.kind {
        case .user:
            return measureUserHeight(for: entry, width: width)
        case .assistant:
            return measureAssistantHeight(for: entry, width: width)
        case .note:
            return measureNoteHeight(entry.text, width: width)
        case .tool:
            return measureToolHeight(for: entry, width: width, isExpanded: isExpanded)
        }
    }

    private func measureUserHeight(for entry: EditorAgentTimelineEntry, width: CGFloat) -> CGFloat {
        let bubbleTextWidth = max(min(width * Layout.userBubbleMaxWidthFraction, Layout.userBubbleMaxWidth) - 28, Layout.minUserBubbleTextWidth)
        let textHeight = measurePlainTextHeight(
            entry.text,
            width: bubbleTextWidth,
            font: .systemFont(ofSize: 13),
            lineSpacing: 1
        )
        let contextHeight: CGFloat = (entry.contextSummary?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false) ? 28 : 0
        return contextHeight + textHeight + 20
    }

    private func measureNoteHeight(_ text: String, width: CGFloat) -> CGFloat {
        let textHeight = measurePlainTextHeight(
            text,
            width: max(width - 24, Layout.minUserBubbleTextWidth),
            font: .systemFont(ofSize: 11, weight: .medium),
            lineSpacing: 1
        )
        return max(textHeight + 4, 18)
    }

    private func measureToolHeight(for entry: EditorAgentTimelineEntry, width: CGFloat, isExpanded: Bool) -> CGFloat {
        agentToolRowMeasuredHeight(
            item: EditorAgentTranscriptItem(entry: entry),
            width: width,
            isExpanded: isExpanded
        )
    }

    private func measureAssistantHeight(for entry: EditorAgentTimelineEntry, width: CGFloat) -> CGFloat {
        let contentWidth = max(width - 10, Layout.minAssistantWidth)
        guard let rendered = entry.renderedMarkdown, !rendered.blocks.isEmpty else {
            let textHeight = measurePlainTextHeight(
                entry.text,
                width: contentWidth,
                font: .systemFont(ofSize: 13),
                lineSpacing: 1
            )
            return max(textHeight + (entry.isStreaming && entry.text.isEmpty ? 22 : 0), 24)
        }

        var totalHeight: CGFloat = 0
        for segment in renderedSegments(for: rendered) {
            switch segment {
            case .text(let runs):
                totalHeight += ceil(editorDocsAttributedBounds(for: runs, width: contentWidth).height)
            case .code(_, let runs):
                let codeHeight = ceil(editorDocsAttributedBounds(for: runs, width: max(width - 20, Layout.minUserBubbleTextWidth)).height)
                totalHeight += codeHeight + 62
            }
        }
        if entry.isStreaming && entry.text.isEmpty {
            totalHeight += 18
        }
        return max(totalHeight, 24)
    }

    private func measurePlainTextHeight(_ text: String, width: CGFloat, font: NSFont, lineSpacing: CGFloat) -> CGFloat {
        let paragraphStyle = NSMutableParagraphStyle()
        paragraphStyle.lineBreakMode = .byWordWrapping
        paragraphStyle.lineSpacing = lineSpacing
        let attributed = NSAttributedString(
            string: text,
            attributes: [
                .font: font,
                .paragraphStyle: paragraphStyle,
            ]
        )
        return ceil(
            attributed.boundingRect(
                with: CGSize(width: max(width, 1), height: .greatestFiniteMagnitude),
                options: [.usesLineFragmentOrigin, .usesFontLeading]
            ).height
        )
    }

    private func updateDocumentFrame(width: Int) {
        guard let documentView else { return }
        let docHeight = max(rowLayouts.last.map { $0.y + $0.height } ?? 0, 1)
        documentView.frame = CGRect(x: 0, y: 0, width: CGFloat(width), height: docHeight)
    }

    private func updateVisibleRows(width: Int, forceReconfigure: Bool = false) {
        guard let scrollView, let documentView else { return }
        let targetIndices = targetIndices(in: scrollView)
        let targetIDs = Set(targetIndices.map { entries[$0].id })

        for id in Array(visibleRowViews.keys) where !targetIDs.contains(id) {
            guard let rowView = visibleRowViews.removeValue(forKey: id) else { continue }
            rowView.removeFromSuperview()
            reusePool.append(rowView)
        }

        for index in targetIndices {
            guard entries.indices.contains(index), rowLayouts.indices.contains(index) else { continue }
            let entry = entries[index]
            let layout = rowLayouts[index]
            let rowView = visibleRowViews[entry.id] ?? dequeueRowView(attachingTo: documentView, id: entry.id)
            let frame = CGRect(x: 0, y: layout.y, width: CGFloat(width), height: layout.height)
            if rowView.frame != frame {
                rowView.frame = frame
            }
            if rowView.superview !== documentView {
                documentView.addSubview(rowView)
            }
            rowView.configure(
                item: EditorAgentTranscriptItem(entry: entry),
                selectionColor: selectionColor,
                width: CGFloat(width),
                isToolExpanded: expandedToolItemIDs.contains(entry.id),
                onToggleToolExpansion: { [weak self] in
                    self?.onToggleToolExpansion?(entry.id)
                }
            )
        }
    }

    private func dequeueRowView(attachingTo documentView: EditorAgentTimelineDocumentView, id: String) -> AgentTranscriptNativeRowView {
        let rowView = reusePool.popLast() ?? AgentTranscriptNativeRowView()
        visibleRowViews[id] = rowView
        documentView.addSubview(rowView)
        return rowView
    }

    private func targetIndices(in scrollView: NSScrollView) -> [Int] {
        guard !entries.isEmpty, !rowLayouts.isEmpty else { return [] }

        let tailStartIndex = max(entries.count - Layout.alwaysLiveTailRows, 0)
        let visibleRect = scrollView.contentView.bounds.insetBy(dx: 0, dy: -Layout.visibleOverscan)
        var indices: [Int] = []

        if tailStartIndex > 0,
           let range = intersectingRange(for: visibleRect, limit: tailStartIndex) {
            indices.append(contentsOf: range)
        }

        if tailStartIndex < entries.count {
            indices.append(contentsOf: tailStartIndex..<entries.count)
        }

        return indices
    }

    private func intersectingRange(for rect: CGRect, limit: Int) -> Range<Int>? {
        guard limit > 0 else { return nil }
        let start = max(firstIntersectingIndex(minY: rect.minY, limit: limit) - 1, 0)
        var end = start
        while end < limit {
            let layout = rowLayouts[end]
            if layout.y > rect.maxY {
                break
            }
            end += 1
        }
        return start < end ? start..<end : nil
    }

    private func firstIntersectingIndex(minY: CGFloat, limit: Int) -> Int {
        guard limit > 0 else { return 0 }
        var low = 0
        var high = limit
        while low < high {
            let mid = (low + high) / 2
            let layout = rowLayouts[mid]
            if layout.y + layout.height < minY {
                low = mid + 1
            } else {
                high = mid
            }
        }
        return min(low, max(limit - 1, 0))
    }

    private func contentWidth(in scrollView: NSScrollView) -> Int {
        let candidate = max(Int(scrollView.contentSize.width.rounded(.toNearestOrAwayFromZero)), 1)
        let visibleHeight = scrollView.contentView.bounds.height
        if candidate >= Layout.minUsableWidth, visibleHeight > 1 {
            return candidate
        }
        if let lastStableContentWidth {
            return lastStableContentWidth
        }
        return Layout.fallbackContentWidth
    }

    private func pruneCaches(removingNonVisibleRows: Bool = true) {
        let liveEntryIDs = Set(entries.map(\.id))
        heightCache = heightCache.filter { liveEntryIDs.contains($0.key.id) }

        guard removingNonVisibleRows else { return }
        for id in Array(visibleRowViews.keys) where !liveEntryIDs.contains(id) {
            guard let rowView = visibleRowViews.removeValue(forKey: id) else { continue }
            rowView.removeFromSuperview()
            reusePool.append(rowView)
        }
    }

    private func boundsDidChange() {
        guard let scrollView else { return }
        let width = contentWidth(in: scrollView)
        let widthChanged = width != lastStableContentWidth
        let wasPinned = isPinnedToBottom(in: scrollView)

        if widthChanged {
            lastStableContentWidth = width
            rebuildLayouts(from: 0, width: width)
            updateDocumentFrame(width: width)
            updateVisibleRows(width: width, forceReconfigure: true)
            if wasPinned {
                requestScrollToBottom(animated: false, in: scrollView)
            }
        } else {
            updateVisibleRows(width: width)
        }

        let now = CFAbsoluteTimeGetCurrent()
        if now - lastBoundsLogTime >= 0.12 {
            lastBoundsLogTime = now
            agentPerfLog(
                "transcript.timeline.boundsDidChange offsetY=\(Int(scrollView.contentView.bounds.origin.y.rounded())) visibleHeight=\(Int(scrollView.contentView.bounds.height.rounded())) docHeight=\(Int((scrollView.documentView?.bounds.height ?? 0).rounded())) distanceToBottom=\(Int(distanceToBottom(in: scrollView).rounded()))"
            )
        }
        updatePinnedState(in: scrollView)
    }

    private func updatePinnedState(in scrollView: NSScrollView) {
        let pinned = isPinnedToBottom(in: scrollView)
        onPinnedStateChange?(pinned)
        if pinned {
            onJumpAffordanceChange?(false)
        }
    }

    private func isPinnedToBottom(in scrollView: NSScrollView) -> Bool {
        distanceToBottom(in: scrollView) <= Layout.bottomPinnedThreshold
    }

    private func distanceToBottom(in scrollView: NSScrollView) -> CGFloat {
        max(maxNormalizedOffsetY(in: scrollView) - normalizedOffsetY(in: scrollView), 0)
    }

    private func normalizedOffsetY(in scrollView: NSScrollView) -> CGFloat {
        scrollView.contentView.bounds.origin.y + scrollView.contentInsets.top
    }

    private func maxNormalizedOffsetY(in scrollView: NSScrollView) -> CGFloat {
        guard let documentView = scrollView.documentView else { return 0 }
        let totalHeight = documentView.bounds.height + scrollView.contentInsets.top + scrollView.contentInsets.bottom
        return max(totalHeight - scrollView.contentView.bounds.height, 0)
    }

    private func requestScrollToBottom(animated: Bool, in scrollView: NSScrollView) {
        pendingAnimatedScrollToBottom = pendingAnimatedScrollToBottom || animated
        pendingScrollToBottomWorkItem?.cancel()
        let workItem = DispatchWorkItem { [weak self, weak scrollView] in
            guard let self, let scrollView else { return }
            let shouldAnimate = self.pendingAnimatedScrollToBottom
            self.pendingAnimatedScrollToBottom = false
            self.performScrollToBottom(animated: shouldAnimate, in: scrollView)
        }
        pendingScrollToBottomWorkItem = workItem
        DispatchQueue.main.async(execute: workItem)
    }

    private func performScrollToBottom(animated: Bool, in scrollView: NSScrollView) {
        let targetY = max(maxNormalizedOffsetY(in: scrollView) - scrollView.contentInsets.top, -scrollView.contentInsets.top)
        let currentY = scrollView.contentView.bounds.origin.y
        guard abs(currentY - targetY) > 0.5 else {
            updatePinnedState(in: scrollView)
            return
        }

        if animated {
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.16
                scrollView.contentView.animator().setBoundsOrigin(NSPoint(x: 0, y: targetY))
            } completionHandler: { [weak self, weak scrollView] in
                guard let self, let scrollView else { return }
                Task { @MainActor in
                    self.updatePinnedState(in: scrollView)
                }
            }
        } else {
            scrollView.contentView.setBoundsOrigin(NSPoint(x: 0, y: targetY))
            scrollView.reflectScrolledClipView(scrollView.contentView)
            updatePinnedState(in: scrollView)
        }
    }
}

struct EditorAgentTimelineView: NSViewRepresentable {
    let controller: EditorAgentTimelineController
    let entries: [EditorAgentTimelineEntry]
    let selectionColor: NSColor
    let expandedToolItemIDs: Binding<Set<String>>
    let jumpToLatestToken: Int
    let isPinnedToBottom: Binding<Bool>
    let showsJumpToLatest: Binding<Bool>
    let topInset: CGFloat

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeNSView(context: Context) -> EditorAgentTimelineScrollView {
        context.coordinator.makeScrollView()
    }

    func updateNSView(_ nsView: EditorAgentTimelineScrollView, context: Context) {
        context.coordinator.update(parent: self)
    }

    static func dismantleNSView(_ nsView: EditorAgentTimelineScrollView, coordinator: Coordinator) {
        coordinator.controller.detach()
    }

    @MainActor
    final class Coordinator {
        var parent: EditorAgentTimelineView
        let controller: EditorAgentTimelineController

        init(parent: EditorAgentTimelineView) {
            self.parent = parent
            controller = parent.controller
        }

        func makeScrollView() -> EditorAgentTimelineScrollView {
            let scrollView = EditorAgentTimelineScrollView()
            let documentView = EditorAgentTimelineDocumentView()
            controller.attach(scrollView: scrollView, documentView: documentView)
            bindControllerOutputs()
            update(parent: parent)
            return scrollView
        }

        func update(parent: EditorAgentTimelineView) {
            self.parent = parent
            bindControllerOutputs()
            controller.apply(
                entries: parent.entries,
                selectionColor: parent.selectionColor,
                expandedToolItemIDs: parent.expandedToolItemIDs.wrappedValue,
                topInset: parent.topInset,
                jumpToLatestToken: parent.jumpToLatestToken,
                onToggleToolExpansion: { [weak self] itemID in
                    self?.toggleToolExpansion(for: itemID)
                }
            )
        }

        private func bindControllerOutputs() {
            controller.onPinnedStateChange = { [weak self] value in
                self?.setPinnedToBottom(value)
            }
            controller.onJumpAffordanceChange = { [weak self] value in
                self?.setShowsJumpToLatest(value)
            }
        }

        private func toggleToolExpansion(for itemID: String) {
            var nextExpandedToolItemIDs = parent.expandedToolItemIDs.wrappedValue
            if nextExpandedToolItemIDs.contains(itemID) {
                nextExpandedToolItemIDs.remove(itemID)
            } else {
                nextExpandedToolItemIDs.insert(itemID)
            }
            parent.expandedToolItemIDs.wrappedValue = nextExpandedToolItemIDs
        }

        private func setPinnedToBottom(_ value: Bool) {
            guard parent.isPinnedToBottom.wrappedValue != value else { return }
            DispatchQueue.main.async { [binding = parent.isPinnedToBottom] in
                binding.wrappedValue = value
            }
        }

        private func setShowsJumpToLatest(_ value: Bool) {
            guard parent.showsJumpToLatest.wrappedValue != value else { return }
            DispatchQueue.main.async { [binding = parent.showsJumpToLatest] in
                binding.wrappedValue = value
            }
        }
    }
}

@MainActor
final class EditorAgentTimelineScrollView: NSScrollView {
    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        drawsBackground = false
        borderType = .noBorder
        hasVerticalScroller = true
        hasHorizontalScroller = false
        autohidesScrollers = true
        scrollerStyle = .overlay
        usesPredominantAxisScrolling = true
        verticalScrollElasticity = .automatic
        horizontalScrollElasticity = .none
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}

@MainActor
final class EditorAgentTimelineDocumentView: NSView {
    override var isFlipped: Bool { true }
}

private enum EditorAgentTimelineMarkdownSegment {
    case text([EditorDocsRun])
    case code(language: String?, runs: [EditorDocsRun])
}

private extension EditorAgentTimelineController {
    func renderedSegments(for rendered: EditorRenderedMarkdown) -> [EditorAgentTimelineMarkdownSegment] {
        var result: [EditorAgentTimelineMarkdownSegment] = []
        var pendingTextBlocks: [EditorMarkdownBlock] = []

        func flushTextBlocks() {
            let runs = mergedRuns(for: pendingTextBlocks, rendered: rendered)
            if !runs.isEmpty {
                result.append(.text(runs))
            }
            pendingTextBlocks.removeAll()
        }

        for block in rendered.blocks {
            if block.kind == .codeFence {
                flushTextBlocks()
                result.append(.code(language: block.language, runs: runs(for: block, rendered: rendered)))
            } else {
                pendingTextBlocks.append(block)
            }
        }

        flushTextBlocks()
        return result
    }

    func mergedRuns(for blocks: [EditorMarkdownBlock], rendered: EditorRenderedMarkdown) -> [EditorDocsRun] {
        guard !blocks.isEmpty else { return [] }
        var merged: [EditorDocsRun] = []

        for (index, block) in blocks.enumerated() {
            if block.kind != .blankLine {
                merged.append(contentsOf: runs(for: block, rendered: rendered))
            }
            guard index < blocks.count - 1 else { continue }
            let nextBlock = blocks[index + 1]
            let referenceRun = merged.last ?? runs(for: nextBlock, rendered: rendered).first
            merged.append(
                EditorDocsRun(
                    text: "\n",
                    style: referenceRun?.style ?? EditorResolvedStyle(
                        fg: nil,
                        bg: nil,
                        underlineColor: nil,
                        addModifiers: 0,
                        removeModifiers: 0,
                        underlineStyle: 0
                    ),
                    kind: .body,
                    linkDestination: nil
                )
            )
        }

        return merged
    }

    func runs(for block: EditorMarkdownBlock, rendered: EditorRenderedMarkdown) -> [EditorDocsRun] {
        guard block.runCount > 0,
              block.runStart >= 0,
              block.runStart + block.runCount <= rendered.runs.count else {
            return []
        }
        return Array(rendered.runs[block.runStart..<(block.runStart + block.runCount)])
    }
}
