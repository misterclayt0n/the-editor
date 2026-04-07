import AppKit
import SwiftUI

struct EditorChromeModel {
    let document: EditorDocumentChrome
    let statusBar: EditorStatusBarState
    let backgroundColor: NSColor

    static let empty = EditorChromeModel(
        document: .empty,
        statusBar: .empty,
        backgroundColor: .windowBackgroundColor
    )

    func matches(_ other: EditorChromeModel) -> Bool {
        document == other.document
            && statusBar == other.statusBar
            && backgroundColor.isEqual(other.backgroundColor)
    }
}

struct EditorChromeView: View {
    @ObservedObject var controller: EditorSurfaceController

    @AppStorage("swift.fileTreeSidebarWidth") private var storedFileTreeWidth: Double = 280
    @GestureState private var fileTreeDragTranslation: CGFloat = 0
    @State private var isFileTreeResizeActive = false

    private let minimumFileTreeWidth: CGFloat = 180
    private let maximumFileTreeWidth: CGFloat = 460

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 0) {
                if controller.fileTree.isVisible {
                    EditorFileTreeSidebarView(
                        tree: controller.fileTree,
                        theme: fileTreeTheme,
                        onSelectIndex: controller.clickFileTreeIndex,
                        onActivateIndex: controller.activateFileTreeIndex,
                        onVisibleRowsChanged: controller.setFileTreeVisibleRows,
                        onFocusSidebar: { controller.setFileTreeActive(true) }
                    )
                    .frame(width: fileTreeWidth)

                    EditorSidebarResizeHandle(
                        color: fileTreeTheme.separatorColor,
                        gesture: fileTreeResizeGesture
                    )
                }

                EditorSurfaceRepresentable(controller: controller)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            EditorStatusAccessoryView(chrome: controller.chrome, mode: controller.currentMode)
        }
        .background(
            EditorWindowChromeAccessor(
                chrome: controller.chrome,
                fileTreeVisible: controller.fileTree.isVisible,
                onToggleFileTree: controller.toggleFileTree
            )
        )
    }

    private var fileTreeTheme: EditorFileTreeSidebarTheme {
        EditorFileTreeSidebarTheme.resolve(scene: controller.scene, chrome: controller.chrome)
    }

    private var fileTreeWidth: CGFloat {
        clampFileTreeWidth(CGFloat(storedFileTreeWidth) + fileTreeDragTranslation)
    }

    private var fileTreeResizeGesture: some Gesture {
        DragGesture(minimumDistance: 0, coordinateSpace: .global)
            .updating($fileTreeDragTranslation) { value, state, _ in
                let translation = value.location.x - value.startLocation.x
                state = translation
                let startWidth = CGFloat(storedFileTreeWidth)
                let proposedWidth = startWidth + translation
                let startText = String(format: "%.1f", startWidth)
                let translationText = String(format: "%.1f", translation)
                let proposedText = String(format: "%.1f", proposedWidth)
                let clampedText = String(format: "%.1f", clampFileTreeWidth(proposedWidth))
                let startXText = String(format: "%.1f", value.startLocation.x)
                let locationXText = String(format: "%.1f", value.location.x)
                scrollPerfLog(
                    "fileTree.resize updating start=\(startText) translation=\(translationText) proposed=\(proposedText) clamped=\(clampedText) startX=\(startXText) currentX=\(locationXText)"
                )
            }
            .onChanged { _ in
                guard !isFileTreeResizeActive else { return }
                isFileTreeResizeActive = true
                controller.beginInteractiveResize(reason: "sidebar")
            }
            .onEnded { value in
                let translation = value.location.x - value.startLocation.x
                let committedWidth = clampFileTreeWidth(CGFloat(storedFileTreeWidth) + translation)
                let storedText = String(format: "%.1f", storedFileTreeWidth)
                let translationText = String(format: "%.1f", translation)
                let committedText = String(format: "%.1f", committedWidth)
                let startXText = String(format: "%.1f", value.startLocation.x)
                let locationXText = String(format: "%.1f", value.location.x)
                scrollPerfLog(
                    "fileTree.resize ended stored=\(storedText) translation=\(translationText) committed=\(committedText) startX=\(startXText) currentX=\(locationXText)"
                )
                storedFileTreeWidth = committedWidth
                if isFileTreeResizeActive {
                    isFileTreeResizeActive = false
                    controller.endInteractiveResize(reason: "sidebar")
                }
            }
    }

    private func clampFileTreeWidth(_ width: CGFloat) -> CGFloat {
        min(max(width, minimumFileTreeWidth), maximumFileTreeWidth)
    }
}

private struct EditorFileTreeSidebarTheme {
    let backgroundColor: NSColor
    let headerColor: NSColor
    let separatorColor: NSColor
    let selectionColor: NSColor
    let hoverColor: NSColor

    static func resolve(scene: EditorRenderScene?, chrome: EditorChromeModel) -> Self {
        let editorBackground = chromeBackgroundColor(base: scene?.backgroundColor ?? chrome.backgroundColor)
        let sidebarBackground = chromeBackgroundColor(base: scene?.gutterBackgroundColor ?? editorBackground)
        let selectionColor = chromeBackgroundColor(
            base: scene?.info.selectionColor?.color
                ?? sidebarBackground.blended(withFraction: 0.28, of: .systemBlue)
                ?? .systemBlue
        )
        let headerColor = sidebarBackground.adjustedBrightness(by: sidebarBackground.isLightColor ? -0.035 : 0.05)
        let separatorColor = sidebarBackground.adjustedBrightness(by: sidebarBackground.isLightColor ? -0.18 : 0.22)
            .withAlphaComponent(0.95)
        let hoverColor = selectionColor.withAlphaComponent(max(selectionColor.alphaComponent * 0.32, 0.08))
        return Self(
            backgroundColor: sidebarBackground,
            headerColor: headerColor,
            separatorColor: separatorColor,
            selectionColor: selectionColor,
            hoverColor: hoverColor
        )
    }
}

private struct EditorFileTreeSidebarView: View {
    let tree: EditorFileTreeState
    let theme: EditorFileTreeSidebarTheme
    let onSelectIndex: (Int) -> Void
    let onActivateIndex: (Int) -> Void
    let onVisibleRowsChanged: (Int) -> Void
    let onFocusSidebar: () -> Void

    @State private var hoveredRowID: String?
    @State private var reportedVisibleRows: Int = 1

    private let headerHeight: CGFloat = 30
    private let rowHeight: CGFloat = 24

    @State private var scrollViewportHeight: CGFloat = 1

    var body: some View {
        VStack(spacing: 0) {
            header

            ScrollViewReader { proxy in
                ScrollView(.vertical, showsIndicators: true) {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        if tree.rows.isEmpty {
                            VStack(alignment: .leading, spacing: 6) {
                                Text("Empty Folder")
                                    .font(.system(size: 12, weight: .semibold))
                                Text("Create a file or open another project folder.")
                                    .font(.system(size: 11))
                                    .foregroundStyle(.secondary)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 14)
                            .padding(.vertical, 16)
                        } else {
                            ForEach(Array(tree.rows.enumerated()), id: \.element.id) { index, row in
                                EditorFileTreeRowView(
                                    row: row,
                                    theme: theme,
                                    isHovered: hoveredRowID == row.id,
                                    onSelect: { onSelectIndex(index) },
                                    onActivate: { onActivateIndex(index) }
                                )
                                .id(row.id)
                                .onHover { isHovering in
                                    hoveredRowID = isHovering ? row.id : (hoveredRowID == row.id ? nil : hoveredRowID)
                                }
                            }
                        }
                    }
                    .padding(.horizontal, 6)
                    .padding(.vertical, 6)
                }
                .scrollContentBackground(.hidden)
                .scrollIndicators(.visible)
                .background(Color.clear)
                .simultaneousGesture(TapGesture().onEnded {
                    onFocusSidebar()
                })
                .background {
                    GeometryReader { proxy in
                        Color.clear
                            .onChange(of: proxy.size.height, initial: true) { _, newHeight in
                                scrollViewportHeight = newHeight
                                updateVisibleRows(for: newHeight)
                            }
                    }
                }
                .overlay(alignment: .trailing) {
                    fileTreeScrollbarThumb
                        .padding(.trailing, 2)
                }
                .onChange(of: tree.scrollOffset, initial: true) {
                    guard tree.rows.indices.contains(tree.scrollOffset) else { return }
                    proxy.scrollTo(tree.rows[tree.scrollOffset].id, anchor: .top)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Color(nsColor: theme.backgroundColor))
        .environment(\.colorScheme, theme.backgroundColor.isLightColor ? .light : .dark)
    }

    private var header: some View {
        HStack(spacing: 8) {
            Image(systemName: "folder.fill")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(Color(nsColor: theme.selectionColor).opacity(0.85))
            Text(rootTitle)
                .font(.system(size: 12, weight: .semibold))
                .lineLimit(1)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12)
        .frame(height: headerHeight)
        .background(Color(nsColor: theme.headerColor))
        .contentShape(Rectangle())
        .onTapGesture {
            onFocusSidebar()
        }
        .help(tree.root ?? rootTitle)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(Color(nsColor: theme.separatorColor))
                .frame(height: 1)
        }
    }

    private var rootTitle: String {
        guard let root = tree.root, !root.isEmpty else { return "Workspace" }
        let url = URL(fileURLWithPath: root)
        let name = url.lastPathComponent
        return name.isEmpty ? root : name
    }

    @ViewBuilder
    private var fileTreeScrollbarThumb: some View {
        let totalRows = max(tree.rows.count, reportedVisibleRows)
        let visibleRows = max(reportedVisibleRows, 1)
        if totalRows > visibleRows {
            let trackHeight = max(scrollViewportHeight - 8, 1)
            let thumbHeight = max(26, trackHeight * (CGFloat(visibleRows) / CGFloat(totalRows)))
            let maxOffset = max(totalRows - visibleRows, 1)
            let progress = CGFloat(min(max(tree.scrollOffset, 0), maxOffset)) / CGFloat(maxOffset)
            let travel = max(trackHeight - thumbHeight, 0)
            RoundedRectangle(cornerRadius: 2.5, style: .continuous)
                .fill(Color(nsColor: theme.separatorColor).opacity(0.92))
                .frame(width: 4, height: thumbHeight)
                .frame(maxHeight: .infinity, alignment: .top)
                .offset(y: 4 + (travel * progress))
                .allowsHitTesting(false)
        }
    }

    private func updateVisibleRows(for height: CGFloat) {
        let contentHeight = max(height - 12, 1)
        let rows = max(Int(floor(contentHeight / rowHeight)), 1)
        guard rows != reportedVisibleRows else { return }
        let heightText = String(format: "%.1f", height)
        scrollPerfLog(
            "fileTree.visibleRows height=\(heightText) rows=\(rows) previous=\(reportedVisibleRows) scrollOffset=\(tree.scrollOffset) totalRows=\(tree.rows.count)"
        )
        reportedVisibleRows = rows
        onVisibleRowsChanged(rows)
    }
}

private struct EditorFileTreeRowView: View {
    let row: EditorFileTreeRow
    let theme: EditorFileTreeSidebarTheme
    let isHovered: Bool
    let onSelect: () -> Void
    let onActivate: () -> Void

    var body: some View {
        Button(action: onSelect) {
            HStack(spacing: 0) {
                Rectangle()
                    .fill(Color(nsColor: leadingRailColor))
                    .frame(width: 2)
                    .opacity(row.isSelected || row.isCurrentFile ? 1 : 0)

                HStack(spacing: 6) {
                    if row.hasChildren || row.isDirectory {
                        Button(action: onActivate) {
                            Image(systemName: row.isExpanded ? "chevron.down" : "chevron.right")
                                .font(.system(size: 9, weight: .semibold))
                                .foregroundStyle(.secondary)
                                .frame(width: 10, height: 10)
                        }
                        .buttonStyle(.plain)
                    } else {
                        Color.clear
                            .frame(width: 10, height: 10)
                    }

                    Image(systemName: symbolName(for: row.iconName, isDirectory: row.isDirectory))
                        .font(.system(size: 11, weight: row.isDirectory ? .medium : .regular))
                        .foregroundStyle(Color(nsColor: iconColor))
                        .frame(width: 12)

                    Text(row.displayName)
                        .font(.system(size: 12, weight: row.isDirectory ? .medium : .regular))
                        .foregroundStyle(rowTextColor)
                        .lineLimit(1)

                    Spacer(minLength: 6)

                    if row.isCurrentFile {
                        Circle()
                            .fill(Color(nsColor: theme.selectionColor).opacity(row.isSelected ? 0.95 : 0.82))
                            .frame(width: 5, height: 5)
                    }
                }
                .padding(.leading, CGFloat(row.depth) * 11)
                .padding(.trailing, 8)
                .padding(.vertical, 4)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(selectionBackground)
            .clipShape(.rect(cornerRadius: 6))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .simultaneousGesture(TapGesture(count: 2).onEnded {
            onActivate()
        })
        .accessibilityAddTraits(row.isSelected ? [.isSelected] : [])
    }

    @ViewBuilder
    private var selectionBackground: some View {
        RoundedRectangle(cornerRadius: 6, style: .continuous)
            .fill(backgroundColor)
    }

    private var backgroundColor: Color {
        if row.isSelected {
            return Color(nsColor: theme.selectionColor).opacity(0.24)
        }
        if isHovered {
            return Color(nsColor: theme.hoverColor)
        }
        return .clear
    }

    private var leadingRailColor: NSColor {
        row.isSelected ? theme.selectionColor : theme.selectionColor.withAlphaComponent(0.78)
    }

    private var iconColor: NSColor {
        if row.isSelected {
            return .labelColor
        }
        if row.isCurrentFile {
            return theme.selectionColor
        }
        return row.isDirectory ? .secondaryLabelColor : .tertiaryLabelColor
    }

    private var rowTextColor: Color {
        if row.isSelected || row.isCurrentFile {
            return .primary
        }
        return .primary.opacity(0.88)
    }
}

private struct EditorSidebarResizeHandle<ResizeGesture: Gesture>: View {
    let color: NSColor
    let gesture: ResizeGesture

    @State private var isHovering = false

    var body: some View {
        Rectangle()
            .fill(Color.clear)
            .frame(width: 8)
            .overlay(alignment: .trailing) {
                Rectangle()
                    .fill(Color(nsColor: color).opacity(isHovering ? 1.0 : 0.9))
                    .frame(width: isHovering ? 2 : 1)
            }
            .contentShape(Rectangle())
            .onHover { hovering in
                isHovering = hovering
                if hovering {
                    NSCursor.resizeLeftRight.set()
                }
            }
            .gesture(gesture)
    }
}

private struct EditorStatusAccessoryView: View {
    let chrome: EditorChromeModel
    let mode: EditorMode

    private var metadataItems: [EditorStatusItem] {
        [
            chrome.document.languageName.map {
                EditorStatusItem(icon: "curlybraces", text: $0, emphasis: .muted)
            },
            chrome.document.encodingName.map {
                EditorStatusItem(icon: "textformat", text: $0, emphasis: .muted)
            },
            chrome.document.lineEndingName.map {
                EditorStatusItem(icon: "return", text: $0, emphasis: .muted)
            },
        ].compactMap { $0 }
    }

    private var lspStatus: EditorLSPStatusPresentation? {
        chrome.statusBar.items
            .lazy
            .compactMap { EditorLSPStatusPresentation(item: $0) }
            .first
    }

    private var nonLSPStatusItems: [EditorStatusItem] {
        chrome.statusBar.items.filter { EditorLSPStatusPresentation(item: $0) == nil }
    }

    var body: some View {
        HStack(spacing: 12) {
            ModePill(mode: mode)

            if let lspStatus {
                LSPStatusAccessoryView(status: lspStatus)
            }

            Spacer(minLength: 12)

            HStack(spacing: 10) {
                ForEach(nonLSPStatusItems) { item in
                    StatusAccessoryItemView(item: item)
                }

                ForEach(metadataItems) { item in
                    StatusAccessoryItemView(item: item)
                }

                Text(chrome.statusBar.cursorText)
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundStyle(.primary)
                    .lineLimit(1)
                    .padding(.leading, 4)
            }
        }
        .padding(.horizontal, 14)
        .frame(height: 28)
        .background(Color(nsColor: chromeBackgroundColor(base: chrome.backgroundColor)))
        .overlay(alignment: .top) {
            Divider()
        }
        .accessibilityElement(children: .contain)
    }
}

private struct StatusAccessoryItemView: View {
    let item: EditorStatusItem

    private var normalized: (icon: String?, text: String) {
        normalizedStatusItemDisplay(icon: item.icon, text: item.text)
    }

    var body: some View {
        HStack(spacing: 5) {
            if let icon = normalized.icon {
                Image(systemName: symbolName(for: icon, isDirectory: false))
                    .font(.system(size: 10, weight: .semibold))
            }

            if !normalized.text.isEmpty {
                Text(normalized.text)
                    .font(textFont)
                    .lineLimit(1)
            }
        }
        .foregroundStyle(foregroundStyle)
        .accessibilityElement(children: .combine)
    }

    private var textFont: Font {
        switch item.emphasis {
        case .normal:
            return .system(size: 11, weight: .regular)
        case .muted:
            return .system(size: 11, weight: .regular)
        case .strong:
            return .system(size: 11, weight: .semibold)
        }
    }

    private var foregroundStyle: Color {
        if let icon = normalized.icon {
            switch icon {
            case "diagnostic_error":
                return .red
            case "diagnostic_warning":
                return .orange
            case "diagnostic_info":
                return .blue
            case "diagnostic_hint":
                return .teal
            case "pi":
                return .purple
            case "copilot", "copilot_disabled", "copilot_init", "copilot_error", "supermaven", "supermaven_disabled", "supermaven_init", "supermaven_error":
                return .accentColor
            default:
                break
            }
        }

        switch item.emphasis {
        case .normal:
            return .primary
        case .muted:
            return .secondary
        case .strong:
            return .primary
        }
    }
}

private struct EditorLSPStatusPresentation: Equatable {
    enum Phase: Equatable {
        case unavailable
        case off(String?)
        case loading(String)
        case ready(String?)
        case error(String?)
    }

    let phase: Phase

    init?(item: EditorStatusItem) {
        let raw = item.text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard raw.hasPrefix("lsp:") else { return nil }
        let payload = raw.dropFirst(4).trimmingCharacters(in: .whitespaces)

        if payload == "unavailable" {
            phase = .unavailable
            return
        }

        if payload.hasPrefix("ready") {
            phase = .ready(Self.extractParentheticalDetail(from: String(payload)))
            return
        }

        if payload.hasPrefix("error") {
            phase = .error(Self.extractParentheticalDetail(from: String(payload)))
            return
        }

        if payload.hasPrefix("off") {
            let detail = payload.dropFirst(3).trimmingCharacters(in: .whitespacesAndNewlines)
            phase = .off(detail.isEmpty ? nil : detail)
            return
        }

        let cleaned = String(payload).trimmingCharacters(in: CharacterSet(charactersIn: "⣾⣽⣻⢿⡿⣟⣯⣷ "))
        phase = .loading(cleaned.isEmpty ? "Language Server" : cleaned)
    }

    private static func extractParentheticalDetail(from text: String) -> String? {
        guard let open = text.firstIndex(of: "("), let close = text.lastIndex(of: ")"), open < close else {
            return nil
        }
        let detail = text[text.index(after: open)..<close].trimmingCharacters(in: .whitespacesAndNewlines)
        return detail.isEmpty ? nil : detail
    }
}

private struct LSPStatusAccessoryView: View {
    let status: EditorLSPStatusPresentation

    @State private var isExpanded = true

    var body: some View {
        Button(action: toggleExpanded) {
            Group {
                if isExpanded {
                    expandedBody
                        .transition(.asymmetric(insertion: .opacity.combined(with: .scale(scale: 0.96)), removal: .opacity))
                } else {
                    collapsedBody
                        .transition(.asymmetric(insertion: .opacity.combined(with: .scale(scale: 0.92)), removal: .opacity))
                }
            }
        }
        .buttonStyle(.plain)
        .animation(.spring(response: 0.24, dampingFraction: 0.88), value: isExpanded)
        .accessibilityLabel(accessibilityLabel)
        .accessibilityHint(isExpanded ? "Collapse language server status" : "Expand language server status")
    }

    private var expandedBody: some View {
        HStack(spacing: 8) {
            statusDot

            Text(title)
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(.primary)
                .lineLimit(1)

            if case .loading = status.phase {
                LSPIndeterminateBar()
                    .frame(width: 112, height: 4)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(
            Capsule(style: .continuous)
                .fill(backgroundColor)
        )
        .overlay {
            Capsule(style: .continuous)
                .strokeBorder(borderColor, lineWidth: 1)
        }
    }

    private var collapsedBody: some View {
        ZStack {
            Circle()
                .fill(backgroundColor)
            Circle()
                .strokeBorder(borderColor, lineWidth: 1)
            statusDot
        }
        .frame(width: 18, height: 18)
    }

    @ViewBuilder
    private var statusDot: some View {
        Circle()
            .fill(dotColor)
            .frame(width: 7, height: 7)
            .overlay {
                if case .loading = status.phase {
                    Circle()
                        .stroke(dotColor.opacity(0.24), lineWidth: 4)
                        .scaleEffect(1.55)
                }
            }
    }

    private var title: String {
        switch status.phase {
        case .unavailable:
            return "Language Server Unavailable"
        case .off(let detail):
            return detail ?? "Language Server Off"
        case .loading(let detail):
            return detail
        case .ready(let detail):
            return detail ?? "Language Server Ready"
        case .error(let detail):
            return detail ?? "Language Server Error"
        }
    }

    private var accessibilityLabel: String {
        switch status.phase {
        case .unavailable:
            return "Language server unavailable"
        case .off(let detail):
            return detail.map { "Language server \($0)" } ?? "Language server off"
        case .loading(let detail):
            return "Language server loading: \(detail)"
        case .ready(let detail):
            return detail.map { "Language server ready: \($0)" } ?? "Language server ready"
        case .error(let detail):
            return detail.map { "Language server error: \($0)" } ?? "Language server error"
        }
    }

    private var dotColor: Color {
        switch status.phase {
        case .unavailable, .off:
            return .secondary
        case .loading:
            return .accentColor
        case .ready:
            return .green
        case .error:
            return .red
        }
    }

    private var backgroundColor: Color {
        switch status.phase {
        case .unavailable, .off:
            return Color.primary.opacity(0.05)
        case .loading:
            return Color.accentColor.opacity(0.10)
        case .ready:
            return Color.green.opacity(0.10)
        case .error:
            return Color.red.opacity(0.10)
        }
    }

    private var borderColor: Color {
        switch status.phase {
        case .unavailable, .off:
            return Color.primary.opacity(0.06)
        case .loading:
            return Color.accentColor.opacity(0.18)
        case .ready:
            return Color.green.opacity(0.18)
        case .error:
            return Color.red.opacity(0.18)
        }
    }

    private func toggleExpanded() {
        isExpanded.toggle()
    }
}

private struct LSPIndeterminateBar: View {
    @State private var phase: CGFloat = -0.55

    var body: some View {
        GeometryReader { proxy in
            let width = proxy.size.width
            let fillWidth = max(width * 0.38, 28)

            Capsule(style: .continuous)
                .fill(Color.accentColor.opacity(0.16))
                .overlay(alignment: .leading) {
                    Capsule(style: .continuous)
                        .fill(
                            LinearGradient(
                                colors: [
                                    Color.accentColor.opacity(0.55),
                                    Color.accentColor.opacity(0.95)
                                ],
                                startPoint: .leading,
                                endPoint: .trailing
                            )
                        )
                        .frame(width: fillWidth)
                        .offset(x: phase * max(width - fillWidth, 1))
                }
                .clipShape(Capsule(style: .continuous))
                .onAppear {
                    phase = -0.55
                    withAnimation(.linear(duration: 1.05).repeatForever(autoreverses: false)) {
                        phase = 1.0
                    }
                }
        }
        .accessibilityHidden(true)
    }
}

private struct ModePill: View {
    let mode: EditorMode

    var body: some View {
        Text(label)
            .font(.system(size: 10, weight: .semibold, design: .rounded))
            .foregroundStyle(foreground)
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(
                Capsule(style: .continuous)
                    .fill(background)
            )
            .accessibilityLabel(label)
    }

    private var label: String {
        switch mode {
        case .normal:
            return "Normal"
        case .insert:
            return "Insert"
        case .select:
            return "Select"
        case .command:
            return "Command"
        }
    }

    private var foreground: Color {
        switch mode {
        case .normal:
            return .secondary
        case .insert:
            return .accentColor
        case .select:
            return .purple
        case .command:
            return .orange
        }
    }

    private var background: Color {
        switch mode {
        case .normal:
            return Color.secondary.opacity(0.12)
        case .insert:
            return Color.accentColor.opacity(0.14)
        case .select:
            return Color.purple.opacity(0.14)
        case .command:
            return Color.orange.opacity(0.14)
        }
    }
}

private struct EditorWindowChromeAccessor: NSViewRepresentable {
    let chrome: EditorChromeModel
    let fileTreeVisible: Bool
    let onToggleFileTree: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        view.isHidden = true
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        if let window = nsView.window {
            context.coordinator.configure(window: window, chrome: chrome, fileTreeVisible: fileTreeVisible, onToggleFileTree: onToggleFileTree)
            return
        }

        DispatchQueue.main.async { [weak nsView] in
            guard let nsView, let window = nsView.window else { return }
            context.coordinator.configure(window: window, chrome: chrome, fileTreeVisible: fileTreeVisible, onToggleFileTree: onToggleFileTree)
        }
    }

    @MainActor
    final class Coordinator: NSObject, NSToolbarDelegate {
        private let toolbarIdentifier = NSToolbar.Identifier("TheSwiftPOC.TitlebarToolbar")
        private let fileTreeItemIdentifier = NSToolbarItem.Identifier("TheSwiftPOC.FileTreeToggle")
        private let documentItemIdentifier = NSToolbarItem.Identifier("TheSwiftPOC.DocumentInfo")
        private let vcsItemIdentifier = NSToolbarItem.Identifier("TheSwiftPOC.VCSInfo")
        private let fileTreeHostingView = NSHostingView(rootView: EditorTitlebarSidebarToggleButton(isActive: false, onToggle: {}))
        private let documentHostingView = NSHostingView(rootView: EditorTitlebarDocumentView(document: .empty))
        private let vcsHostingView = NSHostingView(rootView: EditorTitlebarVCSView(vcsText: nil))
        private weak var observedWindow: NSWindow?
        private var lastChrome: EditorChromeModel = .empty
        private var lastFileTreeVisible = false
        private var toggleFileTreeAction: (() -> Void)?
        private lazy var toolbar: NSToolbar = {
            let toolbar = NSToolbar(identifier: toolbarIdentifier)
            toolbar.delegate = self
            toolbar.displayMode = .iconOnly
            toolbar.allowsUserCustomization = false
            toolbar.autosavesConfiguration = false
            toolbar.showsBaselineSeparator = false
            return toolbar
        }()

        override init() {
            super.init()
            fileTreeHostingView.translatesAutoresizingMaskIntoConstraints = false
            fileTreeHostingView.setContentCompressionResistancePriority(.required, for: .horizontal)
            fileTreeHostingView.setContentHuggingPriority(.required, for: .horizontal)
            documentHostingView.translatesAutoresizingMaskIntoConstraints = false
            documentHostingView.setContentCompressionResistancePriority(.required, for: .horizontal)
            documentHostingView.setContentHuggingPriority(.required, for: .horizontal)
            vcsHostingView.translatesAutoresizingMaskIntoConstraints = false
            vcsHostingView.setContentCompressionResistancePriority(.required, for: .horizontal)
            vcsHostingView.setContentHuggingPriority(.required, for: .horizontal)
        }

        deinit {
            NotificationCenter.default.removeObserver(self)
        }

        func configure(window: NSWindow, chrome: EditorChromeModel, fileTreeVisible: Bool, onToggleFileTree: @escaping () -> Void) {
            let started = CFAbsoluteTimeGetCurrent()
            let windowChanged = observedWindow !== window
            let chromeChanged = !chrome.matches(lastChrome)
            let fileTreeChanged = fileTreeVisible != lastFileTreeVisible
            toggleFileTreeAction = onToggleFileTree
            attachWindowObserversIfNeeded(window: window)
            installToolbarIfNeeded(window: window)
            guard windowChanged || chromeChanged || fileTreeChanged else {
                scrollPerfLog("chrome.configure skipped windowChanged=\(windowChanged) chromeChanged=\(chromeChanged) fileTreeChanged=\(fileTreeChanged)")
                return
            }
            lastChrome = chrome
            lastFileTreeVisible = fileTreeVisible
            let applyStarted = CFAbsoluteTimeGetCurrent()
            applyWindowChrome(window: window, chrome: chrome)
            let applyMs = (CFAbsoluteTimeGetCurrent() - applyStarted) * 1000
            let toolbarStarted = CFAbsoluteTimeGetCurrent()
            updateToolbarContent(window: window, chrome: chrome, fileTreeVisible: fileTreeVisible)
            let toolbarMs = (CFAbsoluteTimeGetCurrent() - toolbarStarted) * 1000
            let totalMs = (CFAbsoluteTimeGetCurrent() - started) * 1000
            scrollPerfLog(
                "chrome.configure windowChanged=\(windowChanged) chromeChanged=\(chromeChanged) applyMs=\(String(format: "%.2f", applyMs)) toolbarMs=\(String(format: "%.2f", toolbarMs)) totalMs=\(String(format: "%.2f", totalMs))"
            )
        }

        private func attachWindowObserversIfNeeded(window: NSWindow) {
            guard observedWindow !== window else { return }
            NotificationCenter.default.removeObserver(self)
            observedWindow = window
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(windowDidChangeState(_:)),
                name: NSWindow.didBecomeMainNotification,
                object: window
            )
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(windowDidChangeState(_:)),
                name: NSWindow.didResignMainNotification,
                object: window
            )
        }

        private func installToolbarIfNeeded(window: NSWindow) {
            if window.toolbar?.identifier != toolbarIdentifier {
                window.toolbar = toolbar
            }
            window.toolbarStyle = .unifiedCompact
        }

        @objc private func windowDidChangeState(_ notification: Notification) {
            guard let window = notification.object as? NSWindow else { return }
            applyWindowChrome(window: window, chrome: lastChrome)
            updateToolbarContent(window: window, chrome: lastChrome, fileTreeVisible: lastFileTreeVisible)
        }

        private func applyWindowChrome(window: NSWindow, chrome: EditorChromeModel) {
            let backgroundColor = chrome.backgroundColor
            window.titleVisibility = .hidden
            window.titlebarAppearsTransparent = false
            window.titlebarSeparatorStyle = .none
            window.toolbarStyle = .unifiedCompact
            window.backgroundColor = backgroundColor.withAlphaComponent(1)
            window.isDocumentEdited = chrome.document.isModified
            window.appearance = backgroundColor.isLightColor
                ? NSAppearance(named: .aqua)
                : NSAppearance(named: .darkAqua)

            let title = windowTitle(for: chrome)
            if window.title != title {
                window.title = title
            }

            if let absolutePath = chrome.document.absolutePath, !absolutePath.isEmpty {
                window.representedURL = URL(fileURLWithPath: absolutePath)
            } else {
                window.representedURL = nil
            }

            applyTitlebarBackground(window: window, color: backgroundColor)
        }

        private func updateToolbarContent(window: NSWindow, chrome: EditorChromeModel, fileTreeVisible: Bool) {
            fileTreeHostingView.rootView = EditorTitlebarSidebarToggleButton(isActive: fileTreeVisible) {
                self.toggleFileTreeAction?()
            }
            documentHostingView.rootView = EditorTitlebarDocumentView(document: chrome.document)
            vcsHostingView.rootView = EditorTitlebarVCSView(vcsText: chrome.document.vcsText)
            fileTreeHostingView.invalidateIntrinsicContentSize()
            documentHostingView.invalidateIntrinsicContentSize()
            vcsHostingView.invalidateIntrinsicContentSize()
            window.toolbar?.validateVisibleItems()
        }

        private func applyTitlebarBackground(window: NSWindow, color: NSColor) {
            if #available(macOS 26.0, *) {
                if let titlebarView = titlebarView(for: window) {
                    titlebarView.wantsLayer = true
                    titlebarView.layer?.backgroundColor = color.cgColor
                }
                titlebarBackgroundView(for: window)?.isHidden = true
            } else {
                window.titlebarAppearsTransparent = true
                guard let titlebarContainer = titlebarContainer(for: window) else { return }
                titlebarContainer.wantsLayer = true
                titlebarContainer.layer?.backgroundColor = color.cgColor
                hideFirstEffectView(in: titlebarContainer)
            }
        }

        private func windowTitle(for chrome: EditorChromeModel) -> String {
            if let relativePath = chrome.document.relativePath, !relativePath.isEmpty {
                return "\(relativePath)/\(chrome.document.name)"
            }
            return chrome.document.name
        }

        private func titlebarContainer(for window: NSWindow) -> NSView? {
            if !window.styleMask.contains(.fullScreen) {
                guard let contentView = window.contentView else { return nil }
                return firstViewFromRoot(startingAt: contentView, classNameContains: "NSTitlebarContainerView")
            }

            for appWindow in NSApplication.shared.windows {
                guard NSStringFromClass(type(of: appWindow)).contains("NSToolbarFullScreenWindow") else { continue }
                guard appWindow.parent == window else { continue }
                guard let contentView = appWindow.contentView else { continue }
                return firstViewFromRoot(startingAt: contentView, classNameContains: "NSTitlebarContainerView")
            }
            return nil
        }

        private func titlebarView(for window: NSWindow) -> NSView? {
            titlebarContainer(for: window).flatMap { firstView(from: $0, classNameContains: "NSTitlebarView") }
        }

        private func titlebarBackgroundView(for window: NSWindow) -> NSView? {
            titlebarContainer(for: window).flatMap { firstView(from: $0, classNameContains: "NSTitlebarBackgroundView") }
        }

        private func firstViewFromRoot(startingAt view: NSView, classNameContains needle: String) -> NSView? {
            var root = view
            while let superview = root.superview {
                root = superview
            }
            return firstView(from: root, classNameContains: needle)
        }

        private func firstView(from root: NSView, classNameContains needle: String) -> NSView? {
            if NSStringFromClass(type(of: root)).contains(needle) {
                return root
            }
            for subview in root.subviews {
                if let match = firstView(from: subview, classNameContains: needle) {
                    return match
                }
            }
            return nil
        }

        private func hideFirstEffectView(in root: NSView) {
            if let effectView = firstEffectView(in: root) {
                effectView.isHidden = true
            }
        }

        private func firstEffectView(in root: NSView) -> NSVisualEffectView? {
            if let effectView = root as? NSVisualEffectView {
                return effectView
            }
            for subview in root.subviews {
                if let match = firstEffectView(in: subview) {
                    return match
                }
            }
            return nil
        }

        func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
            [fileTreeItemIdentifier, documentItemIdentifier, .flexibleSpace, vcsItemIdentifier]
        }

        func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
            [fileTreeItemIdentifier, documentItemIdentifier, .flexibleSpace, vcsItemIdentifier]
        }

        func toolbar(
            _ toolbar: NSToolbar,
            itemForItemIdentifier itemIdentifier: NSToolbarItem.Identifier,
            willBeInsertedIntoToolbar flag: Bool
        ) -> NSToolbarItem? {
            let item = NSToolbarItem(itemIdentifier: itemIdentifier)
            item.isBordered = false

            switch itemIdentifier {
            case fileTreeItemIdentifier:
                item.view = fileTreeHostingView
                item.visibilityPriority = .high
                return item
            case documentItemIdentifier:
                item.view = documentHostingView
                item.visibilityPriority = .high
                return item
            case vcsItemIdentifier:
                item.view = vcsHostingView
                item.visibilityPriority = .low
                return item
            default:
                return nil
            }
        }
    }
}

private struct EditorTitlebarSidebarToggleButton: View {
    let isActive: Bool
    let onToggle: () -> Void

    var body: some View {
        Button(action: onToggle) {
            Image(systemName: "sidebar.left")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(isActive ? .primary : .secondary)
                .frame(width: 28, height: 24)
                .background {
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(isActive ? Color.primary.opacity(0.10) : Color.clear)
                }
        }
        .buttonStyle(.plain)
        .help("Toggle Sidebar")
        .accessibilityLabel("Toggle Sidebar")
    }
}

private struct EditorTitlebarDocumentView: View {
    let document: EditorDocumentChrome

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: symbolName(for: document.icon, isDirectory: false))
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.secondary)

            Text(document.name)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.primary)
                .lineLimit(1)
        }
        .fixedSize()
        .allowsHitTesting(false)
        .accessibilityElement(children: .combine)
    }
}

private struct EditorTitlebarVCSView: View {
    let vcsText: String?

    private var trimmedVCSText: String? {
        let trimmed = vcsText?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let trimmed, !trimmed.isEmpty else { return nil }
        return trimmed
    }

    var body: some View {
        Group {
            if let trimmedVCSText {
                HStack(spacing: 6) {
                    Image(systemName: symbolName(for: "git_branch", isDirectory: false))
                        .font(.system(size: 11, weight: .semibold))
                    Text(trimmedVCSText)
                        .font(.system(size: 12, weight: .medium))
                        .lineLimit(1)
                }
                .foregroundStyle(.secondary)
                .fixedSize()
                .allowsHitTesting(false)
                .accessibilityElement(children: .combine)
            } else {
                Color.clear
                    .frame(width: 1, height: 1)
                    .allowsHitTesting(false)
                    .accessibilityHidden(true)
            }
        }
    }
}

private func chromeBackgroundColor(base: NSColor) -> NSColor {
    base.usingColorSpace(.sRGB) ?? base
}

private extension NSColor {
    var isLightColor: Bool {
        guard let color = usingColorSpace(.sRGB) else { return false }
        let luminance = (0.299 * color.redComponent) + (0.587 * color.greenComponent) + (0.114 * color.blueComponent)
        return luminance > 0.7
    }

    func adjustedBrightness(by amount: CGFloat) -> NSColor {
        guard let color = usingColorSpace(.sRGB) else { return self }
        return NSColor(
            calibratedRed: min(max(color.redComponent + amount, 0), 1),
            green: min(max(color.greenComponent + amount, 0), 1),
            blue: min(max(color.blueComponent + amount, 0), 1),
            alpha: color.alphaComponent
        )
    }
}

private func normalizedStatusItemDisplay(icon: String?, text: String) -> (icon: String?, text: String) {
    if let icon {
        return (icon, text)
    }

    let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
    let glyphMappings: [(String, String)] = [
        ("", "diagnostic_error"),
        ("", "diagnostic_warning"),
        ("", "diagnostic_info"),
        ("󰌵", "diagnostic_hint"),
    ]
    for (glyph, iconName) in glyphMappings where trimmed.hasPrefix(glyph) {
        let remainder = trimmed.dropFirst(glyph.count).trimmingCharacters(in: .whitespacesAndNewlines)
        return (iconName, remainder)
    }
    return (nil, text)
}

private func symbolName(for icon: String, isDirectory: Bool) -> String {
    switch icon {
    case "folder", "folder_open", "folder_search":
        return isDirectory ? "folder.fill" : "folder"
    case "book":
        return "book.closed"
    case "swift":
        return "swift"
    case "rust", "file_rust":
        return "gearshape.2"
    case "file_markdown":
        return "doc.text"
    case "terminal":
        return "terminal"
    case "image":
        return "photo"
    case "json", "file_toml", "settings", "tool_hammer":
        return "doc.badge.gearshape"
    case "git_branch":
        return "point.topleft.down.curvedto.point.bottomright.up"
    case "diagnostic_error":
        return "xmark.circle.fill"
    case "diagnostic_warning":
        return "exclamationmark.triangle.fill"
    case "diagnostic_info":
        return "info.circle.fill"
    case "diagnostic_hint":
        return "lightbulb.fill"
    case "pi":
        return "sparkles"
    case "curlybraces", "textformat", "return":
        return icon
    case "copilot", "supermaven":
        return "wand.and.stars"
    case "copilot_disabled", "supermaven_disabled":
        return "slash.circle"
    case "copilot_init", "supermaven_init":
        return "arrow.triangle.2.circlepath"
    case "copilot_error", "supermaven_error":
        return "exclamationmark.circle"
    default:
        return isDirectory ? "folder" : "doc"
    }
}
