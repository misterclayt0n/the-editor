import SwiftUI
import class TheEditorFFIBridge.PreviewData
import class TheEditorFFIBridge.PreviewLine
import class TheEditorFFIBridge.FilePickerSnapshotData
import class TheEditorFFIBridge.FilePickerItemFFI

// MARK: - Data model

struct FilePickerSnapshot {
    let active: Bool
    let title: String
    let pickerKind: UInt8
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
    let rowKind: UInt8
    let severity: UInt8
    let primary: String
    let secondary: String
    let tertiary: String
    let quaternary: String
    let line: Int
    let column: Int
    let depth: Int

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

fileprivate enum DiagnosticsSeverity {
    case none
    case error
    case warning
    case info
    case hint

    var defaultLabel: String {
        switch self {
        case .none: return "DIAG"
        case .error: return "ERROR"
        case .warning: return "WARN"
        case .info: return "INFO"
        case .hint: return "HINT"
        }
    }

    var symbol: String {
        switch self {
        case .none: return "circle.fill"
        case .error: return "xmark.octagon.fill"
        case .warning: return "exclamationmark.triangle.fill"
        case .info: return "info.circle.fill"
        case .hint: return "lightbulb.fill"
        }
    }

    var accent: Color {
        switch self {
        case .none: return .secondary
        case .error: return Color(nsColor: .systemRed)
        case .warning: return Color(nsColor: .systemOrange)
        case .info: return Color(nsColor: .systemBlue)
        case .hint: return Color(nsColor: .systemTeal)
        }
    }
}

fileprivate func diagnosticsSeverity(from value: UInt8) -> DiagnosticsSeverity {
    switch value {
    case 1: return .error
    case 2: return .warning
    case 3: return .info
    case 4: return .hint
    default: return .none
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
    let onPreviewWindowRequest: ((Int, Int, Int) -> Void)?
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

    private var isDiagnosticsPicker: Bool {
        snapshot.pickerKind == 1
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
                placeholder: isDiagnosticsPicker ? "Filter diagnostics…" : "Open file…",
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
                    let item = items[index]
                    let nextItem: FilePickerItemSnapshot? = (index + 1) < items.count ? items[index + 1] : nil
                    switch item.rowKind {
                    case 1:
                        return AnyView(diagnosticsRowContent(for: item, isSelected: isSelected))
                    case 2:
                        return AnyView(symbolsRowContent(for: item, nextItem: nextItem, isSelected: isSelected))
                    case 3, 4:
                        return AnyView(liveGrepRowContent(for: item, isSelected: isSelected))
                    default:
                        return AnyView(fileRowContent(for: item, isSelected: isSelected))
                    }
                },
                emptyContent: {
                    VStack(spacing: 8) {
                        Image(systemName: "doc.questionmark")
                            .font(FontLoader.uiFont(size: 24))
                            .foregroundStyle(.tertiary)
                        Text(isDiagnosticsPicker ? "No matching diagnostics" : "No matching files")
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
                    onWindowRequest: onPreviewWindowRequest,
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
                    Text(isDiagnosticsPicker ? "\(matchedCount) of \(totalCount) diagnostics" : "\(matchedCount) of \(totalCount)")
                        .font(FontLoader.uiFont(size: 12))
                        .foregroundStyle(.tertiary)
                } else if totalCount > 0 {
                    Text(isDiagnosticsPicker ? "\(totalCount) diagnostics" : "\(totalCount) files")
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

    private func diagnosticsRowContent(for item: FilePickerItemSnapshot, isSelected: Bool) -> some View {
        let severity = diagnosticsSeverity(from: item.severity)
        let severityLabel = severity.defaultLabel
        let message = item.primary.isEmpty ? item.display : item.primary
        let source = item.secondary == "-" ? "" : item.secondary
        let code = item.tertiary == "-" ? "" : item.tertiary
        let location = item.quaternary

        return VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                Image(systemName: severity.symbol)
                    .font(FontLoader.uiFont(size: 11).weight(.semibold))
                    .foregroundStyle(severity.accent)

                Text(severityLabel)
                    .font(FontLoader.uiFont(size: 11).weight(.bold))
                    .padding(.horizontal, 7)
                    .padding(.vertical, 2)
                    .background(
                        Capsule()
                            .fill(severity.accent.opacity(isSelected ? 0.22 : 0.14))
                    )
                    .foregroundStyle(isSelected ? .primary : severity.accent)

                if !source.isEmpty {
                    Text(source)
                        .font(FontLoader.uiFont(size: 11).weight(.medium))
                        .foregroundStyle(.secondary)
                }

                if !code.isEmpty {
                    Text(code)
                        .font(FontLoader.uiFont(size: 11).weight(.semibold))
                        .foregroundStyle(.secondary)
                }

                if !location.isEmpty {
                    Text(location)
                        .font(FontLoader.uiFont(size: 11))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }

                Spacer(minLength: 0)
            }

            Text(message)
                .font(FontLoader.uiFont(size: 13).weight(.medium))
                .foregroundStyle(.primary)
                .lineLimit(2)
                .multilineTextAlignment(.leading)
                .truncationMode(.tail)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func symbolsRowContent(
        for item: FilePickerItemSnapshot,
        nextItem: FilePickerItemSnapshot?,
        isSelected: Bool
    ) -> some View {
        let depth = max(0, item.depth)
        let nextDepth = max(0, nextItem?.depth ?? 0)
        let kind = item.quaternary.uppercased()
        let symbol = symbolIcon(forKind: kind)
        let kindColor = symbolKindColor(forKind: kind)
        let treePrefix = symbolTreePrefix(depth: depth, nextDepth: nextDepth)
        let marker = isSelected ? "▌" : "▏"
        let location = item.line > 0 ? "L\(item.line):\(max(1, item.column))" : ""
        let name = item.primary.isEmpty ? "<unnamed>" : item.primary
        let suffix = symbolSuffix(detail: item.tertiary, container: item.secondary)
        let nameMatchIndices = symbolNameMatchIndices(for: item, name: name)

        return HStack(alignment: .firstTextBaseline, spacing: 6) {
            Text(marker)
                .font(FontLoader.bufferFont(size: 12).weight(.semibold))
                .foregroundStyle(isSelected ? kindColor : Color.secondary)
                .frame(width: 8, alignment: .leading)

            if !treePrefix.isEmpty {
                Text(treePrefix)
                    .font(FontLoader.bufferFont(size: 12))
                    .foregroundStyle(.tertiary)
                    .fixedSize()
            }

            Image(systemName: symbol)
                .font(FontLoader.uiFont(size: 11).weight(.semibold))
                .foregroundStyle(kindColor)
                .frame(width: 14, alignment: .center)

            HStack(alignment: .firstTextBaseline, spacing: 0) {
                highlightedText(
                    name,
                    matchIndices: nameMatchIndices,
                    baseColor: .primary,
                    highlightColor: isSelected ? .primary : .accentColor,
                    fontSize: 13,
                    baseWeight: .semibold
                )
                .lineLimit(1)

                if !suffix.isEmpty {
                    Text(suffix)
                        .font(FontLoader.uiFont(size: 12))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.tail)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            HStack(spacing: 6) {
                if !kind.isEmpty {
                    Text(symbolKindLabel(for: kind))
                        .font(FontLoader.uiFont(size: 10).weight(.semibold))
                        .foregroundStyle(isSelected ? Color.primary.opacity(0.9) : kindColor.opacity(0.95))
                        .lineLimit(1)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(
                            Capsule()
                                .fill(kindColor.opacity(isSelected ? 0.22 : 0.14))
                        )
                }

                if !location.isEmpty {
                    Text(location)
                        .font(FontLoader.bufferFont(size: 11))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
            }
            .fixedSize()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func liveGrepRowContent(for item: FilePickerItemSnapshot, isSelected: Bool) -> some View {
        if item.rowKind == 3 {
            return AnyView(
                HStack(spacing: 8) {
                    Image(systemName: "doc.text.fill")
                        .font(FontLoader.uiFont(size: 12).weight(.semibold))
                        .foregroundStyle(.secondary)
                    Text(item.primary)
                        .font(FontLoader.uiFont(size: 12).weight(.semibold))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Spacer(minLength: 0)
                }
            )
        }

        return AnyView(
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(":\(max(1, item.line)):\(max(1, item.column))")
                    .font(FontLoader.uiFont(size: 11))
                    .foregroundStyle(.secondary)
                    .frame(width: 64, alignment: .leading)

                VStack(alignment: .leading, spacing: 2) {
                    Text(item.primary)
                        .font(FontLoader.uiFont(size: 13).weight(.medium))
                        .foregroundStyle(.primary)
                        .lineLimit(1)
                    if !item.secondary.isEmpty {
                        Text(item.secondary)
                            .font(FontLoader.uiFont(size: 11))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                }
                Spacer(minLength: 0)
            }
        )
    }

    private func symbolIcon(forKind kind: String) -> String {
        switch kind {
        case "FILE": return "doc.fill"
        case "MODULE", "NAMESPACE", "PACKAGE": return "shippingbox.fill"
        case "CLASS", "STRUCT": return "square.grid.2x2.fill"
        case "INTERFACE": return "rectangle.3.group.fill"
        case "METHOD", "FUNCTION", "CONSTRUCTOR": return "function"
        case "PROPERTY", "FIELD": return "text.alignleft"
        case "ENUM", "ENUM_MEMBER": return "list.bullet.rectangle"
        case "VARIABLE", "CONSTANT": return "number"
        default: return "circle.fill"
        }
    }

    private func symbolKindColor(forKind kind: String) -> Color {
        switch kind {
        case "METHOD", "FUNCTION", "CONSTRUCTOR", "OPERATOR":
            return Color(red: 0xDB / 255.0, green: 0xBF / 255.0, blue: 0xEF / 255.0)
        case "FIELD", "VARIABLE", "PROPERTY", "VALUE", "REFERENCE":
            return Color(red: 0xA4 / 255.0, green: 0xA0 / 255.0, blue: 0xE8 / 255.0)
        case "CLASS", "INTERFACE", "ENUM", "STRUCT", "TYPE_PARAM":
            return Color(red: 0xEF / 255.0, green: 0xBA / 255.0, blue: 0x5D / 255.0)
        case "MODULE", "NAMESPACE", "PACKAGE", "FILE", "ENUM_MEMBER", "CONSTANT":
            return Color(red: 0xE8 / 255.0, green: 0xDC / 255.0, blue: 0xA0 / 255.0)
        case "EVENT":
            return Color(red: 0xF4 / 255.0, green: 0x78 / 255.0, blue: 0x68 / 255.0)
        default:
            return .secondary
        }
    }

    private func symbolTreePrefix(depth: Int, nextDepth: Int) -> String {
        if depth == 0 {
            return ""
        }
        var prefix = ""
        for _ in 0..<max(0, depth - 1) {
            prefix += "│ "
        }
        prefix += nextDepth > depth ? "├ " : "└ "
        return prefix
    }

    private func symbolSuffix(detail: String, container: String) -> String {
        var suffix = ""
        if !detail.isEmpty {
            suffix += "  " + detail
        }
        if !container.isEmpty {
            if suffix.isEmpty {
                suffix += "  " + container
            } else {
                suffix += "  · " + container
            }
        }
        return suffix
    }

    private func symbolKindLabel(for kind: String) -> String {
        let upper = kind.uppercased()
        if upper.isEmpty {
            return "SYMBOL"
        }
        return upper.replacingOccurrences(of: "_", with: " ")
    }

    private func symbolNameMatchIndices(for item: FilePickerItemSnapshot, name: String) -> [Int] {
        let nameLength = name.count
        guard nameLength > 0 else { return [] }
        return item.matchIndices.filter { (0..<nameLength).contains($0) }
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
    let onWindowRequest: ((Int, Int, Int) -> Void)?
    let colorForHighlight: ((UInt32) -> SwiftUI.Color?)?

    private var preview: PreviewData? { previewModel.preview }
    private let rowHeight: CGFloat = 16
    private let defaultVisibleRows: Int = 24
    private let overscanRows: Int = 24

    @State private var contentMinY: CGFloat = 0
    @State private var lastRequestedOffset: Int = Int.min
    @State private var lastRequestedVisibleRows: Int = 0
    @State private var lastRequestedOverscan: Int = 0
    @State private var pendingFocusReset: Bool = true

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            previewHeader
            Divider()
            previewContent
        }
        .onAppear {
            pendingFocusReset = true
            requestWindow()
        }
        .onChange(of: preview?.path().toString() ?? "") { _ in
            pendingFocusReset = true
            requestWindow()
        }
        .onChange(of: preview?.kind() ?? 0) { _ in
            pendingFocusReset = true
            requestWindow()
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
        case 1:
            windowedPreview(showLineNumbers: true)
        case 2, 3:
            windowedPreview(showLineNumbers: false)
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

    private func windowedPreview(showLineNumbers: Bool) -> some View {
        let lineCount = Int(preview?.line_count() ?? 0)
        let totalRows = Int(preview?.total_lines() ?? 0)
        let windowStart = Int(preview?.window_start() ?? 0)
        let topRows = max(0, windowStart)
        let bottomRows = max(0, totalRows - windowStart - lineCount)

        return ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if topRows > 0 {
                        Color.clear
                            .frame(height: CGFloat(topRows) * rowHeight)
                    }

                    ForEach(0..<lineCount, id: \.self) { index in
                        let line = preview!.line_at(UInt(index))
                        previewLineView(line: line, showLineNumbers: showLineNumbers, totalRows: totalRows)
                            .id(previewRowId(Int(line.virtual_row())))
                    }

                    if bottomRows > 0 {
                        Color.clear
                            .frame(height: CGFloat(bottomRows) * rowHeight)
                    }
                }
                .background(
                    GeometryReader { proxy in
                        Color.clear.preference(
                            key: PreviewContentOffsetKey.self,
                            value: proxy.frame(in: .named("file-picker-preview-scroll")).minY
                        )
                    }
                )
            }
            .coordinateSpace(name: "file-picker-preview-scroll")
            .onPreferenceChange(PreviewContentOffsetKey.self) { value in
                contentMinY = value
                requestWindow()
            }
            .onAppear {
                syncScrollPosition(proxy)
            }
            .onChange(of: preview?.offset() ?? 0) { _ in
                syncScrollPosition(proxy)
            }
            .onChange(of: preview?.window_start() ?? 0) { _ in
                syncScrollPosition(proxy)
            }
            .onChange(of: preview?.line_count() ?? 0) { _ in
                syncScrollPosition(proxy)
            }
            .padding(.vertical, 4)
            .padding(.horizontal, 8)
        }
    }

    private func previewLineView(line: PreviewLine, showLineNumbers: Bool, totalRows: Int) -> some View {
        let lineKind = Int(line.kind())
        if lineKind == 1 || lineKind == 2 {
            return AnyView(
                Text(line.marker().toString())
                    .font(FontLoader.bufferFont(size: 11))
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .frame(height: rowHeight)
            )
        }

        let focused = line.focused()
        let lineNumber = Int(line.line_number())
        let lineNumberWidth = max(3, String(max(1, totalRows)).count)
        return AnyView(
            HStack(alignment: .top, spacing: 0) {
                if showLineNumbers {
                    let marker = focused ? "▶" : " "
                    Text("\(marker)\(String(format: "%\(lineNumberWidth)d", lineNumber)) ")
                        .font(FontLoader.bufferFont(size: 11))
                        .foregroundStyle(focused ? .primary : .tertiary)
                        .frame(minWidth: CGFloat(lineNumberWidth) * 7 + 14, alignment: .trailing)
                        .padding(.trailing, 6)
                }

                previewSegmentsText(line)
                    .font(FontLoader.bufferFont(size: 11))
                    .lineLimit(1)

                Spacer(minLength: 0)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .frame(height: rowHeight)
            .background(focused ? Color.accentColor.opacity(0.12) : Color.clear)
        )
    }

    private func previewSegmentsText(_ line: PreviewLine) -> Text {
        let segmentCount = Int(line.segment_count())
        guard segmentCount > 0 else {
            return Text(" ")
        }

        var output = Text("")
        for index in 0..<segmentCount {
            let segment = line.segment_at(UInt(index))
            let text = segment.text().toString()
            let highlightId = segment.highlight_id()
            let isMatch = segment.is_match()

            let baseColor: Color
            if highlightId > 0, let mapped = colorForHighlight?(highlightId) {
                baseColor = mapped
            } else {
                baseColor = .primary.opacity(0.75)
            }

            let color = isMatch ? Color.accentColor : baseColor
            output = output + Text(text)
                .foregroundColor(color)
                .font(FontLoader.bufferFont(size: 11).weight(isMatch ? .bold : .regular))
        }
        return output
    }

    private func requestWindow() {
        guard let onWindowRequest else { return }
        let visibleRows = defaultVisibleRows

        let rawOffset = max(0, Int(floor(-contentMinY / rowHeight)))
        let nextOffset = pendingFocusReset ? -1 : rawOffset
        let overscan = overscanRows
        if nextOffset == lastRequestedOffset
            && visibleRows == lastRequestedVisibleRows
            && overscan == lastRequestedOverscan {
            return
        }

        lastRequestedOffset = nextOffset
        lastRequestedVisibleRows = visibleRows
        lastRequestedOverscan = overscan
        pendingFocusReset = false
        if DiagnosticsDebugLog.enabled {
            DiagnosticsDebugLog.log(
                "picker.window_calc contentMinY=\(String(format: "%.1f", contentMinY)) send_offset=\(nextOffset) send_visible=\(visibleRows) send_overscan=\(overscan)"
            )
        }
        onWindowRequest(nextOffset, visibleRows, overscan)
    }

    private func previewRowId(_ virtualRow: Int) -> String {
        "picker-preview-row-\(virtualRow)"
    }

    private func syncScrollPosition(_ proxy: ScrollViewProxy) {
        guard let preview else { return }
        let lineCount = Int(preview.line_count())
        if lineCount == 0 {
            return
        }
        let targetOffset = Int(preview.offset())
        let windowStart = Int(preview.window_start())
        let windowEnd = windowStart + lineCount
        guard (windowStart..<windowEnd).contains(targetOffset) else {
            return
        }
        let targetId = previewRowId(targetOffset)
        DispatchQueue.main.async {
            proxy.scrollTo(targetId, anchor: .top)
        }
    }
}

fileprivate struct PreviewContentOffsetKey: PreferenceKey {
    static var defaultValue: CGFloat = 0

    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = nextValue()
    }
}
