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

    private enum CodingKeys: String, CodingKey {
        case active
        case query
        case matchedCount = "matched_count"
        case totalCount = "total_count"
        case scanning
        case root
        case items
    }
}

struct FilePickerItemSnapshot: Decodable, Identifiable {
    let id: Int
    let display: String
    let isDir: Bool

    private enum CodingKeys: String, CodingKey {
        case display
        case isDir = "is_dir"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        display = try container.decode(String.self, forKey: .display)
        isDir = (try? container.decode(Bool.self, forKey: .isDir)) ?? false
        // id will be set by the parent during array mapping
        id = 0
    }

    init(id: Int, display: String, isDir: Bool) {
        self.id = id
        self.display = display
        self.isDir = isDir
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
}

// MARK: - File type icons

fileprivate func fileIcon(for item: FilePickerItemSnapshot) -> (symbol: String, color: Color) {
    if item.isDir {
        return ("folder.fill", .secondary)
    }
    switch item.fileExtension {
    case "swift":
        return ("swift", .orange)
    case "rs":
        return ("gearshape.2.fill", Color(red: 0.72, green: 0.35, blue: 0.16))
    case "js", "jsx":
        return ("doc.text.fill", .yellow)
    case "ts", "tsx":
        return ("doc.text.fill", .blue)
    case "py":
        return ("doc.text.fill", Color(red: 0.2, green: 0.6, blue: 0.85))
    case "rb":
        return ("doc.text.fill", .red)
    case "go":
        return ("doc.text.fill", .cyan)
    case "md", "markdown":
        return ("doc.richtext.fill", .gray)
    case "json", "yaml", "yml", "toml":
        return ("gearshape.fill", .gray)
    case "html", "htm":
        return ("globe", .orange)
    case "css", "scss", "less":
        return ("paintbrush.fill", .pink)
    case "png", "jpg", "jpeg", "gif", "svg", "ico", "webp":
        return ("photo.fill", .green)
    case "pdf":
        return ("doc.fill", .red)
    case "txt", "log":
        return ("doc.text", .secondary)
    case "sh", "bash", "zsh", "fish":
        return ("terminal.fill", .green)
    case "c", "h":
        return ("doc.text.fill", Color(red: 0.3, green: 0.5, blue: 0.8))
    case "cpp", "cc", "hpp", "cxx":
        return ("doc.text.fill", Color(red: 0.3, green: 0.5, blue: 0.8))
    case "java", "kt", "kts":
        return ("doc.text.fill", .orange)
    case "xml":
        return ("chevron.left.forwardslash.chevron.right", .gray)
    case "lock":
        return ("lock.fill", .gray)
    default:
        return ("doc.fill", .secondary)
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

    @State private var query: String = ""
    @State private var selectedIndex: Int? = nil
    @State private var hoveredIndex: Int? = nil
    @FocusState private var isTextFieldFocused: Bool

    private let paletteWidth: CGFloat = 600
    private let maxListHeight: CGFloat = 380
    private let pageSize: Int = 12
    private let backgroundColor: Color = Color(nsColor: .windowBackgroundColor)

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

    var body: some View {
        pickerCard
            .onAppear {
                query = snapshot.query ?? ""
                selectedIndex = items.isEmpty ? nil : 0
                DispatchQueue.main.async {
                    isTextFieldFocused = true
                }
            }
            .onChange(of: snapshot.query) { newValue in
                let newQuery = newValue ?? ""
                if newQuery != query {
                    query = newQuery
                }
                syncSelection()
            }
            .onChange(of: items.count) { _ in
                syncSelection()
            }
    }

    // MARK: - Card

    private var pickerCard: some View {
        VStack(alignment: .leading, spacing: 0) {
            pickerHeader
            Divider()
            pickerList
        }
        .frame(maxWidth: paletteWidth)
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
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
        .padding()
    }

    // MARK: - Header

    private var pickerHeader: some View {
        ZStack(alignment: .leading) {
            pickerKeyboardShortcuts
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(.secondary)

                TextField("Open file…", text: $query)
                    .font(.system(size: 16, weight: .light))
                    .textFieldStyle(.plain)
                    .focused($isTextFieldFocused)
                    .onSubmit {
                        if let selected = selectedIndex {
                            onSubmit(selected)
                        }
                    }
                    .onExitCommand {
                        onClose()
                    }
                    .onChange(of: query) { newValue in
                        if newValue != (snapshot.query ?? "") {
                            onQueryChange(newValue)
                        }
                    }

                Spacer()

                statusText
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    @ViewBuilder
    private var statusText: some View {
        if items.isEmpty && !query.isEmpty && !isScanning {
            Text("No matches")
                .font(.system(size: 12))
                .foregroundStyle(.tertiary)
        } else if isScanning && matchedCount == 0 {
            ScanningText()
        } else {
            HStack(spacing: 6) {
                if matchedCount > 0 && matchedCount < totalCount {
                    Text("\(matchedCount) of \(totalCount)")
                        .font(.system(size: 12))
                        .foregroundStyle(.tertiary)
                } else if totalCount > 0 {
                    Text("\(totalCount) files")
                        .font(.system(size: 12))
                        .foregroundStyle(.tertiary)
                }
                if isScanning {
                    ScanningText()
                }
            }
        }
    }

    // MARK: - Keyboard shortcuts

    private var pickerKeyboardShortcuts: some View {
        Group {
            // Arrow keys
            Button { moveSelection(-1) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.upArrow, modifiers: [])
            Button { moveSelection(1) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.downArrow, modifiers: [])

            // Ctrl+P / Ctrl+N (vim-style)
            Button { moveSelection(-1) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.init("p"), modifiers: [.control])
            Button { moveSelection(1) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.init("n"), modifiers: [.control])

            // Tab / Shift+Tab
            Button { moveSelection(1) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.tab, modifiers: [])
            Button { moveSelection(-1) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.tab, modifiers: [.shift])

            // Page Up / Page Down
            Button { moveSelection(-pageSize) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.init("u"), modifiers: [.control])
            Button { moveSelection(pageSize) } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.init("d"), modifiers: [.control])

            // Ctrl+C to close
            Button { onClose() } label: { Color.clear }
                .buttonStyle(.plain)
                .keyboardShortcut(.init("c"), modifiers: [.control])
        }
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }

    // MARK: - List

    @ViewBuilder
    private var pickerList: some View {
        if items.isEmpty {
            emptyState
        } else {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 4) {
                        ForEach(items) { item in
                            pickerRow(for: item)
                                .id(item.id)
                        }
                    }
                    .padding(10)
                }
                .frame(maxHeight: maxListHeight)
                .onChange(of: selectedIndex) { newIndex in
                    guard let index = newIndex else { return }
                    withAnimation(.easeOut(duration: 0.1)) {
                        proxy.scrollTo(index, anchor: .center)
                    }
                }
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 8) {
            Image(systemName: "doc.questionmark")
                .font(.system(size: 24))
                .foregroundStyle(.tertiary)
            Text("No matching files")
                .font(.system(size: 14))
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, minHeight: 80)
        .padding()
    }

    // MARK: - Row

    private func pickerRow(for item: FilePickerItemSnapshot) -> some View {
        let isSelected = selectedIndex == item.id
        let isHovered = hoveredIndex == item.id
        let (iconName, iconColor) = fileIcon(for: item)

        return Button {
            selectedIndex = item.id
            onSubmit(item.id)
        } label: {
            HStack(spacing: 8) {
                Image(systemName: iconName)
                    .foregroundStyle(iconColor)
                    .font(.system(size: 14, weight: .medium))
                    .frame(width: 18, alignment: .center)

                VStack(alignment: .leading, spacing: 2) {
                    Text(item.filename)
                        .font(.system(size: 14, weight: item.isDir ? .semibold : .medium))
                        .foregroundColor(.primary)
                        .lineLimit(1)

                    if !item.directory.isEmpty {
                        Text(item.directory)
                            .font(.system(size: 12))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                }

                Spacer()
            }
            .padding(8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(isSelected ? Color.accentColor.opacity(0.2) : (isHovered ? Color.secondary.opacity(0.15) : Color.clear))
            )
        }
        .buttonStyle(.plain)
        .contentShape(Rectangle())
        .onHover { hovering in
            hoveredIndex = hovering ? item.id : nil
        }
    }

    // MARK: - Selection logic

    private func moveSelection(_ delta: Int) {
        guard !items.isEmpty else { return }

        let current = selectedIndex ?? (delta > 0 ? -1 : items.count)
        var next = current + delta

        // Clamp to bounds (no wrapping for multi-step moves, wrap for single step)
        if abs(delta) == 1 {
            if next < 0 { next = items.count - 1 }
            if next >= items.count { next = 0 }
        } else {
            next = max(0, min(next, items.count - 1))
        }

        selectedIndex = next
    }

    private func syncSelection() {
        if items.isEmpty {
            selectedIndex = nil
            return
        }
        if let idx = selectedIndex, idx < items.count {
            return
        }
        selectedIndex = 0
    }
}

// MARK: - Scanning indicator

fileprivate struct ScanningText: View {
    @State private var opacity: Double = 1.0

    var body: some View {
        Text("Scanning…")
            .font(.system(size: 12))
            .foregroundStyle(.tertiary)
            .opacity(opacity)
            .onAppear {
                withAnimation(.easeInOut(duration: 0.8).repeatForever(autoreverses: true)) {
                    opacity = 0.4
                }
            }
    }
}
