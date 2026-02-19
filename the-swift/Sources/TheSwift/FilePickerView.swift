import SwiftUI

// MARK: - Data model

struct FilePickerSnapshot: Decodable {
    let active: Bool
    let query: String?
    let matchedCount: Int?
    let totalCount: Int?
    let scanning: Bool?
    let root: String?
    let items: [FilePickerItemSnapshot]?
    let preview: FilePickerPreviewSnapshot?
    let showPreview: Bool?

    private enum CodingKeys: String, CodingKey {
        case active, query, scanning, root, items, preview
        case matchedCount = "matched_count"
        case totalCount = "total_count"
        case showPreview = "show_preview"
    }

    init(active: Bool, query: String?, matchedCount: Int?, totalCount: Int?,
         scanning: Bool?, root: String?, items: [FilePickerItemSnapshot]?,
         preview: FilePickerPreviewSnapshot?, showPreview: Bool?) {
        self.active = active
        self.query = query
        self.matchedCount = matchedCount
        self.totalCount = totalCount
        self.scanning = scanning
        self.root = root
        self.items = items
        self.preview = preview
        self.showPreview = showPreview
    }
}

// MARK: - Preview data model

enum FilePickerPreviewKind: String, Decodable {
    case empty, source, text, message
}

struct FilePickerPreviewLineSpan: Decodable {
    let charStart: Int
    let charEnd: Int
    let highlightId: UInt32

    init(from decoder: Decoder) throws {
        var container = try decoder.unkeyedContainer()
        charStart = try container.decode(Int.self)
        charEnd = try container.decode(Int.self)
        highlightId = try container.decode(UInt32.self)
    }
}

struct FilePickerPreviewLine: Decodable {
    let text: String
    let spans: [FilePickerPreviewLineSpan]
}

struct FilePickerPreviewSnapshot: Decodable {
    let kind: FilePickerPreviewKind
    let path: String?
    let text: String?
    let truncated: Bool?
    let totalLines: Int?
    let loading: Bool?
    let lines: [FilePickerPreviewLine]?

    private enum CodingKeys: String, CodingKey {
        case kind, path, text, truncated, loading, lines
        case totalLines = "total_lines"
    }

    static let empty = FilePickerPreviewSnapshot(
        kind: .empty, path: nil, text: nil, truncated: nil,
        totalLines: nil, loading: nil, lines: nil
    )

    init(kind: FilePickerPreviewKind, path: String?, text: String?,
         truncated: Bool?, totalLines: Int?, loading: Bool?,
         lines: [FilePickerPreviewLine]?) {
        self.kind = kind
        self.path = path
        self.text = text
        self.truncated = truncated
        self.totalLines = totalLines
        self.loading = loading
        self.lines = lines
    }
}

struct FilePickerItemSnapshot: Decodable, Identifiable {
    let id: Int
    let display: String
    let isDir: Bool
    let icon: String?
    let matchIndices: [Int]

    private enum CodingKeys: String, CodingKey {
        case display
        case isDir = "is_dir"
        case icon
        case matchIndices = "match_indices"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        display = try container.decode(String.self, forKey: .display)
        isDir = (try? container.decode(Bool.self, forKey: .isDir)) ?? false
        icon = try? container.decode(String.self, forKey: .icon)
        matchIndices = (try? container.decode([Int].self, forKey: .matchIndices)) ?? []
        // id will be set by the parent during array mapping
        id = 0
    }

    init(id: Int, display: String, isDir: Bool, icon: String?, matchIndices: [Int]) {
        self.id = id
        self.display = display
        self.isDir = isDir
        self.icon = icon
        self.matchIndices = matchIndices
    }

    var filename: String {
        (display as NSString).lastPathComponent
    }

    var directory: String {
        let parent = (display as NSString).deletingLastPathComponent
        return parent.isEmpty ? "" : parent
    }

    var fileExtension: String {
        (filename as NSString).pathExtension.lowercased()
    }

    var filenameMatchIndices: [Int] {
        let filenameStart = max(0, display.count - filename.count)
        return matchIndices.compactMap { index in
            let local = index - filenameStart
            return (0..<filename.count).contains(local) ? local : nil
        }
    }

    var directoryMatchIndices: [Int] {
        let count = directory.count
        guard count > 0 else { return [] }
        return matchIndices.compactMap { index in
            (0..<count).contains(index) ? index : nil
        }
    }
}

// MARK: - File type icons

fileprivate func iconToken(for item: FilePickerItemSnapshot) -> String {
    if item.isDir {
        return "folder"
    }
    if let icon = item.icon, !icon.isEmpty {
        return icon
    }
    return "file_generic"
}

fileprivate func fileIcon(for item: FilePickerItemSnapshot) -> (svg: Image?, symbol: String) {
    let token = iconToken(for: item)
    let svg = PickerIconLoader.image(named: token)
    let symbol = filePickerSymbol(for: token) ?? fallbackFileIcon(forExtension: item.fileExtension)
    return (svg, symbol)
}

func filePickerSymbol(for icon: String) -> String? {
    switch icon {
    case "archive":
        return "archivebox.fill"
    case "book", "file_markdown":
        return "doc.richtext.fill"
    case "c", "cpp", "go", "html", "java", "javascript", "json", "kotlin", "python", "sass", "swift", "typescript":
        return "chevron.left.forwardslash.chevron.right"
    case "css":
        return "paintbrush.fill"
    case "database":
        return "cylinder.fill"
    case "docker":
        return "shippingbox.fill"
    case "file_doc":
        return "doc.fill"
    case "file_git":
        return "point.topleft.down.curvedto.point.bottomright.up"
    case "file_lock", "lock":
        return "lock.fill"
    case "file_rust", "rust":
        return "r.square.fill"
    case "file_toml", "toml", "settings":
        return "doc.text.fill"
    case "image":
        return "photo.fill"
    case "nix":
        return "chevron.left.forwardslash.chevron.right"
    case "terminal", "tool_hammer":
        return "terminal.fill"
    case "folder", "folder_open", "folder_search":
        return "folder.fill"
    case "file_generic":
        return "doc.fill"
    default:
        return nil
    }
}

fileprivate func fallbackFileIcon(forExtension ext: String) -> String {
    switch ext {
    case "swift":
        return "chevron.left.forwardslash.chevron.right"
    case "rs":
        return "r.square.fill"
    case "js", "jsx":
        return "chevron.left.forwardslash.chevron.right"
    case "ts", "tsx":
        return "chevron.left.forwardslash.chevron.right"
    case "py":
        return "chevron.left.forwardslash.chevron.right"
    case "rb":
        return "chevron.left.forwardslash.chevron.right"
    case "go":
        return "chevron.left.forwardslash.chevron.right"
    case "md", "markdown":
        return "doc.richtext.fill"
    case "json", "yaml", "yml", "toml":
        return "doc.text.fill"
    case "html", "htm":
        return "chevron.left.forwardslash.chevron.right"
    case "css", "scss", "less":
        return "paintbrush.fill"
    case "png", "jpg", "jpeg", "gif", "svg", "ico", "webp":
        return "photo.fill"
    case "pdf":
        return "doc.fill"
    case "txt", "log":
        return "doc.text"
    case "sh", "bash", "zsh", "fish":
        return "terminal.fill"
    case "c", "h":
        return "chevron.left.forwardslash.chevron.right"
    case "cpp", "cc", "hpp", "cxx":
        return "chevron.left.forwardslash.chevron.right"
    case "java", "kt", "kts":
        return "chevron.left.forwardslash.chevron.right"
    case "xml":
        return "chevron.left.forwardslash.chevron.right"
    case "lock":
        return "lock.fill"
    default:
        return "doc.fill"
    }
}

// MARK: - Backdrop

struct FilePickerBackdrop: View {
    var body: some View {
        Color.black.opacity(0.15)
            .ignoresSafeArea()
    }
}

// MARK: - Main view

struct FilePickerView: View {
    let snapshot: FilePickerSnapshot
    let onQueryChange: (String) -> Void
    let onSubmit: (Int) -> Void
    let onClose: () -> Void
    let onSelectionChange: ((Int) -> Void)?
    let colorForHighlight: ((UInt32) -> SwiftUI.Color?)?

    private var items: [FilePickerItemSnapshot] {
        snapshot.items ?? []
    }

    private var matchedCount: Int {
        snapshot.matchedCount ?? 0
    }

    private var totalCount: Int {
        snapshot.totalCount ?? 0
    }

    private var isScanning: Bool {
        snapshot.scanning ?? false
    }

    private var hasPreview: Bool {
        snapshot.showPreview ?? true
    }

    private static let backgroundColor = Color(nsColor: .windowBackgroundColor)

    var body: some View {
        let pickerWidth: CGFloat = hasPreview ? 400 : 600
        let totalWidth: CGFloat = hasPreview ? 920 : 600

        HStack(spacing: 0) {
            PickerPanel(
                width: pickerWidth,
                maxListHeight: hasPreview ? 440 : 380,
                placeholder: "Open file…",
                fontSize: 16,
                layout: .center,
                pageSize: 12,
                showTabNavigation: true,
                showPageNavigation: true,
                showCtrlCClose: true,
                autoSelectFirstItem: true,
                showBackground: !hasPreview,
                itemCount: items.count,
                externalQuery: snapshot.query ?? "",
                externalSelectedIndex: nil,
                onQueryChange: onQueryChange,
                onSubmit: { index in
                    if let index {
                        onSubmit(index)
                    }
                },
                onClose: onClose,
                onSelectionChange: onSelectionChange,
                leadingHeader: {
                    Image(systemName: "magnifyingglass")
                        .font(FontLoader.uiFont(size: 14).weight(.medium))
                        .foregroundStyle(.secondary)
                },
                trailingHeader: {
                    statusText
                },
                itemContent: { index, isSelected, isHovered in
                    fileRowContent(for: items[index], isSelected: isSelected)
                },
                emptyContent: {
                    VStack(spacing: 8) {
                        Image(systemName: "doc.questionmark")
                            .font(FontLoader.uiFont(size: 24))
                            .foregroundStyle(.tertiary)
                        Text("No matching files")
                            .font(FontLoader.uiFont(size: 14))
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, minHeight: 80)
                    .padding()
                }
            )

            if hasPreview {
                Divider()
                FilePreviewPanel(
                    preview: snapshot.preview ?? .empty,
                    colorForHighlight: colorForHighlight
                )
                .frame(maxWidth: .infinity)
            }
        }
        .frame(maxWidth: totalWidth, maxHeight: hasPreview ? 500 : nil)
        .background(
            ZStack {
                Rectangle()
                    .fill(.ultraThinMaterial)
                Rectangle()
                    .fill(Self.backgroundColor)
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
    }

    // MARK: - Status text

    @ViewBuilder
    private var statusText: some View {
        if items.isEmpty && !(snapshot.query ?? "").isEmpty && !isScanning {
            Text("No matches")
                .font(FontLoader.uiFont(size: 12))
                .foregroundStyle(.tertiary)
        } else if isScanning && matchedCount == 0 {
            ScanningText()
        } else {
            HStack(spacing: 6) {
                if matchedCount > 0 && matchedCount < totalCount {
                    Text("\(matchedCount) of \(totalCount)")
                        .font(FontLoader.uiFont(size: 12))
                        .foregroundStyle(.tertiary)
                } else if totalCount > 0 {
                    Text("\(totalCount) files")
                        .font(FontLoader.uiFont(size: 12))
                        .foregroundStyle(.tertiary)
                }
                if isScanning {
                    ScanningText()
                }
            }
        }
    }

    // MARK: - Row content

    private func fileRowContent(for item: FilePickerItemSnapshot, isSelected: Bool) -> some View {
        let icon = fileIcon(for: item)

        return HStack(spacing: 8) {
            Group {
                if let svg = icon.svg {
                    svg
                        .renderingMode(.template)
                } else {
                    Image(systemName: icon.symbol)
                        .symbolRenderingMode(.monochrome)
                }
            }
            .foregroundStyle(.secondary)
            .font(FontLoader.uiFont(size: 14).weight(.medium))
            .frame(width: 18, alignment: .center)

            VStack(alignment: .leading, spacing: 2) {
                highlightedText(
                    item.filename,
                    matchIndices: item.filenameMatchIndices,
                    baseColor: .primary,
                    highlightColor: .accentColor,
                    fontSize: 14,
                    baseWeight: item.isDir ? .semibold : .medium
                )
                    .lineLimit(1)

                if !item.directory.isEmpty {
                    highlightedText(
                        item.directory,
                        matchIndices: item.directoryMatchIndices,
                        baseColor: .secondary,
                        highlightColor: isSelected ? .primary : .accentColor,
                        fontSize: 12,
                        baseWeight: .regular
                    )
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }

            Spacer()
        }
    }

    private func highlightedText(
        _ text: String,
        matchIndices: [Int],
        baseColor: Color,
        highlightColor: Color,
        fontSize: CGFloat,
        baseWeight: Font.Weight
    ) -> Text {
        guard !text.isEmpty else { return Text("") }
        let indexSet = Set(matchIndices)

        func segmentText(_ segment: String, matched: Bool) -> Text {
            Text(segment)
                .font(FontLoader.uiFont(size: fontSize).weight(matched ? .bold : baseWeight))
                .foregroundColor(matched ? highlightColor : baseColor)
        }

        var result = Text("")
        var current = ""
        var currentMatched: Bool? = nil

        for (index, ch) in text.enumerated() {
            let matched = indexSet.contains(index)
            if currentMatched == nil {
                currentMatched = matched
            }
            if currentMatched != matched {
                result = result + segmentText(current, matched: currentMatched ?? false)
                current = ""
                currentMatched = matched
            }
            current.append(ch)
        }

        if !current.isEmpty {
            result = result + segmentText(current, matched: currentMatched ?? false)
        }

        return result
    }
}

// MARK: - Scanning indicator

fileprivate struct ScanningText: View {
    @State private var opacity: Double = 1.0

    var body: some View {
        Text("Scanning…")
            .font(FontLoader.uiFont(size: 12))
            .foregroundStyle(.tertiary)
            .opacity(opacity)
            .onAppear {
                withAnimation(.easeInOut(duration: 0.8).repeatForever(autoreverses: true)) {
                    opacity = 0.4
                }
            }
    }
}

// MARK: - File preview panel

struct FilePreviewPanel: View {
    let preview: FilePickerPreviewSnapshot
    let colorForHighlight: ((UInt32) -> SwiftUI.Color?)?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            previewHeader
            Divider()
            previewContent
        }
    }

    // MARK: - Header

    @ViewBuilder
    private var previewHeader: some View {
        HStack(spacing: 6) {
            Image(systemName: "doc.text")
                .font(FontLoader.uiFont(size: 12).weight(.medium))
                .foregroundStyle(.tertiary)

            if let path = preview.path, !path.isEmpty {
                Text(path)
                    .font(FontLoader.uiFont(size: 12).weight(.medium))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            } else {
                Text("Preview")
                    .font(FontLoader.uiFont(size: 12).weight(.medium))
                    .foregroundStyle(.tertiary)
            }

            Spacer()

            if preview.truncated == true {
                Text("truncated")
                    .font(FontLoader.uiFont(size: 10))
                    .foregroundStyle(.tertiary)
                    .padding(.horizontal, 4)
                    .padding(.vertical, 1)
                    .background(
                        RoundedRectangle(cornerRadius: 3)
                            .fill(Color.secondary.opacity(0.1))
                    )
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    // MARK: - Content

    @ViewBuilder
    private var previewContent: some View {
        switch preview.kind {
        case .empty:
            emptyPreview
        case .message:
            messagePreview(preview.text ?? "")
        case .text:
            textPreview(preview.text ?? "")
        case .source:
            sourcePreview
        }
    }

    private var emptyPreview: some View {
        VStack(spacing: 8) {
            Image(systemName: "doc")
                .font(FontLoader.uiFont(size: 24))
                .foregroundStyle(.quaternary)
            Text("No preview")
                .font(FontLoader.uiFont(size: 13))
                .foregroundStyle(.tertiary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func messagePreview(_ message: String) -> some View {
        VStack(spacing: 8) {
            Image(systemName: "info.circle")
                .font(FontLoader.uiFont(size: 20))
                .foregroundStyle(.tertiary)
            Text(message)
                .font(FontLoader.uiFont(size: 13))
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func textPreview(_ text: String) -> some View {
        ScrollView {
            Text(text)
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(12)
        }
    }

    // MARK: - Source preview

    @ViewBuilder
    private var sourcePreview: some View {
        if let lines = preview.lines, !lines.isEmpty {
            let gutterWidth = max(2, String(lines.count).count)
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(0..<lines.count, id: \.self) { index in
                        sourceLineView(
                            line: lines[index],
                            lineNumber: index + 1,
                            gutterWidth: gutterWidth
                        )
                    }
                }
                .padding(.vertical, 4)
            }
        } else {
            emptyPreview
        }
    }

    private func sourceLineView(
        line: FilePickerPreviewLine,
        lineNumber: Int,
        gutterWidth: Int
    ) -> some View {
        HStack(alignment: .top, spacing: 0) {
            Text(String(format: "%\(gutterWidth)d", lineNumber))
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(.tertiary)
                .frame(minWidth: CGFloat(gutterWidth) * 7 + 8, alignment: .trailing)
                .padding(.trailing, 6)

            highlightedSourceLine(line)
                .font(.system(size: 11, design: .monospaced))
                .lineLimit(1)
        }
        .padding(.horizontal, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .frame(height: 16)
    }

    private func highlightedSourceLine(_ line: FilePickerPreviewLine) -> Text {
        let text = line.text
        guard !text.isEmpty else {
            return Text(" ")
        }

        let chars = Array(text)
        let spans = line.spans

        if spans.isEmpty {
            return Text(text)
                .foregroundColor(.primary.opacity(0.75))
        }

        // Build a highlight-ID array for each character
        var charHighlights: [UInt32?] = Array(repeating: nil, count: chars.count)
        for span in spans {
            let start = max(0, min(span.charStart, chars.count))
            let end = max(0, min(span.charEnd, chars.count))
            guard start < end else { continue }
            for i in start..<end {
                charHighlights[i] = span.highlightId
            }
        }

        // Group consecutive characters with the same highlight ID
        var result = Text("")
        var segmentStart = 0

        while segmentStart < chars.count {
            let currentHL = charHighlights[segmentStart]
            var segmentEnd = segmentStart + 1
            while segmentEnd < chars.count && charHighlights[segmentEnd] == currentHL {
                segmentEnd += 1
            }

            let segment = String(chars[segmentStart..<segmentEnd])
            let color: SwiftUI.Color
            if let hlId = currentHL, let resolve = colorForHighlight?(hlId) {
                color = resolve
            } else {
                color = .primary.opacity(0.75)
            }
            result = result + Text(segment).foregroundColor(color)
            segmentStart = segmentEnd
        }

        return result
    }
}
