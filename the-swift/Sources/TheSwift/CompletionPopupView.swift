import AppKit
import Foundation
import SwiftUI
import class TheEditorFFIBridge.App

struct CompletionPopupView: View {
    let snapshot: CompletionSnapshot
    let cursorOrigin: CGPoint
    let cellSize: CGSize
    let containerSize: CGSize
    let languageHint: String
    let onSelect: (Int) -> Void
    let onSubmit: (Int) -> Void

    @State private var hoveredItemId: Int? = nil

    private let maxVisibleItems = 8
    private let rowHeight: CGFloat = 24
    private let minListWidth: CGFloat = 180
    private let maxListWidth: CGFloat = 320
    private let minDocsWidth: CGFloat = 200
    private let maxDocsWidth: CGFloat = 360
    private let minDocsHeight: CGFloat = 44
    private let maxDocsHeight: CGFloat = 280
    private let docsLineHeight: CGFloat = 18

    private var docsText: String? {
        guard !CompletionPopupRenderConfig.disableDocs else {
            return nil
        }
        return snapshot.docsText
    }

    var body: some View {
        let placement = computePlacement()

        ZStack(alignment: .topLeading) {
            completionList(width: placement.listWidth, height: placement.listHeight)
                .offset(x: placement.listX, y: placement.listY)

            if let docsText, let docsPlacement = placement.docs {
                docsPanel(text: docsText, width: docsPlacement.width, height: docsPlacement.height)
                    .offset(x: docsPlacement.x, y: docsPlacement.y)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .onDisappear {
            DispatchQueue.main.async {
                KeyCaptureFocusBridge.shared.reclaimActive()
            }
        }
    }

    // MARK: - Placement

    private struct DocsPlacement {
        let x: CGFloat
        let y: CGFloat
        let width: CGFloat
        let height: CGFloat
    }

    private struct Placement {
        let listX: CGFloat
        let listY: CGFloat
        let listWidth: CGFloat
        let listHeight: CGFloat
        let docs: DocsPlacement?
    }

    private var resolvedListWidth: CGFloat {
        let longestLabel = snapshot.items
            .prefix(24)
            .map { item in
                let detailBonus = min(18, item.detail?.count ?? 0)
                return item.label.count + detailBonus
            }
            .max() ?? 20
        let estimated = CGFloat(min(48, max(20, longestLabel))) * 6.8 + 24
        return min(max(minListWidth, estimated), maxListWidth)
    }

    private func estimatedDocsWidth(for docs: String) -> CGFloat {
        let longestLine = docs
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map { line in
                line.replacingOccurrences(of: "\t", with: "    ").count
            }
            .filter { $0 > 0 }
            .prefix(12)
            .max() ?? 40
        let targetColumns = min(58, max(30, longestLine))
        let width = CGFloat(targetColumns) * 6.9 + 24
        return min(max(minDocsWidth, width), maxDocsWidth)
    }

    private func estimatedDocsHeight(for docs: String, width: CGFloat) -> CGFloat {
        let columns = max(30, Int((width - 22) / 6.8))
        var wrappedLines = 0
        for rawLine in docs.split(separator: "\n", omittingEmptySubsequences: false) {
            let line = rawLine.replacingOccurrences(of: "\t", with: "    ")
            if line.trimmingCharacters(in: .whitespaces).isEmpty {
                wrappedLines += 1
                continue
            }

            var currentColumn = 0
            var lineWraps = 1
            for word in line.split(separator: " ", omittingEmptySubsequences: false) {
                let wordLen = max(1, word.count)
                if currentColumn == 0 {
                    currentColumn = wordLen
                    continue
                }
                if currentColumn + 1 + wordLen <= columns {
                    currentColumn += 1 + wordLen
                } else {
                    lineWraps += 1
                    currentColumn = wordLen
                }
            }
            wrappedLines += lineWraps
        }

        let lineCount = min(16, max(2, wrappedLines))
        let height = CGFloat(lineCount) * docsLineHeight + 14
        return min(max(minDocsHeight, height), maxDocsHeight)
    }

    private struct SharedRect: Decodable {
        let x: Int
        let y: Int
        let width: Int
        let height: Int
    }

    private struct SharedPlacement: Decodable {
        let list: SharedRect
        let docs: SharedRect?
    }

    private func cellsForWidth(_ width: CGFloat) -> Int {
        let cellWidth = max(1, cellSize.width)
        return max(1, Int(ceil(width / cellWidth)))
    }

    private func cellsForHeight(_ height: CGFloat) -> Int {
        let cellHeight = max(1, cellSize.height)
        return max(1, Int(ceil(height / cellHeight)))
    }

    private func pixelsForCols(_ cols: Int) -> CGFloat {
        CGFloat(max(0, cols)) * max(1, cellSize.width)
    }

    private func pixelsForRows(_ rows: Int) -> CGFloat {
        CGFloat(max(0, rows)) * max(1, cellSize.height)
    }

    private func computePlacement() -> Placement {
        let visibleCount = max(1, min(snapshot.items.count, maxVisibleItems))
        let desiredListHeight = CGFloat(visibleCount) * rowHeight + 8
        let desiredListWidth = resolvedListWidth

        let areaX: CGFloat = 0
        let areaY: CGFloat = 0
        let areaWidth = max(1, containerSize.width)
        let areaHeight = max(1, containerSize.height)
        let areaCols = max(1, Int(floor(areaWidth / max(1, cellSize.width))))
        let areaRows = max(1, Int(floor(areaHeight / max(1, cellSize.height))))
        // Cursor origin is already in viewport coordinates (same origin as the shared layout area).
        // Do not subtract edge padding here, otherwise the popup is biased up/left by ~1 cell.
        let cursorCol = Int(floor(cursorOrigin.x / max(1, cellSize.width)))
        let cursorRow = Int(floor(cursorOrigin.y / max(1, cellSize.height)))

        let listWidthCells = min(areaCols, cellsForWidth(min(desiredListWidth, areaWidth)))
        let listHeightCells = min(areaRows, cellsForHeight(min(desiredListHeight, areaHeight)))

        var docsWidthCells = 0
        var docsHeightCells = 0
        if let docsText {
            let docsWidth = min(estimatedDocsWidth(for: docsText), areaWidth)
            let docsHeight = min(estimatedDocsHeight(for: docsText, width: docsWidth), areaHeight)
            docsWidthCells = min(areaCols, cellsForWidth(docsWidth))
            docsHeightCells = min(areaRows, cellsForHeight(docsHeight))
        }

        let layoutJSON = App.completion_popup_layout_json(
            UInt(areaCols),
            UInt(areaRows),
            Int64(cursorCol),
            Int64(cursorRow),
            UInt(listWidthCells),
            UInt(listHeightCells),
            UInt(docsWidthCells),
            UInt(docsHeightCells)
        ).toString()

        let placementData = Data(layoutJSON.utf8)
        let shared = (try? JSONDecoder().decode(SharedPlacement.self, from: placementData))
            ?? SharedPlacement(
                list: SharedRect(
                    x: 0,
                    y: 0,
                    width: listWidthCells,
                    height: listHeightCells
                ),
                docs: nil
            )

        let listX = areaX + pixelsForCols(shared.list.x)
        let listY = areaY + pixelsForRows(shared.list.y)
        let listWidth = pixelsForCols(shared.list.width)
        let listHeight = pixelsForRows(shared.list.height)

        let docsPlacement = shared.docs.map { docsRect in
            DocsPlacement(
                x: areaX + pixelsForCols(docsRect.x),
                y: listY,
                width: pixelsForCols(docsRect.width),
                height: pixelsForRows(docsRect.height)
            )
        }

        return Placement(
            listX: listX,
            listY: listY,
            listWidth: listWidth,
            listHeight: listHeight,
            docs: docsPlacement
        )
    }

    // MARK: - List

    private func completionList(width: CGFloat, height: CGFloat) -> some View {
        ScrollViewReader { proxy in
            ScrollView(.vertical, showsIndicators: true) {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(snapshot.items) { item in
                        completionRow(
                            item: item,
                            isSelected: snapshot.selectedIndex == item.id,
                            isHovered: hoveredItemId == item.id
                        )
                        .contentShape(Rectangle())
                        .onHover { hovering in
                            if hovering {
                                hoveredItemId = item.id
                            } else if hoveredItemId == item.id {
                                hoveredItemId = nil
                            }
                        }
                        .onTapGesture {
                            if snapshot.selectedIndex == item.id {
                                onSubmit(item.id)
                            } else {
                                onSelect(item.id)
                            }
                        }
                            .id(item.id)
                    }
                }
                .padding(.vertical, 4)
            }
            .frame(width: width, height: height)
            .scrollIndicators(.visible)
            .glassBackground(cornerRadius: 8)
            .onChange(of: snapshot.selectedIndex) { newIndex in
                guard let index = newIndex else { return }
                withAnimation(.none) {
                    proxy.scrollTo(index, anchor: nil)
                }
            }
            .onAppear {
                if let index = snapshot.selectedIndex {
                    proxy.scrollTo(index, anchor: nil)
                }
            }
        }
    }

    private func completionRow(item: CompletionItemSnapshot, isSelected: Bool, isHovered: Bool) -> some View {
        HStack(spacing: 6) {
            // Kind icon badge.
            if let icon = item.kindIcon, !icon.isEmpty {
                let color = item.kindColor ?? Color(nsColor: .tertiaryLabelColor)
                Text(icon)
                    .font(.system(size: 10, weight: .bold, design: .monospaced))
                    .foregroundColor(color)
                    .frame(width: 18, height: 18)
                    .background(
                        RoundedRectangle(cornerRadius: 4)
                            .fill(color.opacity(0.15))
                    )
            } else {
                Color.clear.frame(width: 18, height: 18)
            }

            // Label.
            Text(item.label)
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(.primary)
                .lineLimit(1)

            Spacer(minLength: 4)

            // Detail (type signature).
            if let detail = item.detail, !detail.isEmpty {
                Text(detail)
                    .font(.system(size: 12))
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
                    .truncationMode(.tail)
                    .frame(maxWidth: 120, alignment: .trailing)
            }
        }
        .padding(.horizontal, 8)
        .frame(height: rowHeight)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 5)
                .fill(
                    isSelected
                        ? Color.accentColor.opacity(0.22)
                        : (isHovered ? Color.white.opacity(0.08) : Color.clear)
                )
                .padding(.horizontal, 4)
        )
    }

    // MARK: - Docs

    @ViewBuilder
    private func docsPanel(text: String, width: CGFloat, height: CGFloat) -> some View {
        CompletionDocsTextView(
            docs: text,
            width: width,
            height: height,
            languageHint: languageHint
        )
        .frame(width: width, height: height)
        .glassBackground(cornerRadius: 8)
    }
}

private struct CompletionDocsTextView: View {
    let docs: String
    let width: CGFloat
    let height: CGFloat
    let languageHint: String

    @Environment(\.colorScheme) private var colorScheme
    @StateObject private var viewModel = CompletionDocsViewModel()

    private var contentWidth: CGFloat {
        max(1, width - 20)
    }

    private var theme: CompletionDocsTheme {
        CompletionDocsTheme(colorScheme: colorScheme)
    }

    var body: some View {
        CompletionDocsAppKitTextView(
            attributedText: viewModel.attributedText,
            isLoading: viewModel.isLoading,
            renderKey: viewModel.renderKey
        )
        .onAppear {
            viewModel.update(docs: docs, contentWidth: contentWidth, theme: theme, languageHint: languageHint)
        }
        .onChange(of: docs) { nextDocs in
            viewModel.update(docs: nextDocs, contentWidth: contentWidth, theme: theme, languageHint: languageHint)
        }
        .onChange(of: contentWidth) { nextWidth in
            viewModel.update(docs: docs, contentWidth: nextWidth, theme: theme, languageHint: languageHint)
        }
        .onChange(of: colorScheme) { _ in
            viewModel.update(docs: docs, contentWidth: contentWidth, theme: theme, languageHint: languageHint)
        }
        .onChange(of: languageHint) { nextHint in
            viewModel.update(docs: docs, contentWidth: contentWidth, theme: theme, languageHint: nextHint)
        }
    }
}

private struct CompletionDocsAppKitTextView: NSViewRepresentable {
    let attributedText: NSAttributedString
    let isLoading: Bool
    let renderKey: String

    final class CompletionDocsScrollView: NSScrollView {
        override func mouseUp(with event: NSEvent) {
            super.mouseUp(with: event)
            KeyCaptureFocusBridge.shared.reclaim(in: window)
        }
    }

    final class CompletionDocsNativeTextView: NSTextView {
        override func keyDown(with event: NSEvent) {
            if event.modifierFlags.contains(.command) {
                super.keyDown(with: event)
                return
            }
            if let responder = KeyCaptureFocusBridge.shared.keyResponder(in: window) {
                responder.keyDown(with: event)
                return
            }
            super.keyDown(with: event)
        }

        override func mouseUp(with event: NSEvent) {
            super.mouseUp(with: event)
            KeyCaptureFocusBridge.shared.reclaim(in: window)
        }
    }

    final class Coordinator {
        var lastRenderKey: String?
        var lastLoadingState = false
        private var keyMonitor: Any?

        deinit {
            if let keyMonitor {
                NSEvent.removeMonitor(keyMonitor)
            }
        }

        func installKeyForwardingMonitor() {
            guard keyMonitor == nil else {
                return
            }

            keyMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown]) { event in
                // Preserve standard command shortcuts like copy/select-all while docs text is focused.
                if event.modifierFlags.contains(.command) {
                    return event
                }

                guard let window = event.window,
                      let keyResponder = KeyCaptureFocusBridge.shared.keyResponder(in: window) else {
                    return event
                }

                // Let the normal responder chain handle keys if key capture already owns focus.
                if window.firstResponder === keyResponder {
                    return event
                }

                keyResponder.keyDown(with: event)
                return nil
            }
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> NSScrollView {
        context.coordinator.installKeyForwardingMonitor()

        let scrollView = CompletionDocsScrollView()
        scrollView.borderType = .noBorder
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.scrollerStyle = .overlay

        let textView = CompletionDocsNativeTextView(frame: .zero)
        textView.isEditable = false
        textView.isSelectable = true
        textView.drawsBackground = false
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.minSize = .zero
        textView.textContainerInset = NSSize(width: 10, height: 10)
        textView.textContainer?.lineFragmentPadding = 0
        textView.textContainer?.widthTracksTextView = true
        textView.textContainer?.heightTracksTextView = false
        textView.textContainer?.lineBreakMode = .byWordWrapping
        textView.linkTextAttributes = [
            .foregroundColor: NSColor.systemBlue,
            .underlineStyle: NSUnderlineStyle.single.rawValue,
        ]

        scrollView.documentView = textView
        scrollView.verticalScroller?.controlSize = .small
        applyContent(to: textView, coordinator: context.coordinator)
        return scrollView
    }

    func updateNSView(_ nsView: NSScrollView, context: Context) {
        context.coordinator.installKeyForwardingMonitor()

        guard let textView = nsView.documentView as? NSTextView else {
            return
        }

        let shouldResetScroll =
            context.coordinator.lastRenderKey != nil &&
            context.coordinator.lastRenderKey != renderKey

        applyContent(to: textView, coordinator: context.coordinator)
        if shouldResetScroll {
            nsView.contentView.scroll(to: .zero)
            nsView.reflectScrolledClipView(nsView.contentView)
        }
    }

    private func applyContent(to textView: NSTextView, coordinator: Coordinator) {
        if coordinator.lastRenderKey == renderKey && coordinator.lastLoadingState == isLoading {
            return
        }
        coordinator.lastRenderKey = renderKey
        coordinator.lastLoadingState = isLoading

        if isLoading && attributedText.length == 0 {
            textView.textStorage?.setAttributedString(CompletionDocsRenderer.loadingPlaceholder)
            return
        }
        textView.textStorage?.setAttributedString(attributedText)
    }
}

@MainActor
private final class CompletionDocsViewModel: ObservableObject {
    @Published private(set) var attributedText: NSAttributedString = CompletionDocsRenderer.emptyPlaceholder
    @Published private(set) var isLoading: Bool = false
    @Published private(set) var renderKey: String = "initial"

    private var activeRequestToken = UUID()
    private var currentCacheKey: String? = nil
    private var lastDocsFingerprint: String? = nil

    func update(docs: String, contentWidth: CGFloat, theme: CompletionDocsTheme, languageHint: String) {
        let cacheKey = CompletionDocsLayoutCache.cacheKey(
            for: docs,
            contentWidth: contentWidth,
            themeKey: theme.cacheKey,
            languageHint: languageHint
        )
        if let currentCacheKey, currentCacheKey == cacheKey {
            return
        }
        currentCacheKey = cacheKey

        if let cachedText = CompletionDocsLayoutCache.shared.attributedText(for: cacheKey) {
            attributedText = cachedText
            renderKey = cacheKey
            isLoading = false
            return
        }

        let docsFingerprint = CompletionDocsLayoutCache.docsFingerprint(for: docs)
        let docsChanged = lastDocsFingerprint != docsFingerprint
        lastDocsFingerprint = docsFingerprint
        if docsChanged || attributedText.length == 0 {
            isLoading = true
        }
        let requestToken = UUID()
        activeRequestToken = requestToken
        let docsCopy = docs
        let widthCopy = contentWidth
        let themeCopy = theme
        let hintCopy = languageHint

        DispatchQueue.global(qos: .userInitiated).async {
            let rendered = CompletionDocsRenderer.render(
                docs: docsCopy,
                contentWidth: widthCopy,
                theme: themeCopy,
                languageHint: hintCopy
            )
            CompletionDocsLayoutCache.shared.set(attributedText: rendered, for: cacheKey)
            DispatchQueue.main.async {
                guard self.activeRequestToken == requestToken else {
                    return
                }
                self.attributedText = rendered
                self.renderKey = cacheKey
                self.isLoading = false
            }
        }
    }
}

private final class CompletionDocsLayoutCache {
    static let shared = CompletionDocsLayoutCache()

    private let cache = NSCache<NSString, CompletionDocsLayoutCacheEntry>()

    private init() {
        cache.countLimit = 128
    }

    func attributedText(for key: String) -> NSAttributedString? {
        cache.object(forKey: key as NSString)?.attributedText
    }

    func set(attributedText: NSAttributedString, for key: String) {
        let immutableCopy = attributedText.copy() as? NSAttributedString ?? attributedText
        cache.setObject(
            CompletionDocsLayoutCacheEntry(attributedText: immutableCopy),
            forKey: key as NSString,
            cost: immutableCopy.length
        )
    }

    static func cacheKey(
        for docs: String,
        contentWidth: CGFloat,
        themeKey: String,
        languageHint: String
    ) -> String {
        let widthBucket = Int(contentWidth.rounded(.toNearestOrAwayFromZero))
        var hasher = Hasher()
        hasher.combine(widthBucket)
        hasher.combine(themeKey)
        hasher.combine(languageHint)
        hasher.combine(docs)
        hasher.combine(docs.count)
        let hash = hasher.finalize()
        return "\(themeKey):\(languageHint):\(widthBucket):\(docs.count):\(hash)"
    }

    static func docsFingerprint(for docs: String) -> String {
        var hasher = Hasher()
        hasher.combine(docs)
        hasher.combine(docs.count)
        return String(hasher.finalize())
    }
}

private final class CompletionDocsLayoutCacheEntry: NSObject {
    let attributedText: NSAttributedString

    init(attributedText: NSAttributedString) {
        self.attributedText = attributedText
    }
}

private struct CompletionDocsTheme: Hashable {
    let colorScheme: ColorScheme

    var cacheKey: String {
        switch colorScheme {
        case .dark:
            return "dark"
        case .light:
            return "light"
        @unknown default:
            return "unknown"
        }
    }

    var baseFont: NSFont {
        NSFont.systemFont(ofSize: 13, weight: .regular)
    }

    var codeFont: NSFont {
        NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
    }

    var bodyColor: NSColor {
        colorScheme == .dark
            ? NSColor(calibratedRed: 0.78, green: 0.80, blue: 0.88, alpha: 1)
            : NSColor.secondaryLabelColor
    }

    var headingColor: NSColor {
        colorScheme == .dark
            ? NSColor(calibratedRed: 0.88, green: 0.89, blue: 0.95, alpha: 1)
            : NSColor.labelColor
    }

    var linkColor: NSColor {
        colorScheme == .dark
            ? NSColor(calibratedRed: 0.58, green: 0.72, blue: 1.00, alpha: 1)
            : NSColor.systemBlue
    }

    var codeColor: NSColor {
        colorScheme == .dark
            ? NSColor(calibratedRed: 0.73, green: 0.75, blue: 0.86, alpha: 1)
            : NSColor.labelColor
    }

    var keywordColor: NSColor {
        colorScheme == .dark
            ? NSColor(calibratedRed: 0.80, green: 0.66, blue: 0.98, alpha: 1)
            : NSColor.systemPurple
    }

    var typeColor: NSColor {
        colorScheme == .dark
            ? NSColor(calibratedRed: 0.59, green: 0.75, blue: 1.00, alpha: 1)
            : NSColor.systemBlue
    }

    var numberColor: NSColor {
        colorScheme == .dark
            ? NSColor(calibratedRed: 0.95, green: 0.73, blue: 0.46, alpha: 1)
            : NSColor.systemOrange
    }

    var stringColor: NSColor {
        colorScheme == .dark
            ? NSColor(calibratedRed: 0.60, green: 0.84, blue: 0.65, alpha: 1)
            : NSColor.systemGreen
    }

    var commentColor: NSColor {
        colorScheme == .dark
            ? NSColor(calibratedRed: 0.50, green: 0.52, blue: 0.63, alpha: 1)
            : NSColor.tertiaryLabelColor
    }

    func paragraphStyle(code: Bool) -> NSParagraphStyle {
        let style = NSMutableParagraphStyle()
        style.lineBreakMode = code ? .byCharWrapping : .byWordWrapping
        style.lineSpacing = code ? 1 : 3
        style.paragraphSpacing = code ? 4 : 8
        return style
    }

    var estimatedColumnWidth: CGFloat {
        let attrs: [NSAttributedString.Key: Any] = [.font: baseFont]
        let sample = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
        let width = (sample as NSString).size(withAttributes: attrs).width / CGFloat(sample.count)
        return max(5, width)
    }

    func proportionalVariant(for font: NSFont?) -> NSFont {
        guard let font else {
            return baseFont
        }
        let size = max(11, font.pointSize)
        let bold = font.fontDescriptor.symbolicTraits.contains(.bold)
        let italic = font.fontDescriptor.symbolicTraits.contains(.italic)
        var converted = NSFont.systemFont(ofSize: size, weight: bold ? .semibold : .regular)
        if italic {
            converted = NSFontManager.shared.convert(converted, toHaveTrait: .italicFontMask)
        }
        return converted
    }

    func monospacedVariant(for font: NSFont?) -> NSFont {
        guard let font else {
            return codeFont
        }
        let size = max(10, font.pointSize)
        let bold = font.fontDescriptor.symbolicTraits.contains(.bold)
        let italic = font.fontDescriptor.symbolicTraits.contains(.italic)
        var converted = NSFont.monospacedSystemFont(ofSize: size, weight: bold ? .semibold : .regular)
        if italic {
            converted = NSFontManager.shared.convert(converted, toHaveTrait: .italicFontMask)
        }
        return converted
    }
}

private enum CompletionDocsRenderer {
    private static let modifierBold: UInt16 = 0b0000_0000_0001
    private static let modifierDim: UInt16 = 0b0000_0000_0010
    private static let modifierItalic: UInt16 = 0b0000_0000_0100

    private struct RustDocsPayload: Decodable {
        let lines: [[RustDocsRun]]
    }

    private struct RustDocsRun: Decodable {
        let text: String
        let style: RustDocsStyle
    }

    private struct RustDocsStyle: Decodable {
        let has_fg: Bool
        let fg: RustDocsColor
        let has_bg: Bool
        let bg: RustDocsColor
        let has_underline_color: Bool
        let underline_color: RustDocsColor
        let underline_style: UInt8
        let add_modifier: UInt16
        let sub_modifier: UInt16
    }

    private struct RustDocsColor: Decodable {
        let kind: UInt8
        let value: UInt32
    }

    private enum MarkdownChunk {
        case markdown(String)
        case code(language: String?, text: String)
    }

    private enum CodeTokenKind {
        case code
        case keyword
        case typeName
        case number
        case string
        case comment
    }

    private struct CodeToken {
        var text: String
        let kind: CodeTokenKind
    }

    static let emptyPlaceholder = NSAttributedString(string: "")
    static let loadingPlaceholder = NSAttributedString(
        string: "Loading docs...",
        attributes: [
            .font: NSFont.systemFont(ofSize: 13, weight: .regular),
            .foregroundColor: NSColor.secondaryLabelColor,
        ]
    )

    static func render(
        docs: String,
        contentWidth: CGFloat,
        theme: CompletionDocsTheme,
        languageHint: String
    ) -> NSAttributedString {
        let trimmed = docs.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return placeholder("No docs", theme: theme)
        }

        if let fromRust = renderViaRust(
            docs: docs,
            contentWidth: contentWidth,
            theme: theme,
            languageHint: languageHint
        ) {
            return fromRust
        }

        let chunks = splitMarkdownAndCodeBlocks(in: docs)
        let rendered = NSMutableAttributedString()
        var firstChunk = true

        for chunk in chunks {
            let chunkText: NSAttributedString
            switch chunk {
            case .markdown(let markdown):
                chunkText = renderMarkdown(markdown, theme: theme)
            case .code(let language, let text):
                chunkText = renderCodeBlock(text, language: language, theme: theme)
            }

            guard chunkText.length > 0 else {
                continue
            }

            if !firstChunk, !rendered.string.hasSuffix("\n") {
                rendered.append(NSAttributedString(string: "\n", attributes: baseAttributes(theme: theme)))
            }
            rendered.append(chunkText)
            if !rendered.string.hasSuffix("\n") {
                rendered.append(NSAttributedString(string: "\n", attributes: baseAttributes(theme: theme)))
            }
            firstChunk = false
        }

        if rendered.length == 0 {
            return placeholder("No docs", theme: theme)
        }
        return rendered
    }

    private static func renderViaRust(
        docs: String,
        contentWidth: CGFloat,
        theme: CompletionDocsTheme,
        languageHint: String
    ) -> NSAttributedString? {
        _ = contentWidth
        _ = theme
        // Keep markdown/code highlighting in Rust, but let AppKit own soft wrapping
        // so GUI wrapping follows exact view width instead of pre-wrapped rows.
        let wrapColumns = 2048
        let json = App.completion_docs_render_json(docs, UInt(wrapColumns), languageHint).toString()
        guard let data = json.data(using: .utf8),
              let payload = try? JSONDecoder().decode(RustDocsPayload.self, from: data) else {
            return nil
        }

        let rendered = NSMutableAttributedString()
        for (lineIndex, line) in payload.lines.enumerated() {
            if line.isEmpty {
                // Preserve blank lines from Rust markdown wrapping.
            } else {
                for run in line {
                    rendered.append(
                        NSAttributedString(
                            string: run.text,
                            attributes: attributes(for: run.style, theme: theme)
                        )
                    )
                }
            }

            if lineIndex + 1 < payload.lines.count {
                rendered.append(NSAttributedString(string: "\n", attributes: baseAttributes(theme: theme)))
            }
        }

        return rendered.length > 0 ? rendered : nil
    }

    private static func attributes(for style: RustDocsStyle, theme: CompletionDocsTheme) -> [NSAttributedString.Key: Any] {
        _ = style.sub_modifier
        let codeLike = isCodeStyle(style)
        let headingLike = !codeLike && (style.add_modifier & modifierBold) != 0
        let baseSize = codeLike ? theme.codeFont.pointSize : theme.baseFont.pointSize
        let targetSize = headingLike ? baseSize + 1 : baseSize
        let weight: NSFont.Weight = (style.add_modifier & modifierBold) != 0 ? .semibold : .regular

        var font = codeLike
            ? NSFont.monospacedSystemFont(ofSize: targetSize, weight: weight)
            : NSFont.systemFont(ofSize: targetSize, weight: weight)
        if (style.add_modifier & modifierItalic) != 0 {
            font = NSFontManager.shared.convert(font, toHaveTrait: .italicFontMask)
        }

        let defaultForeground = headingLike ? theme.headingColor : (codeLike ? theme.codeColor : theme.bodyColor)
        var foreground = style.has_fg ? nsColor(from: style.fg) ?? defaultForeground : defaultForeground
        if (style.add_modifier & modifierDim) != 0 {
            foreground = foreground.withAlphaComponent(foreground.alphaComponent * 0.72)
        }

        var attrs: [NSAttributedString.Key: Any] = [
            .font: font,
            .foregroundColor: foreground,
            .paragraphStyle: theme.paragraphStyle(code: codeLike),
        ]

        if style.has_bg, let bg = nsColor(from: style.bg) {
            attrs[.backgroundColor] = bg
        }
        if style.underline_style != 0 {
            attrs[.underlineStyle] = NSUnderlineStyle.single.rawValue
            if style.has_underline_color, let underline = nsColor(from: style.underline_color) {
                attrs[.underlineColor] = underline
            }
        }
        return attrs
    }

    private static func isCodeStyle(_ style: RustDocsStyle) -> Bool {
        if style.has_bg {
            return true
        }
        if (style.add_modifier & modifierDim) != 0 && (style.add_modifier & modifierBold) == 0 {
            return true
        }
        return false
    }

    private static func nsColor(from color: RustDocsColor) -> NSColor? {
        switch color.kind {
        case 0:
            return nil
        case 1:
            let palette: [NSColor] = [
                .black, .systemRed, .systemGreen, .systemYellow, .systemBlue, .systemPurple, .systemCyan, .gray,
                .systemRed.withAlphaComponent(0.85), .systemGreen.withAlphaComponent(0.85),
                .systemYellow.withAlphaComponent(0.85), .systemBlue.withAlphaComponent(0.85),
                .systemPurple.withAlphaComponent(0.85), .systemCyan.withAlphaComponent(0.85),
                .lightGray, .white,
            ]
            let idx = Int(color.value)
            guard idx >= 0, idx < palette.count else {
                return NSColor.white
            }
            return palette[idx]
        case 2:
            let r = CGFloat((color.value >> 16) & 0xFF) / 255.0
            let g = CGFloat((color.value >> 8) & 0xFF) / 255.0
            let b = CGFloat(color.value & 0xFF) / 255.0
            return NSColor(red: r, green: g, blue: b, alpha: 1.0)
        case 3:
            return xterm256Color(index: Int(color.value))
        default:
            return nil
        }
    }

    private static func xterm256Color(index: Int) -> NSColor? {
        guard index >= 0 else {
            return nil
        }
        if index < 16 {
            return nsColor(from: RustDocsColor(kind: 1, value: UInt32(index)))
        }
        if index >= 232 {
            let level = CGFloat(index - 232) / 23.0
            return NSColor(white: level, alpha: 1.0)
        }

        let idx = index - 16
        let r = idx / 36
        let g = (idx % 36) / 6
        let b = idx % 6
        func component(_ value: Int) -> CGFloat {
            let levels: [CGFloat] = [0.0, 0.37, 0.58, 0.74, 0.87, 1.0]
            return levels[min(max(value, 0), levels.count - 1)]
        }

        return NSColor(
            red: component(r),
            green: component(g),
            blue: component(b),
            alpha: 1.0
        )
    }

    private static func placeholder(_ text: String, theme: CompletionDocsTheme) -> NSAttributedString {
        NSAttributedString(
            string: text,
            attributes: [
                .font: theme.baseFont,
                .foregroundColor: theme.bodyColor,
                .paragraphStyle: theme.paragraphStyle(code: false),
            ]
        )
    }

    private static func splitMarkdownAndCodeBlocks(in docs: String) -> [MarkdownChunk] {
        let lines = docs.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)

        var chunks: [MarkdownChunk] = []
        var markdownLines: [String] = []
        var codeLines: [String] = []
        var inCodeBlock = false
        var codeLanguage: String? = nil

        for rawLine in lines {
            let trimmed = rawLine.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("```") {
                if inCodeBlock {
                    chunks.append(.code(language: codeLanguage, text: codeLines.joined(separator: "\n")))
                    codeLines.removeAll(keepingCapacity: true)
                    codeLanguage = nil
                    inCodeBlock = false
                } else {
                    if !markdownLines.isEmpty {
                        chunks.append(.markdown(markdownLines.joined(separator: "\n")))
                        markdownLines.removeAll(keepingCapacity: true)
                    }
                    inCodeBlock = true
                    codeLanguage = parseFenceLanguage(trimmed)
                }
                continue
            }

            if inCodeBlock {
                codeLines.append(rawLine.replacingOccurrences(of: "\t", with: "  "))
            } else {
                markdownLines.append(rawLine)
            }
        }

        if inCodeBlock {
            chunks.append(.code(language: codeLanguage, text: codeLines.joined(separator: "\n")))
        }

        if !markdownLines.isEmpty {
            chunks.append(.markdown(markdownLines.joined(separator: "\n")))
        }

        if chunks.isEmpty {
            chunks.append(.markdown(docs))
        }
        return chunks
    }

    private static func parseFenceLanguage(_ trimmedLine: String) -> String? {
        guard trimmedLine.hasPrefix("```") else {
            return nil
        }

        let token = trimmedLine
            .dropFirst(3)
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .split { ch in
                ch.isWhitespace || ch == "," || ch == "{" || ch == "}"
            }
            .first
            .map(String.init)?
            .trimmingCharacters(in: CharacterSet(charactersIn: "."))
            .lowercased()

        guard let token, !token.isEmpty else {
            return nil
        }
        return token
    }

    private static func renderMarkdown(_ markdown: String, theme: CompletionDocsTheme) -> NSAttributedString {
        let mutable: NSMutableAttributedString
        do {
            let parsed = try AttributedString(
                markdown: markdown,
                options: AttributedString.MarkdownParsingOptions(
                    interpretedSyntax: .full,
                    failurePolicy: .returnPartiallyParsedIfPossible
                )
            )
            mutable = NSMutableAttributedString(attributedString: NSAttributedString(parsed))
        } catch {
            mutable = NSMutableAttributedString(string: markdown)
        }

        guard mutable.length > 0 else {
            return mutable
        }

        let fullRange = NSRange(location: 0, length: mutable.length)
        mutable.addAttribute(.paragraphStyle, value: theme.paragraphStyle(code: false), range: fullRange)
        mutable.enumerateAttribute(.font, in: fullRange) { value, range, _ in
            let font = value as? NSFont
            let name = font?.fontName.lowercased() ?? ""
            let isMonospaced = font?.fontDescriptor.symbolicTraits.contains(.monoSpace) == true || name.contains("mono")
            let converted = isMonospaced
                ? theme.monospacedVariant(for: font)
                : theme.proportionalVariant(for: font)
            mutable.addAttribute(.font, value: converted, range: range)
            mutable.addAttribute(.paragraphStyle, value: theme.paragraphStyle(code: isMonospaced), range: range)
            if !isMonospaced && converted.pointSize > theme.baseFont.pointSize {
                mutable.addAttribute(.foregroundColor, value: theme.headingColor, range: range)
            }
        }
        mutable.enumerateAttribute(.foregroundColor, in: fullRange) { value, range, _ in
            if value == nil {
                mutable.addAttribute(.foregroundColor, value: theme.bodyColor, range: range)
            }
        }
        mutable.enumerateAttribute(.link, in: fullRange) { value, range, _ in
            guard value != nil else {
                return
            }
            mutable.addAttribute(.foregroundColor, value: theme.linkColor, range: range)
            mutable.addAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue, range: range)
        }

        return mutable
    }

    private static func renderCodeBlock(_ code: String, language: String?, theme: CompletionDocsTheme) -> NSAttributedString {
        let normalizedLanguage = normalizeLanguage(language)
        let lines = code.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        let mutable = NSMutableAttributedString()

        if lines.isEmpty {
            return mutable
        }

        for (index, line) in lines.enumerated() {
            let tokens = highlightCodeLine(line, language: normalizedLanguage)
            if tokens.isEmpty {
                mutable.append(NSAttributedString(string: "", attributes: codeAttributes(for: .code, theme: theme)))
            } else {
                for token in tokens {
                    mutable.append(NSAttributedString(string: token.text, attributes: codeAttributes(for: token.kind, theme: theme)))
                }
            }
            if index + 1 < lines.count {
                mutable.append(NSAttributedString(string: "\n", attributes: codeAttributes(for: .code, theme: theme)))
            }
        }

        if mutable.length > 0 {
            let fullRange = NSRange(location: 0, length: mutable.length)
            mutable.addAttribute(.paragraphStyle, value: theme.paragraphStyle(code: true), range: fullRange)
        }
        return mutable
    }

    private static func normalizeLanguage(_ language: String?) -> String? {
        guard let language else {
            return nil
        }
        switch language.lowercased() {
        case "rs", "rust":
            return "rust"
        case "js", "javascript":
            return "javascript"
        case "ts", "typescript":
            return "typescript"
        case "py", "python":
            return "python"
        case "sh", "bash", "shell", "zsh":
            return "shell"
        default:
            return language.lowercased()
        }
    }

    private static func highlightCodeLine(_ line: String, language: String?) -> [CodeToken] {
        guard !line.isEmpty else {
            return []
        }

        let chars = Array(line)
        let keywords = keywordSet(for: language)
        let commentMarker = commentPrefix(for: language).map(Array.init)
        var tokens: [CodeToken] = []
        var idx = 0

        while idx < chars.count {
            if let commentMarker,
               hasPrefix(chars, at: idx, prefix: commentMarker) {
                appendToken(&tokens, text: String(chars[idx...]), kind: .comment)
                break
            }

            let ch = chars[idx]

            if ch == "\"" || ch == "'" {
                let quote = ch
                var end = idx + 1
                var escaped = false
                while end < chars.count {
                    let candidate = chars[end]
                    if candidate == "\\" && !escaped {
                        escaped = true
                        end += 1
                        continue
                    }
                    if candidate == quote && !escaped {
                        end += 1
                        break
                    }
                    escaped = false
                    end += 1
                }
                appendToken(&tokens, text: String(chars[idx..<min(end, chars.count)]), kind: .string)
                idx = min(end, chars.count)
                continue
            }

            if ch.isNumber {
                var end = idx + 1
                while end < chars.count && (chars[end].isNumber || chars[end] == "_") {
                    end += 1
                }
                appendToken(&tokens, text: String(chars[idx..<end]), kind: .number)
                idx = end
                continue
            }

            if ch.isLetter || ch == "_" {
                var end = idx + 1
                while end < chars.count && (chars[end].isLetter || chars[end].isNumber || chars[end] == "_") {
                    end += 1
                }
                let token = String(chars[idx..<end])
                if keywords.contains(token) {
                    appendToken(&tokens, text: token, kind: .keyword)
                } else if token.first?.isUppercase == true {
                    appendToken(&tokens, text: token, kind: .typeName)
                } else {
                    appendToken(&tokens, text: token, kind: .code)
                }
                idx = end
                continue
            }

            appendToken(&tokens, text: String(ch), kind: .code)
            idx += 1
        }

        return tokens
    }

    private static func hasPrefix(_ chars: [Character], at index: Int, prefix: [Character]) -> Bool {
        guard index + prefix.count <= chars.count else {
            return false
        }
        for offset in 0..<prefix.count where chars[index + offset] != prefix[offset] {
            return false
        }
        return true
    }

    private static func appendToken(_ tokens: inout [CodeToken], text: String, kind: CodeTokenKind) {
        guard !text.isEmpty else {
            return
        }
        if let last = tokens.last, last.kind == kind {
            tokens[tokens.count - 1].text += text
            return
        }
        tokens.append(CodeToken(text: text, kind: kind))
    }

    private static func keywordSet(for language: String?) -> Set<String> {
        switch language {
        case "rust":
            return [
                "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
                "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop",
                "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static",
                "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while"
            ]
        case "swift":
            return [
                "actor", "as", "associatedtype", "async", "await", "break", "case", "catch",
                "class", "continue", "default", "defer", "do", "else", "enum", "extension",
                "false", "for", "func", "guard", "if", "import", "in", "init", "inout", "is",
                "let", "nil", "protocol", "repeat", "return", "self", "static", "struct", "subscript",
                "super", "switch", "throw", "throws", "true", "try", "typealias", "var", "where", "while"
            ]
        default:
            return [
                "as", "break", "case", "class", "const", "continue", "def", "do", "else", "enum",
                "export", "extends", "false", "fn", "for", "func", "function", "if", "import", "in",
                "interface", "let", "match", "mut", "nil", "private", "protected", "pub", "public",
                "return", "self", "static", "struct", "switch", "this", "trait", "true", "type",
                "var", "while"
            ]
        }
    }

    private static func commentPrefix(for language: String?) -> String? {
        switch language {
        case "python", "shell", "yaml", "toml", "ruby":
            return "#"
        case "rust", "javascript", "typescript", "swift", "java", "kotlin", "go", "c", "cpp", "c++":
            return "//"
        default:
            return "//"
        }
    }

    private static func baseAttributes(theme: CompletionDocsTheme) -> [NSAttributedString.Key: Any] {
        [
            .font: theme.baseFont,
            .foregroundColor: theme.bodyColor,
            .paragraphStyle: theme.paragraphStyle(code: false),
        ]
    }

    private static func codeAttributes(for kind: CodeTokenKind, theme: CompletionDocsTheme) -> [NSAttributedString.Key: Any] {
        let color: NSColor
        switch kind {
        case .code:
            color = theme.codeColor
        case .keyword:
            color = theme.keywordColor
        case .typeName:
            color = theme.typeColor
        case .number:
            color = theme.numberColor
        case .string:
            color = theme.stringColor
        case .comment:
            color = theme.commentColor
        }

        return [
            .font: theme.codeFont,
            .foregroundColor: color,
            .paragraphStyle: theme.paragraphStyle(code: true),
        ]
    }
}

// MARK: - Glass background modifier

private struct GlassBackgroundModifier: ViewModifier {
    let cornerRadius: CGFloat
    private var tint: Color { Color(nsColor: .windowBackgroundColor) }

    @ViewBuilder
    func body(content: Content) -> some View {
        if CompletionPopupRenderConfig.lightweightStyle {
            content
                .background(
                    RoundedRectangle(cornerRadius: cornerRadius)
                        .fill(tint.opacity(0.98))
                )
                .clipShape(RoundedRectangle(cornerRadius: cornerRadius))
                .overlay(
                    RoundedRectangle(cornerRadius: cornerRadius)
                        .stroke(Color(nsColor: .tertiaryLabelColor).opacity(0.35), lineWidth: 0.5)
                )
        } else {
            content
                .background(
                    ZStack {
                        RoundedRectangle(cornerRadius: cornerRadius)
                            .fill(.ultraThinMaterial)
                        RoundedRectangle(cornerRadius: cornerRadius)
                            .fill(tint)
                            .blendMode(.color)
                    }
                    .compositingGroup()
                )
                .clipShape(RoundedRectangle(cornerRadius: cornerRadius))
                .overlay(
                    RoundedRectangle(cornerRadius: cornerRadius)
                        .stroke(Color(nsColor: .tertiaryLabelColor).opacity(0.5), lineWidth: 0.5)
                )
                .shadow(color: Color.black.opacity(0.25), radius: 16, x: 0, y: 6)
        }
    }
}

private enum CompletionPopupRenderConfig {
    static let lightweightStyle: Bool = {
        ProcessInfo.processInfo.environment["THE_SWIFT_LIGHTWEIGHT_COMPLETION_POPUP"] == "1"
    }()

    static let disableDocs: Bool = {
        ProcessInfo.processInfo.environment["THE_SWIFT_DISABLE_COMPLETION_DOCS"] == "1"
    }()
}

extension View {
    fileprivate func glassBackground(cornerRadius: CGFloat) -> some View {
        modifier(GlassBackgroundModifier(cornerRadius: cornerRadius))
    }
}
