import SwiftUI
import class TheEditorFFIBridge.PreviewData
import class TheEditorFFIBridge.PreviewLine
import class TheEditorFFIBridge.FilePickerSnapshotData
import class TheEditorFFIBridge.FilePickerItemFFI

// MARK: - Data model

struct FilePickerSnapshot {
    let active: Bool
    let query: String
    let matchedCount: Int
    let totalCount: Int
    let scanning: Bool
    let root: String
    let items: [FilePickerItemSnapshot]
}

// MARK: - Preview model (isolated from main EditorModel to avoid re-rendering the file list)

class FilePickerPreviewModel: ObservableObject {
    @Published var preview: PreviewData? = nil
}

struct FilePickerItemSnapshot: Identifiable {
    let id: Int
    let display: String
    let isDir: Bool
    let icon: String?
    let matchIndices: [Int]

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
    let previewModel: FilePickerPreviewModel
    let onQueryChange: (String) -> Void
    let onSubmit: (Int) -> Void
    let onClose: () -> Void
    let onSelectionChange: ((Int) -> Void)?
    let colorForHighlight: ((UInt32) -> SwiftUI.Color?)?

    private var items: [FilePickerItemSnapshot] {
        snapshot.items
    }

    private var matchedCount: Int {
        snapshot.matchedCount
    }

    private var totalCount: Int {
        snapshot.totalCount
    }

    private var isScanning: Bool {
        snapshot.scanning
    }

    // Always show preview layout — the preview panel handles its own empty state.
    // This avoids re-evaluating the body when preview data arrives.
    private let hasPreview = true

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
                externalQuery: snapshot.query,
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
                    previewModel: previewModel,
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
        .background(EscapeInterceptor(onEscape: onClose).frame(width: 0, height: 0))
    }

    // MARK: - Status text

    @ViewBuilder
    private var statusText: some View {
        if items.isEmpty && !snapshot.query.isEmpty && !isScanning {
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

// MARK: - Escape interceptor

/// Installs a local NSEvent monitor that catches Escape key presses *before*
/// SwiftUI's `.textSelection(.enabled)` can consume them to deselect text.
private struct EscapeInterceptor: NSViewRepresentable {
    let onEscape: () -> Void

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        let coordinator = context.coordinator
        coordinator.isActive = true
        coordinator.monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            guard coordinator.isActive else { return event }
            if event.keyCode == 53 { // Escape
                coordinator.onEscape()
                return nil // swallow
            }
            return event
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        context.coordinator.onEscape = onEscape
    }

    static func dismantleNSView(_ nsView: NSView, coordinator: Coordinator) {
        coordinator.isActive = false
        if let monitor = coordinator.monitor {
            NSEvent.removeMonitor(monitor)
            coordinator.monitor = nil
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator(onEscape: onEscape) }

    class Coordinator {
        var onEscape: () -> Void
        var monitor: Any?
        var isActive: Bool = false

        init(onEscape: @escaping () -> Void) {
            self.onEscape = onEscape
        }

        deinit {
            if let monitor { NSEvent.removeMonitor(monitor) }
        }
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
    @ObservedObject var previewModel: FilePickerPreviewModel
    let colorForHighlight: ((UInt32) -> SwiftUI.Color?)?

    private var preview: PreviewData? { previewModel.preview }

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
        let path = preview?.path().toString() ?? ""
        HStack(spacing: 6) {
            Image(systemName: "doc.text")
                .font(FontLoader.uiFont(size: 12).weight(.medium))
                .foregroundStyle(.tertiary)

            if !path.isEmpty {
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

            if preview?.truncated() == true {
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
        let kind = preview?.kind() ?? 0
        switch kind {
        case 1: sourcePreview
        case 2: textPreview(preview?.text().toString() ?? "")
        case 3: messagePreview(preview?.text().toString() ?? "")
        default: emptyPreview
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
                .font(FontLoader.bufferFont(size: 11))
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(12)
                .textSelection(.enabled)
        }
    }

    // MARK: - Source preview

    @ViewBuilder
    private var sourcePreview: some View {
        let lineCount = Int(preview?.line_count() ?? 0)
        if lineCount > 0 {
            let gutterWidth = max(2, String(lineCount).count)
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(0..<lineCount, id: \.self) { index in
                        let line = preview!.line_at(UInt(index))
                        sourceLineView(
                            line: line,
                            lineNumber: index + 1,
                            gutterWidth: gutterWidth
                        )
                    }
                }
                .padding(.vertical, 4)
                .textSelection(.enabled)
            }
        } else {
            emptyPreview
        }
    }

    private func sourceLineView(
        line: PreviewLine,
        lineNumber: Int,
        gutterWidth: Int
    ) -> some View {
        HStack(alignment: .top, spacing: 0) {
            Text(String(format: "%\(gutterWidth)d", lineNumber))
                .font(FontLoader.bufferFont(size: 11))
                .foregroundStyle(.tertiary)
                .frame(minWidth: CGFloat(gutterWidth) * 7 + 8, alignment: .trailing)
                .padding(.trailing, 6)

            highlightedSourceLine(line)
                .font(FontLoader.bufferFont(size: 11))
                .lineLimit(1)
        }
        .padding(.horizontal, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .frame(height: 16)
    }

    private func highlightedSourceLine(_ line: PreviewLine) -> Text {
        let text = line.text().toString()
        guard !text.isEmpty else {
            return Text(" ")
        }

        let chars = Array(text)
        let spanCount = Int(line.span_count())

        if spanCount == 0 {
            return Text(text)
                .foregroundColor(.primary.opacity(0.75))
        }

        // Build a highlight-ID array for each character
        var charHighlights: [UInt32?] = Array(repeating: nil, count: chars.count)
        for i in 0..<spanCount {
            let start = max(0, min(Int(line.span_char_start(UInt(i))), chars.count))
            let end = max(0, min(Int(line.span_char_end(UInt(i))), chars.count))
            guard start < end else { continue }
            let hlId = line.span_highlight(UInt(i))
            for j in start..<end {
                charHighlights[j] = hlId
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
