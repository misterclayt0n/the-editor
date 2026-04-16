import AppKit
import Foundation

private enum AgentTranscriptCanvasLayout {
    static let userBubbleMaxWidthFraction: CGFloat = 0.78
    static let userBubbleCornerRadius: CGFloat = 16
    static let toolRowCornerRadius: CGFloat = 10
}

struct AgentTranscriptCanvasLayoutEntry {
    let id: String
    let y: CGFloat
    let height: CGFloat
}

private enum AgentTranscriptCanvasInteraction {
    case toggleTool(String)
    case copy(String)
    case openLink(URL)
    case openStringLink(String)
}

private struct AgentTranscriptCanvasColorKey: Hashable {
    let r: Int
    let g: Int
    let b: Int
    let a: Int

    init(_ color: NSColor) {
        let resolved = color.usingColorSpace(.sRGB) ?? color
        r = Int((resolved.redComponent * 255).rounded())
        g = Int((resolved.greenComponent * 255).rounded())
        b = Int((resolved.blueComponent * 255).rounded())
        a = Int((resolved.alphaComponent * 255).rounded())
    }
}

private struct AgentTranscriptCanvasRasterKey: Hashable {
    let item: EditorAgentTranscriptItem
    let width: Int
    let isExpanded: Bool
    let selectionColor: AgentTranscriptCanvasColorKey
}

@MainActor
final class AgentTranscriptCanvasDocumentView: NSView {

    private var items: [EditorAgentTranscriptItem] = []
    private var rowLayouts: [AgentTranscriptCanvasLayoutEntry] = []
    private var selectionColor: NSColor = .selectedContentBackgroundColor
    private var expandedToolItemIDs: Set<String> = []
    private var contentWidth: CGFloat = 720
    private var onToggleToolExpansion: ((String) -> Void)?
    private var rasterCache: [AgentTranscriptCanvasRasterKey: CGImage] = [:]
    private var lastRasterWidth: Int?
    private var lastSelectionColorKey: AgentTranscriptCanvasColorKey?

    override var isFlipped: Bool { true }
    override var isOpaque: Bool { false }

    func update(
        items: [EditorAgentTranscriptItem],
        rowLayouts: [AgentTranscriptCanvasLayoutEntry],
        selectionColor: NSColor,
        expandedToolItemIDs: Set<String>,
        contentWidth: CGFloat,
        onToggleToolExpansion: @escaping (String) -> Void
    ) {
        let widthKey = Int(contentWidth.rounded(.toNearestOrAwayFromZero))
        let selectionKey = AgentTranscriptCanvasColorKey(selectionColor)
        if lastRasterWidth != widthKey || lastSelectionColorKey != selectionKey {
            rasterCache.removeAll()
            lastRasterWidth = widthKey
            lastSelectionColorKey = selectionKey
        }

        self.items = items
        self.rowLayouts = rowLayouts
        self.selectionColor = selectionColor
        self.expandedToolItemIDs = expandedToolItemIDs
        self.contentWidth = contentWidth
        self.onToggleToolExpansion = onToggleToolExpansion
        pruneRasterCache()
        needsDisplay = true
    }

    func warmCache(visibleRect: CGRect, overscan: CGFloat) {
        guard !items.isEmpty, !rowLayouts.isEmpty else { return }
        let started = CFAbsoluteTimeGetCurrent()
        let targetRect = visibleRect.insetBy(dx: 0, dy: -overscan)
        let startIndex = max(firstIntersectingIndex(minY: targetRect.minY) - 1, 0)
        let endIndex = min(firstIntersectingIndex(minY: targetRect.maxY + 1) + 2, rowLayouts.count)
        guard startIndex < endIndex else { return }

        var misses = 0
        for index in startIndex..<endIndex {
            guard items.indices.contains(index) else { continue }
            let layout = rowLayouts[index]
            let frame = CGRect(x: 0, y: 0, width: contentWidth, height: layout.height)
            let key = rasterKey(for: items[index])
            if rasterCache[key] == nil, let image = renderRowImage(for: items[index], in: frame) {
                rasterCache[key] = image
                misses += 1
            }
        }
        let elapsedMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
        if misses > 0 && elapsedMs >= 4 {
            agentPerfLog("transcript.canvas.warmCache misses=\(misses) totalMs=\(String(format: "%.2f", elapsedMs))")
        }
    }

    override func draw(_ dirtyRect: NSRect) {
        let started = CFAbsoluteTimeGetCurrent()
        super.draw(dirtyRect)
        guard !items.isEmpty, !rowLayouts.isEmpty else { return }

        var misses = 0
        let startIndex = max(firstIntersectingIndex(minY: dirtyRect.minY) - 1, 0)
        let endIndex = min(firstIntersectingIndex(minY: dirtyRect.maxY + 1) + 2, rowLayouts.count)
        guard startIndex < endIndex else { return }

        for index in startIndex..<endIndex {
            guard items.indices.contains(index) else { continue }
            let layout = rowLayouts[index]
            let frame = CGRect(x: 0, y: layout.y, width: contentWidth, height: layout.height)
            guard frame.intersects(dirtyRect) else { continue }
            let key = rasterKey(for: items[index])
            let image: CGImage?
            if let cached = rasterCache[key] {
                image = cached
            } else {
                image = renderRowImage(for: items[index], in: CGRect(x: 0, y: 0, width: frame.width, height: frame.height))
                if let image {
                    rasterCache[key] = image
                }
                misses += 1
            }
            if let image {
                NSGraphicsContext.current?.cgContext.draw(image, in: frame)
            }
        }

        let elapsedMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
        if elapsedMs >= 4 || misses > 0 {
            agentPerfLog("transcript.canvas.draw dirtyH=\(Int(dirtyRect.height.rounded())) misses=\(misses) totalMs=\(String(format: "%.2f", elapsedMs))")
        }
    }

    override func mouseDown(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        guard let interaction = interaction(at: point) else {
            super.mouseDown(with: event)
            return
        }

        switch interaction {
        case .toggleTool(let id):
            onToggleToolExpansion?(id)
        case .copy(let text):
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
        case .openLink(let url):
            NSWorkspace.shared.open(url)
        case .openStringLink(let raw):
            if let url = URL(string: raw) {
                NSWorkspace.shared.open(url)
            }
        }
    }

    private func drawItem(_ item: EditorAgentTranscriptItem, in frame: CGRect) {
        switch item.kind {
        case .user:
            drawUserItem(item, in: frame)
        case .assistant:
            drawAssistantItem(item, in: frame)
        case .thinking:
            drawNoteItem(item, in: frame)
        case .note:
            drawNoteItem(item, in: frame)
        case .tool:
            drawToolItem(item, in: frame)
        }
    }

    private func drawUserItem(_ item: EditorAgentTranscriptItem, in frame: CGRect) {
        let bubbleTextWidth = max(min(frame.width * AgentTranscriptCanvasLayout.userBubbleMaxWidthFraction, 520) - 28, 160)
        let textHeight = agentCanvasMeasurePlainTextHeight(
            item.text,
            width: bubbleTextWidth,
            font: .systemFont(ofSize: 13),
            lineSpacing: 1
        )
        let bubbleWidth = bubbleTextWidth + 28
        let bubbleHeight = textHeight + 16
        let showsContext = !(item.contextSummary?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ?? true)

        var bubbleOriginY = frame.minY
        if showsContext, let context = item.contextSummary?.trimmingCharacters(in: .whitespacesAndNewlines) {
            let iconWidth: CGFloat = 10
            let contextFont = NSFont.systemFont(ofSize: 10, weight: .medium)
            let labelWidth = ceil((context as NSString).size(withAttributes: [.font: contextFont]).width)
            let pillWidth = min(max(labelWidth + iconWidth + 18, 44), max(bubbleWidth, 44))
            let pillRect = CGRect(x: frame.maxX - pillWidth, y: frame.minY, width: pillWidth, height: 24)
            let path = NSBezierPath(roundedRect: pillRect, xRadius: 12, yRadius: 12)
            NSColor.labelColor.withAlphaComponent(0.05).setFill()
            path.fill()
            NSColor.labelColor.withAlphaComponent(0.08).setStroke()
            path.lineWidth = 0.5
            path.stroke()
            agentCanvasDrawSymbol("doc.text", pointSize: 9, weight: .semibold, color: .secondaryLabelColor, in: CGRect(x: pillRect.minX + 8, y: pillRect.minY + 7, width: 10, height: 10))
            agentCanvasPlainAttributedString(
                context,
                font: contextFont,
                color: .secondaryLabelColor,
                alignment: .left,
                lineSpacing: 1
            )
            .draw(in: CGRect(x: pillRect.minX + 20, y: pillRect.minY + 4, width: pillRect.width - 28, height: 16))
            bubbleOriginY += 28
        }

        let bubbleRect = CGRect(x: frame.maxX - bubbleWidth, y: bubbleOriginY, width: bubbleWidth, height: bubbleHeight)
        let bubblePath = NSBezierPath(roundedRect: bubbleRect, xRadius: AgentTranscriptCanvasLayout.userBubbleCornerRadius, yRadius: AgentTranscriptCanvasLayout.userBubbleCornerRadius)
        selectionColor.withAlphaComponent(0.22).setFill()
        bubblePath.fill()
        NSColor.labelColor.withAlphaComponent(0.08).setStroke()
        bubblePath.lineWidth = 0.5
        bubblePath.stroke()

        agentCanvasPlainAttributedString(
            item.text,
            font: .systemFont(ofSize: 13),
            color: .labelColor,
            alignment: .left,
            lineSpacing: 1
        )
        .draw(with: bubbleRect.insetBy(dx: 14, dy: 8), options: [.usesLineFragmentOrigin, .usesFontLeading])
    }

    private func drawAssistantItem(_ item: EditorAgentTranscriptItem, in frame: CGRect) {
        let contentWidth = max(frame.width - 10, 180)
        var y = frame.minY

        if let rendered = item.renderedMarkdown, !rendered.blocks.isEmpty {
            for segment in agentCanvasRenderedMarkdownSegments(for: rendered) {
                switch segment {
                case .text(let runs):
                    let attributed = editorDocsAttributedText(for: runs)
                    let height = ceil(editorDocsAttributedBounds(for: runs, width: contentWidth).height)
                    attributed.draw(
                        with: CGRect(x: frame.minX, y: y, width: contentWidth, height: height),
                        options: [.usesLineFragmentOrigin, .usesFontLeading]
                    )
                    y += height
                case .code(let language, let runs):
                    let metrics = agentCanvasCodeBlockMetrics(language: language, runs: runs, width: contentWidth)
                    drawCodeBlock(metrics, atY: y, inWidth: contentWidth)
                    y += metrics.outerRect.height
                }
            }
        } else {
            let attributed = agentCanvasPlainAttributedString(
                item.text,
                font: .systemFont(ofSize: 13),
                color: .labelColor,
                alignment: .left,
                lineSpacing: 1
            )
            let height = ceil(
                attributed.boundingRect(
                    with: CGSize(width: contentWidth, height: .greatestFiniteMagnitude),
                    options: [.usesLineFragmentOrigin, .usesFontLeading]
                ).height
            )
            attributed.draw(
                with: CGRect(x: frame.minX, y: y, width: contentWidth, height: height),
                options: [.usesLineFragmentOrigin, .usesFontLeading]
            )
            y += height
        }

        if item.isStreaming && item.text.isEmpty {
            drawThinkingDots(in: CGRect(x: frame.minX, y: y + 2, width: 22, height: 12))
        }
    }

    private func drawNoteItem(_ item: EditorAgentTranscriptItem, in frame: CGRect) {
        let attributed = agentCanvasPlainAttributedString(
            item.text,
            font: .systemFont(ofSize: 11, weight: .medium),
            color: .secondaryLabelColor,
            alignment: .center,
            lineSpacing: 1
        )
        attributed.draw(
            with: frame.insetBy(dx: 12, dy: 2),
            options: [.usesLineFragmentOrigin, .usesFontLeading]
        )
    }

    private func drawToolItem(_ item: EditorAgentTranscriptItem, in frame: CGRect) {
        let isExpanded = expandedToolItemIDs.contains(item.id)
        let outerPath = NSBezierPath(roundedRect: frame, xRadius: AgentTranscriptCanvasLayout.toolRowCornerRadius, yRadius: AgentTranscriptCanvasLayout.toolRowCornerRadius)
        NSColor.labelColor.withAlphaComponent(0.04).setFill()
        outerPath.fill()
        NSColor.labelColor.withAlphaComponent(0.06).setStroke()
        outerPath.lineWidth = 0.5
        outerPath.stroke()

        let headerRect = CGRect(x: frame.minX, y: frame.minY, width: frame.width, height: 36)
        agentCanvasDrawSymbol("wrench.and.screwdriver", pointSize: 10, weight: .semibold, color: .secondaryLabelColor, in: CGRect(x: headerRect.minX + 10, y: headerRect.minY + 12, width: 12, height: 12))

        let status = agentCanvasToolStatusSymbol(for: item.status)
        agentCanvasDrawSymbol(status.name, pointSize: 10, weight: .semibold, color: status.color, in: CGRect(x: headerRect.maxX - 20, y: headerRect.minY + 12, width: 12, height: 12))

        var trailingX = headerRect.maxX - 28
        if !item.text.isEmpty {
            agentCanvasDrawSymbol(isExpanded ? "chevron.up" : "chevron.down", pointSize: 9, weight: .semibold, color: .secondaryLabelColor, in: CGRect(x: trailingX - 10, y: headerRect.minY + 13, width: 10, height: 10))
            trailingX -= 18
        }
        if item.isStreaming {
            agentCanvasDrawSymbol("circle.dotted", pointSize: 9, weight: .semibold, color: .secondaryLabelColor, in: CGRect(x: trailingX - 10, y: headerRect.minY + 13, width: 10, height: 10))
            trailingX -= 18
        }

        agentCanvasPlainAttributedString(
            item.title ?? "Tool",
            font: .systemFont(ofSize: 11, weight: .semibold),
            color: .labelColor,
            alignment: .left,
            lineSpacing: 1
        )
        .draw(with: CGRect(x: frame.minX + 30, y: headerRect.minY + 8, width: max(trailingX - (frame.minX + 30), 0), height: 20), options: [.usesLineFragmentOrigin, .usesFontLeading])

        guard isExpanded, !item.text.isEmpty else { return }
        let bodyRect = CGRect(x: frame.minX + 10, y: frame.minY + 42, width: max(frame.width - 20, 0), height: max(frame.height - 52, 0))
        NSGraphicsContext.saveGraphicsState()
        NSBezierPath(rect: bodyRect).addClip()
        agentCanvasPlainAttributedString(
            item.text,
            font: .monospacedSystemFont(ofSize: 11, weight: .regular),
            color: .secondaryLabelColor,
            alignment: .left,
            lineSpacing: 1
        )
        .draw(with: bodyRect, options: [.usesLineFragmentOrigin, .usesFontLeading])
        NSGraphicsContext.restoreGraphicsState()
    }

    private func drawCodeBlock(_ metrics: AgentCanvasCodeBlockMetrics, atY y: CGFloat, inWidth width: CGFloat) {
        let outerRect = CGRect(x: 0, y: y, width: width, height: metrics.outerRect.height)
        let path = NSBezierPath(roundedRect: outerRect, xRadius: 12, yRadius: 12)
        NSColor.labelColor.withAlphaComponent(0.035).setFill()
        path.fill()
        NSColor.labelColor.withAlphaComponent(0.08).setStroke()
        path.lineWidth = 0.5
        path.stroke()

        NSColor.labelColor.withAlphaComponent(0.04).setFill()
        NSBezierPath(rect: CGRect(x: outerRect.minX, y: outerRect.minY, width: outerRect.width, height: 32)).fill()

        agentCanvasPlainAttributedString(
            metrics.languageLabel,
            font: .monospacedSystemFont(ofSize: 10, weight: .semibold),
            color: .secondaryLabelColor,
            alignment: .left,
            lineSpacing: 1
        )
        .draw(in: CGRect(x: outerRect.minX + 10, y: outerRect.minY + 9, width: max(outerRect.width - 90, 0), height: 14))

        let copyRect = CGRect(x: outerRect.maxX - 60, y: outerRect.minY + 4, width: 50, height: 24)
        agentCanvasDrawSymbol("doc.on.doc", pointSize: 10, weight: .semibold, color: .secondaryLabelColor, in: CGRect(x: copyRect.minX, y: copyRect.minY + 7, width: 10, height: 10))
        agentCanvasPlainAttributedString(
            "Copy",
            font: .systemFont(ofSize: 10, weight: .semibold),
            color: .secondaryLabelColor,
            alignment: .left,
            lineSpacing: 1
        )
        .draw(in: CGRect(x: copyRect.minX + 14, y: copyRect.minY + 5, width: 36, height: 14))

        metrics.attributed.draw(
            with: CGRect(x: outerRect.minX + 10, y: outerRect.minY + 40, width: max(outerRect.width - 20, 0), height: metrics.contentHeight),
            options: [.usesLineFragmentOrigin, .usesFontLeading]
        )
    }

    private func drawThinkingDots(in rect: CGRect) {
        let dotWidth: CGFloat = 5
        let spacing: CGFloat = 4
        let totalWidth = dotWidth * 3 + spacing * 2
        let startX = rect.minX + max((rect.width - totalWidth) * 0.5, 0)
        for index in 0..<3 {
            let alpha = CGFloat(0.3 + (Double(index) * 0.2))
            NSColor.secondaryLabelColor.withAlphaComponent(alpha).setFill()
            NSBezierPath(ovalIn: CGRect(x: startX + CGFloat(index) * (dotWidth + spacing), y: rect.minY + 2, width: dotWidth, height: dotWidth)).fill()
        }
    }

    private func interaction(at point: CGPoint) -> AgentTranscriptCanvasInteraction? {
        guard let index = rowIndex(containingY: point.y), items.indices.contains(index) else { return nil }
        let item = items[index]
        let layout = rowLayouts[index]
        let frame = CGRect(x: 0, y: layout.y, width: contentWidth, height: layout.height)

        switch item.kind {
        case .tool:
            let headerRect = CGRect(x: frame.minX, y: frame.minY, width: frame.width, height: 36)
            if headerRect.contains(point), !item.text.isEmpty {
                return .toggleTool(item.id)
            }
            return nil
        case .assistant:
            return assistantInteraction(item: item, frame: frame, point: point)
        default:
            return nil
        }
    }

    private func assistantInteraction(item: EditorAgentTranscriptItem, frame: CGRect, point: CGPoint) -> AgentTranscriptCanvasInteraction? {
        let contentWidth = max(frame.width - 10, 180)
        var y = frame.minY

        if let rendered = item.renderedMarkdown, !rendered.blocks.isEmpty {
            for segment in agentCanvasRenderedMarkdownSegments(for: rendered) {
                switch segment {
                case .text(let runs):
                    let attributed = editorDocsAttributedText(for: runs)
                    let rect = CGRect(x: frame.minX, y: y, width: contentWidth, height: ceil(editorDocsAttributedBounds(for: runs, width: contentWidth).height))
                    if let interaction = agentCanvasTextInteraction(at: point, in: rect, attributed: attributed) {
                        return interaction
                    }
                    y += rect.height
                case .code(let language, let runs):
                    let metrics = agentCanvasCodeBlockMetrics(language: language, runs: runs, width: contentWidth)
                    let outerRect = CGRect(x: frame.minX, y: y, width: contentWidth, height: metrics.outerRect.height)
                    let copyRect = CGRect(x: outerRect.maxX - 60, y: outerRect.minY + 4, width: 50, height: 24)
                    if copyRect.contains(point) {
                        return .copy(runs.map(\.text).joined())
                    }
                    let textRect = CGRect(x: outerRect.minX + 10, y: outerRect.minY + 40, width: max(outerRect.width - 20, 0), height: metrics.contentHeight)
                    if let interaction = agentCanvasTextInteraction(at: point, in: textRect, attributed: metrics.attributed) {
                        return interaction
                    }
                    y += outerRect.height
                }
            }
        }

        return nil
    }

    private func pruneRasterCache() {
        let liveIDs = Set(items.map(\.id))
        rasterCache = rasterCache.filter { liveIDs.contains($0.key.item.id) }
        if rasterCache.count > 512 {
            let survivors = Set(items.suffix(200).map(\.id))
            rasterCache = rasterCache.filter { survivors.contains($0.key.item.id) }
        }
    }

    private func rasterKey(for item: EditorAgentTranscriptItem) -> AgentTranscriptCanvasRasterKey {
        AgentTranscriptCanvasRasterKey(
            item: item,
            width: Int(contentWidth.rounded(.toNearestOrAwayFromZero)),
            isExpanded: expandedToolItemIDs.contains(item.id),
            selectionColor: AgentTranscriptCanvasColorKey(selectionColor)
        )
    }

    private func renderRowImage(for item: EditorAgentTranscriptItem, in frame: CGRect) -> CGImage? {
        let size = CGSize(width: max(frame.width, 1), height: max(frame.height, 1))
        let image = NSImage(size: size)
        image.lockFocusFlipped(true)
        defer { image.unlockFocus() }
        NSColor.clear.setFill()
        NSBezierPath(rect: CGRect(origin: .zero, size: size)).fill()
        drawItem(item, in: CGRect(origin: .zero, size: size))
        return image.cgImage(forProposedRect: nil, context: nil, hints: nil)
    }

    private func rowIndex(containingY y: CGFloat) -> Int? {
        let index = max(firstIntersectingIndex(minY: y) - 1, 0)
        guard rowLayouts.indices.contains(index) else { return nil }
        if rowLayouts[index].y <= y, rowLayouts[index].y + rowLayouts[index].height >= y {
            return index
        }
        for next in index..<min(index + 3, rowLayouts.count) {
            let layout = rowLayouts[next]
            if layout.y <= y, layout.y + layout.height >= y {
                return next
            }
        }
        return nil
    }

    private func firstIntersectingIndex(minY: CGFloat) -> Int {
        var lower = 0
        var upper = rowLayouts.count
        while lower < upper {
            let mid = (lower + upper) / 2
            let maxY = rowLayouts[mid].y + rowLayouts[mid].height
            if maxY < minY {
                lower = mid + 1
            } else {
                upper = mid
            }
        }
        return lower
    }
}

private enum AgentCanvasMarkdownSegment {
    case text([EditorDocsRun])
    case code(language: String?, runs: [EditorDocsRun])
}

private struct AgentCanvasCodeBlockMetrics {
    let attributed: NSAttributedString
    let contentHeight: CGFloat
    let outerRect: CGRect
    let languageLabel: String
}

private func agentCanvasRenderedMarkdownSegments(for rendered: EditorRenderedMarkdown) -> [AgentCanvasMarkdownSegment] {
    var result: [AgentCanvasMarkdownSegment] = []
    var pendingTextBlocks: [EditorMarkdownBlock] = []

    func flush() {
        let runs = agentCanvasMergedRuns(for: pendingTextBlocks, rendered: rendered)
        if !runs.isEmpty {
            result.append(.text(runs))
        }
        pendingTextBlocks.removeAll()
    }

    for block in rendered.blocks {
        if block.kind == .codeFence {
            flush()
            result.append(.code(language: block.language, runs: agentCanvasRuns(for: block, rendered: rendered)))
        } else {
            pendingTextBlocks.append(block)
        }
    }
    flush()
    return result
}

private func agentCanvasMergedRuns(for blocks: [EditorMarkdownBlock], rendered: EditorRenderedMarkdown) -> [EditorDocsRun] {
    guard !blocks.isEmpty else { return [] }
    var merged: [EditorDocsRun] = []
    for (index, block) in blocks.enumerated() {
        if block.kind != .blankLine {
            merged.append(contentsOf: agentCanvasRuns(for: block, rendered: rendered))
        }
        guard index < blocks.count - 1 else { continue }
        let nextBlock = blocks[index + 1]
        let referenceRun = merged.last ?? agentCanvasRuns(for: nextBlock, rendered: rendered).first
        merged.append(
            EditorDocsRun(
                text: "\n",
                style: referenceRun?.style ?? EditorResolvedStyle(fg: nil, bg: nil, underlineColor: nil, addModifiers: 0, removeModifiers: 0, underlineStyle: 0),
                kind: .body,
                linkDestination: nil
            )
        )
    }
    return merged
}

private func agentCanvasRuns(for block: EditorMarkdownBlock, rendered: EditorRenderedMarkdown) -> [EditorDocsRun] {
    guard block.runCount > 0,
          block.runStart >= 0,
          block.runStart + block.runCount <= rendered.runs.count else { return [] }
    return Array(rendered.runs[block.runStart..<(block.runStart + block.runCount)])
}

private func agentCanvasPlainAttributedString(
    _ text: String,
    font: NSFont,
    color: NSColor,
    alignment: NSTextAlignment,
    lineSpacing: CGFloat
) -> NSAttributedString {
    let paragraphStyle = NSMutableParagraphStyle()
    paragraphStyle.lineBreakMode = .byWordWrapping
    paragraphStyle.alignment = alignment
    paragraphStyle.lineSpacing = lineSpacing
    return NSAttributedString(
        string: text,
        attributes: [
            .font: font,
            .foregroundColor: color,
            .paragraphStyle: paragraphStyle,
        ]
    )
}

private func agentCanvasMeasurePlainTextHeight(_ text: String, width: CGFloat, font: NSFont, lineSpacing: CGFloat) -> CGFloat {
    let attributed = agentCanvasPlainAttributedString(text, font: font, color: .labelColor, alignment: .left, lineSpacing: lineSpacing)
    return ceil(attributed.boundingRect(with: CGSize(width: max(width, 1), height: .greatestFiniteMagnitude), options: [.usesLineFragmentOrigin, .usesFontLeading]).height)
}

@MainActor
private func agentCanvasCodeBlockMetrics(language: String?, runs: [EditorDocsRun], width: CGFloat) -> AgentCanvasCodeBlockMetrics {
    let attributed = editorDocsAttributedText(for: runs)
    let contentHeight = ceil(editorDocsAttributedBounds(for: runs, width: max(width - 20, 160)).height)
    return AgentCanvasCodeBlockMetrics(
        attributed: attributed,
        contentHeight: contentHeight,
        outerRect: CGRect(x: 0, y: 0, width: width, height: contentHeight + 62),
        languageLabel: (language?.isEmpty == false ? language! : "code").uppercased()
    )
}

private func agentCanvasToolStatusSymbol(for status: String?) -> (name: String, color: NSColor) {
    switch status {
    case "failed":
        return ("xmark.circle.fill", .systemRed)
    case "done":
        return ("checkmark.circle.fill", .systemGreen)
    default:
        return ("circle.dotted", .secondaryLabelColor)
    }
}

private func agentCanvasDrawSymbol(_ name: String, pointSize: CGFloat, weight: NSFont.Weight, color: NSColor, in rect: CGRect) {
    let _ = color
    guard let image = NSImage(systemSymbolName: name, accessibilityDescription: nil)?.withSymbolConfiguration(.init(pointSize: pointSize, weight: weight)) else { return }
    image.draw(in: rect)
}

private func agentCanvasTextInteraction(at point: CGPoint, in rect: CGRect, attributed: NSAttributedString) -> AgentTranscriptCanvasInteraction? {
    guard rect.contains(point) else { return nil }
    let storage = NSTextStorage(attributedString: attributed)
    let layoutManager = NSLayoutManager()
    storage.addLayoutManager(layoutManager)
    let textContainer = NSTextContainer(size: CGSize(width: rect.width, height: CGFloat.greatestFiniteMagnitude))
    textContainer.lineFragmentPadding = 0
    textContainer.maximumNumberOfLines = 0
    textContainer.lineBreakMode = .byWordWrapping
    layoutManager.addTextContainer(textContainer)

    let localPoint = CGPoint(x: point.x - rect.minX, y: point.y - rect.minY)
    let glyphIndex = layoutManager.glyphIndex(for: localPoint, in: textContainer)
    guard glyphIndex < layoutManager.numberOfGlyphs else { return nil }
    let glyphRect = layoutManager.boundingRect(forGlyphRange: NSRange(location: glyphIndex, length: 1), in: textContainer)
    guard glyphRect.contains(localPoint) else { return nil }
    let charIndex = layoutManager.characterIndexForGlyph(at: glyphIndex)
    guard charIndex < storage.length else { return nil }

    if let url = storage.attribute(.link, at: charIndex, effectiveRange: nil) as? URL {
        return .openLink(url)
    }
    if let raw = storage.attribute(.link, at: charIndex, effectiveRange: nil) as? String {
        return .openStringLink(raw)
    }
    return nil
}
